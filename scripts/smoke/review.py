"""P1-M24 paragraph review on the built diary-server: start, reject, durable accept, restart
recovery by operation ID, invalidation and the discard/regeneration guards.
Only the fake CLI runs inference-shaped fixtures; uses synthetic data and a private scratch dir."""
import json
import os
from pathlib import Path
import shlex
import signal
import stat
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid

root = Path(__file__).resolve().parents[2]
manifest_path = str(root / 'src-tauri/Cargo.toml')
target = root / 'src-tauri/target/fault'  # a separate fault-injection binary, like the authentic smoke
subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', manifest_path, '-p', 'diary-server', '--features', 'fault-injection', '--target-dir', str(target)], check=True)
subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', manifest_path, '-p', 'diary-infrastructure', '--example', 'fake_cli', '--target-dir', str(target)], check=True)
binary = target / 'debug/diary-server'
fake_binary = target / 'debug/examples/fake_cli'
DATE = '2026-11-06'
CANDIDATE = 'Tea was nice.'
OP = lambda: str(uuid.uuid4())

with tempfile.TemporaryDirectory(prefix='diary-review-smoke-') as temporary:
    base = Path(temporary).resolve()
    workspace, runtime, source, home = base / 'workspace', base / 'runtime', base / 'source', base / 'codex-home'
    for folder in [base, workspace, runtime, source, home]:
        folder.mkdir(exist_ok=True, mode=0o700)
        folder.chmod(0o700)
    fake = base / 'codex'
    fake.write_text('#!/bin/sh\nexec ' + shlex.quote(str(fake_binary)) + ' "$@"\n')
    fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
    file = source / '2026/11/06/2026-11-06_00-00-00-123.md'
    file.parent.mkdir(parents=True)
    file.write_text('---\nid: "2026-11-06_00-00-00-123"\ncreated_at: "2026-11-06T00:00:00.123Z"\ntags: []\n---\nSYNTHETIC_REVIEW_SOURCE. I had tea.\n')
    config = base / 'config.yaml'
    config.write_text(
        f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\n'
        f'sources:\n  - {{alias: smoke-readable, type: ayumi, root: {source}}}\n'
        f'agents:\n  profiles:\n    - {{alias: main, type: codex, profile_home: {home}, executable: {fake}}}\n')
    config.chmod(0o600)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    processes = []

    def start(fault=None):
        env = dict(os.environ)
        if fault:
            env['DIARY_FAULT'] = fault
        process = subprocess.Popen([str(binary), '--config', str(config)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
        processes.append(process)
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
            # POST and DELETE always carry a JSON body, as the routes expect.
            return self.raw(path, method, body if body is not None or method == 'GET' else {}, self.session) if method != 'GET' else self.raw(path, method, None, self.session)

        def wait(self, job_id):
            for _ in range(1000):
                code, job = self.call('/jobs/' + job_id)
                assert code == 200, (code, job)
                if job['state'] in ('succeeded', 'failed', 'cancelled'):
                    return job
                self.call('/sessions/' + self.session + '/heartbeat', 'POST', {})
                time.sleep(0.02)
            raise AssertionError('timeout')

        def candidate(self, tag):
            code, imported = self.call('/imports', 'POST', {'date': DATE, 'request_id': 'imp-' + tag})
            assert code == 202, (code, imported)
            assert self.wait(imported['import_id'])['state'] == 'succeeded'
            code, page = self.call('/sources?import_id=' + imported['import_id'])
            selected = [{'import_id': imported['import_id'], 'source_id': r['source']['id']} for r in page['items'] if r['kind'] == 'included']
            code, accepted = self.call('/generations', 'POST', {'request_id': 'gen-' + tag, 'date': DATE, 'profile': 'main', 'locale': 'en-US', 'count': 1, 'selections': selected})
            assert code == 202, (code, accepted)
            job = accepted['job']['id']
            assert self.wait(job)['state'] == 'succeeded'
            return job, job + '-1'

        def set_body(self, body):
            code, loaded = self.call('/authentic/' + DATE)
            assert code == 200, (code, loaded)
            code, saved = self.call('/authentic/' + DATE, 'PUT', {'operation_id': OP(), 'expected_revision': loaded['revision'], 'body': body, 'buffer_generation': 1})
            assert code == 200, (code, saved)

        def review(self, review_id, candidate):
            return self.call('/reviews', 'POST', {'review_id': review_id, 'date': DATE, 'candidate_id': candidate})

        def decide(self, review_id, view, hunk, action, op=None):
            return self.call(f'/reviews/{review_id}/decisions', 'POST', {'operation_id': op or OP(), 'hunk_id': hunk, 'action': action, 'expected_revision': view['revision'], 'expected_set_digest': view['set_digest']})

    def stop(process):
        process.send_signal(signal.SIGINT)
        process.wait(timeout=10)

    try:
        # 1. Absent baseline: start writes nothing; reject writes nothing; accept is one durable save.
        server, record = start()
        c = Client(record)
        job, cand = c.candidate('one')
        rid = OP()
        code, view = c.review(rid, cand)
        assert code == 201 and view['body'] == '' and [h['kind'] for h in view['hunks']] == ['insert'], (code, view)
        assert c.call('/authentic/' + DATE)[1]['exists'] is False, 'starting a review wrote a file'
        # Guards while the review is live.
        assert c.call(f'/jobs/{job}/result', 'DELETE', {})[1]['error']['code'] == 'review_active'
        assert c.call(f'/generations/{job}', 'DELETE', {})[1]['error']['code'] == 'review_active'
        code, refused = c.call('/generations', 'POST', {'request_id': 'regen', 'date': DATE, 'profile': 'main', 'locale': 'en-US', 'count': 1, 'selections': [{'import_id': 'x', 'source_id': 'y'}]})
        assert code == 409 and refused['error']['code'] == 'review_active', (code, refused)
        hunk = view['hunks'][0]['id']
        code, rejected = c.decide(rid, view, hunk, 'reject')
        assert code == 200 and rejected['view']['hunks'] == [] and rejected['receipt'] is None, (code, rejected)
        assert c.call('/authentic/' + DATE)[1]['exists'] is False, 'reject wrote a file'
        assert c.call(f'/reviews/{rid}', 'DELETE', {})[0] == 200 and c.call(f'/reviews/{rid}', 'DELETE', {})[0] == 200
        rid = OP()
        code, view = c.review(rid, cand)
        assert code == 201
        op_a = OP()
        code, done = c.decide(rid, view, view['hunks'][0]['id'], 'accept', op_a)
        assert code == 200 and done['view']['body'] == CANDIDATE and done['view']['hunks'] == [] and done['receipt'], (code, done)
        assert done['receipt']['before'] == view['set_digest']
        loaded = c.call('/authentic/' + DATE)[1]
        assert loaded['body'] == CANDIDATE and loaded['revision'] == done['view']['revision'], loaded
        # A repeated accept (lost reply) returns the receipt and does not save again.
        code, again = c.decide(rid, view, view['hunks'][0]['id'], 'accept', op_a)
        assert code == 200 and again['receipt']['revision'] == done['receipt']['revision'], (code, again)
        assert c.call('/authentic/' + DATE)[1]['revision'] == loaded['revision']
        assert c.call(f'/reviews/{rid}', 'DELETE', {})[0] == 200
        assert c.call(f'/generations/{job}', 'DELETE', {})[0] == 200
        # 2. Invalidation: an unrelated save drops the remaining suggestions, never rebases.
        c.set_body('Hello.\n\n')
        job, cand = c.candidate('two')
        rid = OP()
        code, view = c.review(rid, cand)
        assert code == 201 and [h['kind'] for h in view['hunks']] == ['replace'], (code, view)
        c.set_body('Changed elsewhere.\n\n')
        code, bad = c.decide(rid, view, view['hunks'][0]['id'], 'accept')
        assert code == 409 and bad['error']['code'] == 'review_invalidated', (code, bad)
        assert c.call('/authentic/' + DATE)[1]['body'] == 'Changed elsewhere.\n\n'
        assert c.call(f'/reviews/{rid}', 'DELETE', {})[0] == 200
        assert c.call(f'/generations/{job}', 'DELETE', {})[0] == 200
        stop(server)

        # 3. A kill between publication and receipt: the saved body survives and the original
        # operation ID answers with its receipt after restart; the review itself is gone.
        server, record = start()
        Client(record).set_body('Hello.\n\n')
        stop(server)
        server, record = start(fault='receipt:before:kill')
        c = Client(record)
        job, cand = c.candidate('three')
        rid = OP()
        code, view = c.review(rid, cand)
        assert code == 201 and view['hunks'], (code, view)
        op_b = OP()
        code, out = c.decide(rid, view, view['hunks'][0]['id'], 'accept', op_b)
        assert code == 0
        server.wait(timeout=10)
        assert server.returncode == -signal.SIGKILL, server.returncode
        server, record = start()
        c = Client(record)
        loaded = c.call('/authentic/' + DATE)[1]
        assert loaded['body'] == CANDIDATE, loaded
        code, replay = c.decide(rid, view, view['hunks'][0]['id'], 'accept', op_b)
        assert code == 200 and replay['view'] is None and replay['receipt']['revision'] == loaded['revision'], (code, replay)
        code, op = c.call(f'/operations/{op_b}')
        assert code == 200 and op['succeeded'] is True, (code, op)
        stop(server)
        logs = ''.join(p.read_text() for p in runtime.glob('*/logs/server/*.log'))
        assert 'SYNTHETIC_REVIEW_SOURCE' not in logs and CANDIDATE not in logs and str(workspace) not in logs
        print('PASS review smoke')
    finally:
        for process in processes:
            if process.poll() is None:
                process.kill()
