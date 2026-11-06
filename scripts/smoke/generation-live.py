"""P1-M22 env-gated live workflow proof against the built diary-server binary.

DIARY_GENERATION_LIVE=codex|claude|qwen|apple
DIARY_GENERATION_LIVE_HOME=<approved profile_home>  # required for CLIs

One candidate and one real spawned-run cancellation. Uses synthetic Ayumi snippets,
private scratch storage, real import/capture/routes/adapters, and only Python stdlib.
CLI runs spend account usage. No prompt, pin, argv or profile configuration is changed.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import uuid

kind = os.environ.get('DIARY_GENERATION_LIVE')
if kind not in ('codex', 'claude', 'qwen', 'apple'):
    print('generation live: skipped (set DIARY_GENERATION_LIVE)')
    raise SystemExit(0)
home = os.environ.get('DIARY_GENERATION_LIVE_HOME')
assert kind == 'apple' or (home and Path(home).is_absolute()), 'an absolute approved profile home is required'
root = Path(__file__).resolve().parents[2]
subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', str(root / 'src-tauri/Cargo.toml'), '-p', 'diary-server'], check=True)
binary = Path(os.environ.get('CARGO_TARGET_DIR', root / 'src-tauri/target')) / 'debug/diary-server'
version = None
if kind != 'apple':
    executable = shutil.which(kind)
    assert executable, 'live executable missing'
    probe = subprocess.run([executable, '--version'], capture_output=True, text=True, timeout=20)
    match = re.search(r'\d+\.\d+\.\d+', probe.stdout)
    assert probe.returncode == 0 and match, 'live version probe failed'
    version = match.group()
os_version = subprocess.check_output(['sw_vers', '-productVersion'], text=True).strip()
sdk = subprocess.check_output(['xcrun', '--sdk', 'macosx', '--show-sdk-version'], text=True).strip()
DATE = '2026-11-06'
SECRET = 'SYNTHETIC_GENERATION_LIVE_SOURCE'
# Factual synthetic input, not app-authored instructions. The existing prompt asset is unchanged.
BODY = SECRET + '''
At nine I brewed a small pot of green tea and opened the kitchen window.
The air was cool. I watered the basil on the sill and noticed two new leaves.
At ten I walked to the library and returned a novel. I borrowed a book about birds.
I sat at the park for a while and watched three sparrows near the bench.
At noon I made rice and an omelette. I washed the dishes before reading a chapter.
In the afternoon I called my sister and we talked about our weekend plans.
I went for a short walk at sunset. After dinner I listened to quiet piano music.
I put tomorrow's clothes on the chair and felt glad to have had a calm day.
'''
locale = 'ko-KR' if kind == 'apple' else 'en-US'

def descendant_runs(server_pid):
    """Inspect only our server's descendants; never print unrelated process arguments."""
    process_table = subprocess.run(['ps', '-axo', 'pid=,ppid=,command='], capture_output=True, text=True, check=True).stdout
    rows = []
    for line in process_table.splitlines():
        parts = line.strip().split(None, 2)
        if len(parts) == 3:
            try:
                rows.append((int(parts[0]), int(parts[1]), parts[2]))
            except ValueError:
                pass
    descendants = {server_pid}
    while True:
        more = {pid for pid, parent, _ in rows if parent in descendants}
        if more <= descendants:
            break
        descendants |= more
    def inference(command):
        if kind == 'apple':
            return 'apple-fm-' in command and command.rstrip().endswith(' run')
        # Protocol-start progress below distinguishes the actual run from status probes.
        # CLIs can replace their process title, so do not depend on their argv being visible.
        return True

    return [pid for pid, _, command in rows if pid != server_pid and pid in descendants and inference(command)]

with tempfile.TemporaryDirectory(prefix='diary-generation-live-') as temporary:
    base = Path(temporary).resolve()
    workspace, runtime, source = base / 'workspace', base / 'runtime', base / 'source'
    for folder in [base, workspace, runtime, source]:
        folder.mkdir(exist_ok=True, mode=0o700)
        folder.chmod(0o700)
    snippet = source / '2026/11/06/2026-11-06_00-00-00-123.md'
    snippet.parent.mkdir(parents=True)
    original = ('---\nid: "2026-11-06_00-00-00-123"\ncreated_at: "2026-11-06T00:00:00.123Z"\ntags: []\n---\n' + BODY).encode()
    snippet.write_bytes(original)
    profile_type = 'apple-foundation-models' if kind == 'apple' else kind
    profile = f'    - alias: live\n      type: {profile_type}\n'
    if kind != 'apple':
        profile += '      profile_home: ' + json.dumps(home) + '\n'
    config = base / 'config.yaml'
    config.write_text('schema_version: 1\npaths:\n  entries_root: ' + json.dumps(str(workspace)) + '\n  runtime_root: ' + json.dumps(str(runtime)) + '\nsources:\n  - alias: synthetic\n    type: ayumi\n    root: ' + json.dumps(str(source)) + '\nagents:\n  profiles:\n' + profile)
    config.chmod(0o600)
    # File-backed output cannot fill a pipe during a long Qwen generation. Only typed server
    # diagnostics are retained; private scratch output is removed with the rest of this fixture.
    with (base / 'server-output').open('w+') as output:
        process = subprocess.Popen([str(binary), '--config', str(config)], stdout=output, stderr=output)
        try:
            record = None
            for _ in range(500):
                for found in runtime.glob('*/discovery.json'):
                    candidate = json.loads(found.read_text())
                    if candidate['pid'] == process.pid:
                        record = candidate
                if record:
                    break
                assert process.poll() is None, 'server exited before readiness'
                time.sleep(0.02)
            assert record, 'server readiness timeout'
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
            session = None
            last_beat = time.monotonic()
            def call(path, method='GET', body=None):
                headers = {'Authorization': 'Bearer ' + record['token'], 'Content-Type': 'application/json'}
                if session:
                    headers['X-Diary-Session'] = session
                req = urllib.request.Request(record['origin'] + '/api/v1' + path, headers=headers, data=None if body is None else json.dumps(body).encode(), method=method)
                try:
                    with opener.open(req, timeout=25) as response:
                        data = response.read()
                        return response.status, json.loads(data) if data else None
                except urllib.error.HTTPError as error:
                    return error.code, json.loads(error.read() or b'null')
            def heartbeat():
                global last_beat
                if time.monotonic() - last_beat >= 4:
                    assert call('/sessions/' + session + '/heartbeat', 'POST', {})[0] == 204
                    last_beat = time.monotonic()
            def wait(job_id, limit=600):
                until = time.monotonic() + limit
                while time.monotonic() < until:
                    heartbeat()
                    status, job = call('/jobs/' + job_id)
                    assert status == 200, (status, job)
                    if job['state'] in ('succeeded', 'failed', 'cancelled'):
                        return job
                    time.sleep(0.1)
                raise AssertionError('live job timeout')
            status, opened = call('/sessions', 'POST', {})
            assert status == 200, (status, opened)
            session = opened['session_id']
            status, checked = call('/profiles/live/check', 'POST', {})
            assert status == 200 and checked['status'] == 'ready', (status, checked)
            heartbeat()
            assert not descendant_runs(process.pid), 'a status probe left a child behind'
            status, imported = call('/imports', 'POST', {'request_id': str(uuid.uuid4()), 'date': DATE})
            assert status == 202, (status, imported)
            import_id = imported['import_id']
            assert wait(import_id)['state'] == 'succeeded'
            status, page = call('/sources?import_id=' + import_id)
            assert status == 200, (status, page)
            selected = [{'import_id': import_id, 'source_id': row['source']['id']} for row in page['items'] if row['kind'] == 'included']
            assert len(selected) == 1, page
            def request():
                return {'request_id': str(uuid.uuid4()), 'date': DATE, 'profile': 'live', 'locale': locale, 'count': 1, 'selections': selected}
            started = time.monotonic()
            status, accepted = call('/generations', 'POST', request())
            assert status == 202, (status, accepted)
            job_id = accepted['job']['id']
            terminal = wait(job_id)
            assert terminal['state'] == 'succeeded', terminal
            candidate_seconds = time.monotonic() - started
            status, manifest = call('/generations/' + job_id)
            assert status == 200 and manifest['progress']['ready_count'] == 1, (status, manifest)
            assert SECRET not in json.dumps(manifest) and 'provisional' not in manifest
            candidate_id = manifest['candidates'][0]['id']
            status, candidate = call('/candidates/' + candidate_id)
            assert status == 200 and candidate['draft_markdown'].strip(), 'candidate read failed'
            assert all(s['snippet_ids'] and set(s['snippet_ids']) <= {'snippet-1'} for s in candidate['claim_support'])
            assert call('/generations/' + job_id, 'DELETE')[0] == 200
            heartbeat()
            status, accepted = call('/generations', 'POST', request())
            assert status == 202, (status, accepted)
            cancel_job = accepted['job']['id']
            # CLI protocol-start events prove an actual turn; the native helper exposes only
            # final generation output, so observe its supervised `run` child instead.
            started_child = False
            until = time.monotonic() + 90
            while time.monotonic() < until:
                heartbeat()
                _, progress = call('/jobs/' + cancel_job)
                assert progress['state'] not in ('succeeded', 'failed', 'cancelled'), 'live run finished before cancellation'
                pids = descendant_runs(process.pid)
                if pids and (kind == 'apple' or progress['phase'] in ('waiting_for_output', 'receiving_output')):
                    started_child = True
                    if kind == 'apple':
                        time.sleep(0.15)
                    break
                time.sleep(0.02)
            assert started_child, 'live inference child was not observed'
            cancel_started = time.monotonic()
            status, cancelling = call('/jobs/' + cancel_job + '/cancel', 'POST', {})
            assert status == 200 and cancelling['cancellation_requested'], (status, cancelling)
            terminal = wait(cancel_job, 30)
            cancel_seconds = time.monotonic() - cancel_started
            assert terminal['state'] == 'cancelled', terminal
            status, manifest = call('/generations/' + cancel_job)
            assert status == 200 and (manifest['progress']['ready_count'], manifest['progress']['cancelled_count']) == (0, 1), (status, manifest)
            assert not descendant_runs(process.pid), 'live child remains after cancellation acknowledgement'
            assert not list(runtime.glob('agent-runs/*')), 'run directory remains after acknowledgement'
            assert snippet.read_bytes() == original, 'generation changed its source'
            assert not list(workspace.rglob('*.md')), 'generation unexpectedly wrote an Entry'
            assert call('/sessions/' + session, 'DELETE')[0] == 204
            process.send_signal(signal.SIGINT)
            process.wait(timeout=15)
            assert process.returncode == 0
            lines = [json.loads(line) for log in runtime.glob('*/logs/server/*.log') for line in log.read_text().splitlines()]
            run_records = [line['agent'] for line in lines if line.get('agent', {}).get('action') == 'generate' and line['agent'].get('style')]
            assert len(run_records) == 2, 'expected exactly one model call per admitted candidate'
            assert any(line.get('stop') == 'cancelled' for line in run_records), 'no supervised cancellation record'
            logs = json.dumps(lines)
            assert SECRET not in logs and candidate['draft_markdown'] not in logs and str(base) not in logs, 'private content reached diagnostics'
            print(json.dumps({'kind': kind, 'version': version, 'os': os_version, 'sdk': sdk, 'locale': locale, 'zone': page['zone'], 'candidate_elapsed_s': round(candidate_seconds, 3), 'cancel_ack_s': round(cancel_seconds, 3), 'spawned_cancel': started_child, 'adapter_runs': len(run_records), 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest()}))
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGINT)
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
