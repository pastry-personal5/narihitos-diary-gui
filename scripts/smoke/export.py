"""P1-M25/M26/M27 export coordination on the fault-injection diary-server binary. Legs 0-4 use the
directory-backed fake destination; leg 5 uses the real macOS iCloud adapter on a temp directory
(no live iCloud), plus cancel and abandon; leg 6 uses the real Notion adapter over a file-backed
in-memory fake service (no network). create, verified no-op, external-change confirmation, duplicate confirmation,
disabled target, and kill/restart recovery by read-back. Synthetic data, private scratch dir.
No real iCloud or Notion contact happens. The live Notion round trip is notion-live.py."""
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid

root = Path(__file__).resolve().parents[2]
manifest_path = str(root / 'src-tauri/Cargo.toml')
target = root / 'src-tauri/target/fault'
subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', manifest_path, '-p', 'diary-server', '--features', 'fault-injection', '--target-dir', str(target)], check=True)
binary = target / 'debug/diary-server'
DATE = '2026-11-07'
OP = lambda: str(uuid.uuid4())

with tempfile.TemporaryDirectory(prefix='diary-export-smoke-') as temporary:
    base = Path(temporary).resolve()
    workspace, runtime, remote = base / 'workspace', base / 'runtime', base / 'remote'
    for folder in [base, workspace, runtime, remote]:
        folder.mkdir(exist_ok=True, mode=0o700)
        folder.chmod(0o700)
    config = base / 'config.yaml'
    config.write_text(f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\n')
    config.chmod(0o600)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    file = remote / f'{DATE}.md'

    def start(fault=None, destination=True, cfg=None, notion=None):
        env = dict(os.environ)
        env.pop('DIARY_FAULT', None)
        env.pop('DIARY_FAKE_EXPORT_DIR', None)
        env.pop('DIARY_FAKE_NOTION_FILE', None)
        if notion:
            env['DIARY_FAKE_NOTION_FILE'] = str(notion)
        if destination:
            env['DIARY_FAKE_EXPORT_DIR'] = str(remote)
        if fault:
            env['DIARY_FAULT'] = fault
        process = subprocess.Popen([str(binary), '--config', str(cfg or config)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
        for _ in range(300):
            for found in runtime.glob('*/discovery.json'):
                record = json.loads(found.read_text())
                if record['pid'] == process.pid:
                    return process, record
            assert process.poll() is None, 'server exited before readiness'
            time.sleep(0.02)
        raise AssertionError('server readiness timeout')

    class Client:
        def __init__(self, record):
            self.record = record
            self.session = self.raw('/sessions', 'POST', {})[1]['session_id']

        def raw(self, path, method='GET', body=None, session=None):
            headers = {'Authorization': 'Bearer ' + self.record['token'], 'Content-Type': 'application/json'}
            if session:
                headers['X-Diary-Session'] = session
            request = urllib.request.Request(self.record['origin'] + '/api/v1' + path, data=None if body is None else json.dumps(body).encode(), headers=headers, method=method)
            try:
                with opener.open(request, timeout=10) as response:
                    data = response.read()
                    return response.status, json.loads(data) if data else None
            except urllib.error.HTTPError as error:
                return error.code, json.loads(error.read() or b'null')
            except (urllib.error.URLError, ConnectionError):
                return 0, None

        def call(self, path, method='GET', body=None):
            return self.raw(path, method, body, self.session)

        def save(self, body):
            code, loaded = self.call('/authentic/' + DATE)
            assert code == 200, (code, loaded)
            code, saved = self.call('/authentic/' + DATE, 'PUT', {'operation_id': OP(), 'expected_revision': loaded['revision'], 'body': body, 'buffer_generation': 1})
            assert code == 200, (code, saved)
            return self.call('/authentic/' + DATE)[1]['revision']

        def export(self, revision, op=None, target='icloud'):
            op = op or OP()
            code, out = self.call('/exports', 'POST', {'operation_id': op, 'date': DATE, 'target': target, 'expected_revision': revision})
            return op, code, out

        def wait(self, op, state):
            for _ in range(500):
                code, found = self.call('/operations/' + op)
                assert code == 200, (code, found)
                if found['export']['state'] == state:
                    return found
                time.sleep(0.02)
            raise AssertionError(f'{op} never reached {state}: {found}')

    def stop(process):
        process.send_signal(signal.SIGINT)
        process.wait(timeout=10)

    def confirm(c, op, preview, step=None):
        step = step or OP()
        code, ack = c.call(f'/exports/{op}/confirmation', 'POST', {'step_id': step, 'expected_checkpoint': preview['expected_checkpoint'], 'token': preview['token']})
        return step, code, ack

    # 0. Without a destination every target is disabled.
    server, record = start(destination=False)
    c = Client(record)
    rev = c.save('Smoke one.\n')
    op, code, out = c.export(rev)
    assert code == 422 and out['error']['code'] == 'export_disabled', (code, out)
    stop(server)

    server, record = start()
    c = Client(record)
    # 1. First create, then a verified no-op that writes nothing.
    op1, code, out = c.export(rev)
    assert code == 200 and out['outcome'] in ('running', 'succeeded'), (code, out)
    done = c.wait(op1, 'succeeded')
    assert done['kind'] == 'export' and done['succeeded'] is True
    assert 'Smoke one.' in file.read_text()
    stamp = file.stat().st_mtime_ns
    op2, code, out = c.export(rev)
    assert code == 200 and out['outcome'] == 'succeeded' and file.stat().st_mtime_ns == stamp, (code, out)
    # A repeat of the same ID returns the recorded result; changed inputs are a mismatch.
    assert c.export(rev, op1)[1:] == (200, c.export(rev, op1)[2])
    rev2 = c.save('Smoke two.\n')
    assert c.export(rev2, op1)[1] == 409
    # 2. An external change needs confirmation; nothing is overwritten before it.
    file.write_text('edited outside')
    op3, code, preview = c.export(rev2)
    assert code == 200 and preview['outcome'] == 'needs_confirmation' and preview['reason'] == 'changed', (code, preview)
    assert file.read_text() == 'edited outside'
    assert c.call(f'/operations/{op3}')[1]['state'] == 'paused'
    listed = c.call('/operations?kind=export&state=unfinished')[1]
    assert [o['operation_id'] for o in listed['operations']] == [op3], listed
    # A stale checkpoint is refused; the real step is admitted once and a duplicate returns it.
    bad = dict(preview, expected_checkpoint=9)
    assert confirm(c, op3, bad)[1] == 409
    step, code, ack = confirm(c, op3, preview)
    assert code == 200 and ack['admitted_checkpoint'] == preview['expected_checkpoint'] + 1, (code, ack)
    assert confirm(c, op3, preview, step)[1:] == (200, ack)
    c.wait(op3, 'succeeded')
    assert 'Smoke two.' in file.read_text()
    # A finished export cannot be cancelled again; the call reports the recorded job.
    assert c.call(f'/jobs/{op3}/cancel', 'POST', {})[0] == 200
    stop(server)

    # 3. A kill after the destination write: restart proves it by read-back, never a second write.
    server, record = start(fault='export:after_apply:kill')
    c = Client(record)
    rev3 = c.save('Smoke three.\n')
    op4, code, out = c.export(rev3)
    server.wait(timeout=10)
    assert server.returncode == 86, server.returncode
    assert 'Smoke three.' in file.read_text()
    server, record = start()
    c = Client(record)
    found = c.wait(op4, 'succeeded')
    assert found['export']['reason'] is None
    assert c.call('/operations?kind=export&state=unfinished')[1]['operations'] == []
    stop(server)

    # 4. A kill before the destination write is unprovable: uncertain, and it reserves the destination.
    server, record = start(fault='export:before_apply:kill')
    c = Client(record)
    rev4 = c.save('Smoke four.\n')
    op5, code, out = c.export(rev4)
    server.wait(timeout=10)
    assert server.returncode == 86 and 'Smoke four.' not in file.read_text()
    server, record = start()
    c = Client(record)
    assert c.call(f'/operations/{op5}')[1]['export']['state'] == 'uncertain'
    op6, code, out = c.export(c.save('Smoke five.\n'))
    assert code == 409 and out['error']['code'] == 'outcome_uncertain', (code, out)
    stop(server)

    # 5. Real macOS adapter on a temp directory: create, external edit, cancel, abandon.
    icloud = base / 'icloud'
    icloud.mkdir(mode=0o700)
    real_cfg = base / 'real.yaml'
    real_cfg.write_text(f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\n  icloud_entries_root: {icloud}\n')
    real_cfg.chmod(0o600)
    target_file = icloud.resolve() / DATE[:4] / DATE[5:7] / f'{DATE}.md'
    server, record = start(destination=False, cfg=real_cfg)
    c = Client(record)
    revr = c.save('Real one.\n')
    opr, code, out = c.export(revr)
    assert code == 200, (code, out)
    c.wait(opr, 'succeeded')
    assert target_file.read_bytes() == next(workspace.rglob(f'{DATE}.md')).read_bytes(), 'byte-identical export'
    assert not list(target_file.parent.glob('.*.stage')), 'no staged leftovers'
    # The real adapter verifies no-op without republishing and confirms external deletion.
    stamp = target_file.stat().st_mtime_ns
    opn, code, out = c.export(revr)
    assert code == 200, (code, out)
    c.wait(opn, 'succeeded')
    assert target_file.stat().st_mtime_ns == stamp, 'no-op republished the file'
    target_file.unlink()
    opd, code, pv = c.export(revr)
    assert code == 200 and pv['outcome'] == 'needs_confirmation' and pv['reason'] == 'deleted' and pv['write'] == 'create', (code, pv)
    confirm(c, opd, pv)
    c.wait(opd, 'succeeded')
    assert target_file.read_bytes() == next(workspace.rglob(f'{DATE}.md')).read_bytes(), 'recreated exact bytes'
    # An external edit needs confirmation; declining cancels and never touches the file.
    target_file.write_text('edited outside')
    revr2 = c.save('Real two.\n')
    opx, code, pv = c.export(revr2)
    assert code == 200 and pv['outcome'] == 'needs_confirmation' and pv['reason'] == 'changed', (code, pv)
    code, job = c.call(f'/jobs/{opx}/cancel', 'POST', {})
    assert code == 200, (code, job)
    assert c.call(f'/operations/{opx}')[1]['export']['state'] == 'cancelled'
    assert target_file.read_text() == 'edited outside'
    assert opx not in [o['operation_id'] for o in c.call('/operations?kind=export&state=unfinished')[1]['operations']]
    stop(server)
    # A kill before the write leaves an uncertain export; abandon needs a one-use confirmation.
    server, record = start(fault='export:before_apply:kill', destination=False, cfg=real_cfg)
    c = Client(record)
    opu, code, out = c.export(c.save('Real three.\n'))
    assert code == 200 and out['outcome'] == 'needs_confirmation', (code, out)
    confirm(c, opu, out)
    server.wait(timeout=10)
    assert server.returncode == 86 and target_file.read_text() == 'edited outside', server.returncode
    server, record = start(destination=False, cfg=real_cfg)
    c = Client(record)
    state = c.call(f'/operations/{opu}')[1]['export']['state']
    assert state == 'uncertain', state
    code, pv = c.call(f'/exports/{opu}/abandon', 'POST', {})
    assert code == 200 and pv['outcome'] == 'needs_confirmation' and pv['reason'] == 'uncertain', (code, pv)
    step = OP()
    body = {'step_id': step, 'expected_checkpoint': pv['expected_checkpoint'], 'token': pv['token']}
    code, done = c.call(f'/exports/{opu}/abandon/confirmation', 'POST', body)
    assert code == 200 and done['state'] == 'cancelled' and done['reason'] == 'abandoned', (code, done)
    assert c.call(f'/exports/{opu}/abandon/confirmation', 'POST', body)[0] == 200
    assert opu not in [o['operation_id'] for o in c.call('/operations?kind=export&state=unfinished')[1]['operations']]
    assert target_file.read_text() == 'edited outside'
    stop(server)

    # 6. Notion over the file-backed fake: hierarchy, no-op, external edit, kill and recovery.
    page = '0123456789abcdef0123456789abcdef'
    notion_cfg = base / 'notion.yaml'
    notion_cfg.write_text(f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\nnotion:\n  root_page_id: {page}\n  token: smoke-notion-token\n')
    notion_cfg.chmod(0o600)
    remote_state = base / 'notion-state.json'

    def nodes():
        return json.loads(remote_state.read_text())['nodes']

    def section_texts():
        found = nodes()
        toggles = [k for k, n in found.items() if n['kind'] == 'toggle' and ''.join(i['plain_text'] for i in n['payload']['rich_text']) == DATE]
        assert len(toggles) == 1, toggles
        return toggles[0], [''.join(i['plain_text'] for i in found[c]['payload']['rich_text']) for c in found[toggles[0]]['children']]

    server, record = start(destination=False, cfg=notion_cfg, notion=remote_state)
    c = Client(record)
    rev_n = c.save('Notion one.\n\nNotion two.\n')
    opn1, code, out = c.export(rev_n, target='notion')
    assert code == 200, (code, out)
    c.wait(opn1, 'succeeded')
    titles = sorted(n['title'] for n in nodes().values() if n['kind'] == 'child_page')
    assert titles == [DATE[:4], DATE[:7]], titles
    assert section_texts()[1] == ['Notion one.', 'Notion two.']
    stamp = remote_state.stat().st_mtime_ns
    opn2, code, out = c.export(rev_n, target='notion')
    c.wait(opn2, 'succeeded')
    assert remote_state.stat().st_mtime_ns == stamp, 'no-op wrote to Notion'
    # iCloud stays disabled when only Notion is configured.
    assert c.export(rev_n)[1] == 422
    stop(server)
    # An edit made in Notion needs confirmation; the replacement is a new section after the old.
    state = json.loads(remote_state.read_text())
    toggle_id, _ = section_texts()
    first = state['nodes'][toggle_id]['children'][0]
    state['nodes'][first]['payload']['rich_text'] = [{'type': 'text', 'text': {'content': 'edited in Notion'}, 'plain_text': 'edited in Notion'}]
    remote_state.write_text(json.dumps(state))
    server, record = start(destination=False, cfg=notion_cfg, notion=remote_state)
    c = Client(record)
    rev_n2 = c.save('Notion three.\n')
    opn3, code, pv = c.export(rev_n2, target='notion')
    assert code == 200 and pv['outcome'] == 'needs_confirmation' and pv['reason'] == 'changed', (code, pv)
    assert section_texts()[1][0] == 'edited in Notion'
    confirm(c, opn3, pv)
    c.wait(opn3, 'succeeded')
    new_toggle, texts = section_texts()
    assert texts == ['Notion three.'] and new_toggle != toggle_id, 'replace inserts a new section (O10)'
    toggle_id = new_toggle
    stop(server)
    # A kill right after the Notion write is proven by read-back on restart; nothing is replayed.
    server, record = start(fault='export:after_apply:kill', destination=False, cfg=notion_cfg, notion=remote_state)
    c = Client(record)
    rev_n3 = c.save('Notion four.\n')
    opn4, code, out = c.export(rev_n3, target='notion')
    if code == 200 and out['outcome'] == 'needs_confirmation':
        confirm(c, opn4, out)
    server.wait(timeout=10)
    assert server.returncode == 86, server.returncode
    assert section_texts()[1] == ['Notion four.']
    server, record = start(destination=False, cfg=notion_cfg, notion=remote_state)
    c = Client(record)
    c.wait(opn4, 'succeeded')
    assert opn4 not in [o['operation_id'] for o in c.call('/operations?kind=export&state=unfinished')[1]['operations']]
    stop(server)

    logs = ''.join(p.read_text() for p in runtime.glob('*/logs/server/*.log'))
    assert 'Smoke one' not in logs and str(remote) not in logs and str(icloud) not in logs and 'private' not in logs and 'smoke-notion-token' not in logs and 'Notion one' not in logs
    print('PASS export smoke')
