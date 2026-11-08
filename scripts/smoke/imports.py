"""P1-M10 synthetic real-server import, resolution, restart and redaction smoke."""
from isolation import server_configuration
import json
import os
from pathlib import Path
import signal
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

root = Path(__file__).resolve().parents[2]
subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', str(root / 'src-tauri/Cargo.toml'), '-p', 'diary-server'], check=True)
binary = root / 'src-tauri/target/debug/diary-server'
DATE = '2024-02-29'
STEM = '2024-02-29_00-00-00-123'
SECRET = 'SYNTHETIC_IMPORT_BODY_SENTINEL'
with tempfile.TemporaryDirectory(prefix='diary-import-smoke-') as temporary:
    base = Path(temporary).resolve()
    workspace, runtime, source = base / 'workspace', base / 'runtime', base / 'source'
    for folder in [base, workspace, runtime, source]:
        folder.mkdir(exist_ok=True, mode=0o700)
        folder.chmod(0o700)
    config = base / 'config.yaml'
    config.write_text(server_configuration() + f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\nsources:\n  - {{alias: smoke, type: ayumi, root: {source}}}\n')
    config.chmod(0o600)
    file = source / f'2024/02/29/{STEM}.md'
    file.parent.mkdir(parents=True)
    original = f'---\nid: "{STEM}"\ncreated_at: "2024-02-29T00:00:01.123Z"\ntags: []\n---\n{SECRET}\n'.encode()
    file.write_bytes(original)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    processes = []

    def start():
        env = dict(os.environ)
        env.pop('DIARY_FAULT', None)
        process = subprocess.Popen([str(binary), '--config', str(config)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
        processes.append(process)
        for _ in range(300):
            for found in runtime.glob('*/discovery.json'):
                record = json.loads(found.read_text())
                if record['pid'] == process.pid:
                    return process, record
            if process.poll() is not None:
                raise AssertionError('server exited before readiness: ' + process.stderr.read())
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

    def import_date(record, session, request_id):
        code, ack = call(record, session, '/imports', 'POST', {'date': DATE, 'request_id': request_id})
        assert code == 202, (code, ack)
        import_id = ack['import_id']
        for _ in range(300):
            code, job = call(record, session, '/jobs/' + import_id)
            assert code == 200, (code, job)
            if job['state'] in ['succeeded', 'failed', 'cancelled']:
                assert job['state'] == 'succeeded', job
                return import_id
            time.sleep(0.01)
        raise AssertionError('import job timeout')

    try:
        server, record = start()
        session = session_for(record)
        code, settings = call(record, session, '/settings')
        assert code == 200 and settings['sources_available'], (code, settings)
        assert settings['sources'] == [{'alias': 'smoke', 'available': True, 'reason': 'ready'}], settings
        code, authentic = call(record, session, f'/authentic/{DATE}/empty', 'POST', {'operation_id': '00000000-0000-4000-8000-000000000001'})
        assert code == 200, (code, authentic)
        authentic_file = workspace / f'assets/diary/authentic/2024/02/{DATE}.md'
        authentic_bytes = authentic_file.read_bytes()
        import_id = import_date(record, session, 'first')
        code, page = call(record, session, '/sources?import_id=' + import_id)
        assert code == 200 and not page['incomplete'], (code, page)
        assert [i['kind'] for i in page['items']] == ['warning'], page
        observation = page['items'][0]['source']
        source_id = observation['id']
        assert observation['dating']['state'] == 'timestamp_conflict'
        code, ack = call(record, session, f'/sources/{source_id}/time-resolution', 'POST', {'import_id': import_id, 'digest': observation['digest'], 'authority': 'filename'})
        assert code == 200 and ack['resolved'], (code, ack)
        code, page = call(record, session, '/sources?import_id=' + import_id)
        assert code == 200 and [i['kind'] for i in page['items']] == ['warning'], (code, page)
        assert file.read_bytes() == original
        stop(server)
        server, record = start()
        session = session_for(record)
        code, page = call(record, session, '/sources?import_id=' + import_id)
        assert code == 200 and page['items'][0]['kind'] == 'warning', (code, page)
        second = import_date(record, session, 'second')
        code, page = call(record, session, '/sources?import_id=' + second)
        assert code == 200 and page['items'][0]['source']['id'] == source_id and page['items'][0]['kind'] == 'included'
        frozen = page
        file.write_bytes(original + b' ')
        code, page = call(record, session, '/sources?import_id=' + second)
        assert code == 200 and page == frozen, (code, page)
        third = import_date(record, session, 'third')
        code, refreshed = call(record, session, '/sources?import_id=' + third)
        assert code == 200 and [i['kind'] for i in refreshed['items']] == ['warning'], (code, refreshed)
        code, error = call(record, session, f'/sources/{source_id}/time-resolution', 'POST', {'import_id': second, 'digest': observation['digest'], 'authority': 'header'})
        assert code == 409 and error['error']['code'] == 'source_stale', (code, error)
        assert authentic_file.read_bytes() == authentic_bytes
        assert SECRET not in json.dumps(page) and str(source) not in json.dumps(page)
        stop(server)
        db = sqlite3.connect(workspace / '.diary/state.sqlite3')
        contexts = db.execute('SELECT import_id, zone, database_version, completed_at FROM import_contexts').fetchall()
        assert len(contexts) == 3 and all(row[1] and row[2] and row[3] for row in contexts)
        assert db.execute('SELECT count(*) FROM time_resolutions').fetchone()[0] == 1
        payloads = db.execute('SELECT payload FROM import_snapshots').fetchall()
        assert len(payloads) == 3 and all(SECRET.encode() not in row[0] and str(source).encode() not in row[0] for row in payloads)
        db.close()
        logs = ''.join(p.read_text() for p in runtime.glob('*/logs/server/*.log'))
        assert logs and SECRET not in logs and str(source) not in logs and str(workspace) not in logs
        print('PASS civil-date imports native smoke')
    finally:
        for process in processes:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
