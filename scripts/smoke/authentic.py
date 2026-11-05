"""P1-M6/P1-M7 native smoke: create, save, restart, reload, watcher event and recover a killed save
through the real diary-server binary. Builds a separate fault-injection binary; synthetic content only."""
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

root = Path(__file__).resolve().parents[2]
target = root / 'src-tauri/target/fault'
subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', str(root / 'src-tauri/Cargo.toml'), '-p', 'diary-server', '--features', 'fault-injection', '--target-dir', str(target)], check=True)
binary = target / 'debug/diary-server'
DATE = '2026-11-05'
SECRET = 'synthetic authentic sentinel body'
OP = lambda n: f'00000000-0000-4000-8000-{n:012x}'

with tempfile.TemporaryDirectory(prefix='diary-authentic-smoke-') as temporary:
    base = Path(temporary).resolve()
    workspace, runtime, source = base / 'workspace', base / 'runtime', base / 'source'
    for folder in [base, workspace, runtime, source]:
        folder.mkdir(exist_ok=True, mode=0o700)
        folder.chmod(0o700)
    config = base / 'config.yaml'
    config.write_text(f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\nsources:\n  - {{alias: smoke-readable, type: narihito, root: {source}}}\n')
    config.chmod(0o600)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    processes = []

    def start(fault=None):
        env = dict(os.environ)
        env.pop('DIARY_FAULT', None)
        if fault:
            env['DIARY_FAULT'] = fault
        before = {p for p in runtime.glob('*/discovery.json')}
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

    def call(record, session, path, method='GET', payload=None):
        headers = {'Authorization': 'Bearer ' + record['token'], 'X-Diary-Session': session}
        if payload is not None:
            headers['Content-Type'] = 'application/json'
        request = urllib.request.Request(record['origin'] + '/api/v1' + path, data=None if payload is None else json.dumps(payload).encode(), headers=headers, method=method)
        try:
            with opener.open(request, timeout=5) as response:
                data = response.read()
                return response.status, json.loads(data) if data else None
        except urllib.error.HTTPError as error:
            return error.code, json.loads(error.read() or b'null')
        except (urllib.error.URLError, ConnectionError):
            return 0, None

    def session_for(record):
        request = urllib.request.Request(record['origin'] + '/api/v1/sessions', data=b'{}', headers={'Authorization': 'Bearer ' + record['token'], 'Content-Type': 'application/json'}, method='POST')
        with opener.open(request, timeout=5) as response:
            return json.loads(response.read())['session_id']

    def stop(process):
        process.send_signal(signal.SIGINT)
        process.wait(timeout=5)
        assert process.returncode == 0

    try:
        server, record = start()
        session = session_for(record)
        code, created = call(record, session, f'/authentic/{DATE}/empty', 'POST', {'operation_id': OP(1)})
        assert code == 200 and created['revision']['sequence'] == 1, (code, created)
        assert call(record, session, f'/authentic/{DATE}/empty', 'POST', {'operation_id': OP(2)})[0] == 409
        code, saved = call(record, session, f'/authentic/{DATE}', 'PUT', {'operation_id': OP(3), 'expected_revision': created['revision'], 'body': SECRET, 'buffer_generation': 4})
        assert code == 200 and saved['revision']['sequence'] == 2 and saved['buffer_generation'] == 4, (code, saved)
        stop(server)

        server, record = start()
        session = session_for(record)
        code, loaded = call(record, session, f'/authentic/{DATE}')
        assert code == 200 and loaded['body'] == SECRET and loaded['revision'] == saved['revision'], (code, loaded)
        code, listing = call(record, session, '/authentic')
        assert code == 200 and [i['date'] for i in listing['items']] == [DATE]
        code, op = call(record, session, f'/operations/{OP(3)}')
        assert code == 200 and op['succeeded'] is True, (code, op)

        # An external touch of the file becomes one path-free invalidation event (watcher hint).
        stream = urllib.request.Request(record['origin'] + '/api/v1/events', headers={'Authorization': 'Bearer ' + record['token'], 'X-Diary-Session': session})
        with opener.open(stream, timeout=5) as events:
            time.sleep(0.5)  # let FSEvents register before the change
            file = workspace / f'assets/diary/authentic/2026/11/{DATE}.md'
            file.write_bytes(file.read_bytes())
            deadline = time.monotonic() + 5
            frame = ''
            while 'authentic_changed' not in frame:
                assert time.monotonic() < deadline, 'no authentic_changed event'
                frame += events.readline().decode()
            assert 'data: {}' in events.readline().decode() + events.readline().decode()
            assert str(workspace) not in frame and DATE not in frame
        stop(server)

        # A save killed after publication and before its receipt is reconciled by a retry of its ID.
        server, record = start(fault='receipt:before:kill')
        session = session_for(record)
        code, _ = call(record, session, f'/authentic/{DATE}', 'PUT', {'operation_id': OP(4), 'expected_revision': saved['revision'], 'body': SECRET + ' two', 'buffer_generation': 5})
        assert code == 0
        server.wait(timeout=5)
        assert server.returncode == -signal.SIGKILL, server.returncode
        assert (SECRET + ' two') in (workspace / f'assets/diary/authentic/2026/11/{DATE}.md').read_text(), 'published before the kill'
        server, record = start()
        session = session_for(record)
        code, op = call(record, session, f'/operations/{OP(4)}')
        assert code == 200 and op['succeeded'] is True, (code, op)  # startup recovery settled it
        code, retried = call(record, session, f'/authentic/{DATE}', 'PUT', {'operation_id': OP(4), 'expected_revision': saved['revision'], 'body': SECRET + ' two', 'buffer_generation': 6})
        assert code == 200 and retried['revision']['sequence'] == 3 and retried['buffer_generation'] == 6, (code, retried)
        assert not list((workspace / '.diary/pending').iterdir()), 'stage left behind'
        month = workspace / 'assets/diary/authentic/2026/11'
        assert [p.name for p in month.iterdir()] == [f'{DATE}.md']
        assert call(record, session, f'/operations/{OP(99)}')[0] == 404
        stop(server)

        # Logs carry no diary text and no workspace path.
        logs = ''.join(p.read_text() for p in runtime.glob('*/logs/server/*.log'))
        assert logs and SECRET not in logs and str(workspace) not in logs
        codes = {json.loads(l)['code'] for l in logs.splitlines()}
        assert {'authentic.created', 'authentic.saved', 'authentic.recovered', 'authentic.rebuilt'} <= codes, codes
        print('PASS authentic native smoke')
    finally:
        for process in processes:
            if process.poll() is None:
                process.kill()
