"""Normal-binary HTTP integration after the separate native enablement review."""
import hashlib
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


def verify_server(root, producer, worker, audio, member):
    subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', str(root / 'src-tauri/Cargo.toml'),
                    '-p', 'diary-server'], check=True)
    binary = root / 'src-tauri/target/debug/diary-server'
    with tempfile.TemporaryDirectory(prefix='diary-trash-smoke-') as temporary:
        base = Path(temporary).resolve()
        base.chmod(0o700)
        (base / 'fixture-marker').write_text('synthetic-source-trash-v1')
        source, workspace, runtime = (base / p for p in ['source', 'workspace', 'runtime'])
        for p in [source, workspace, runtime]:
            p.mkdir(mode=0o700)
        relative, md, recording, assets = member(source, str(uuid.uuid4()), 'target\n', audio)
        neighbor_id = str(uuid.uuid4())
        _, neighbor, neighbor_audio, _ = member(source, neighbor_id, 'neighbor\n', audio)
        preserved = {p: hashlib.sha256(p.read_bytes()).digest() for p in [neighbor, neighbor_audio]}
        (base / 'target-relative').write_text(relative)
        config = base / 'config.yaml'
        config.write_text(f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\nsources:\n  - {{alias: fixture, type: narihito, root: {source}}}\n')
        config.chmod(0o600)
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        process = None
        session = None
        record = None

        def start():
            nonlocal process, session, record
            env = dict(os.environ)
            env.pop('DIARY_FAULT', None)
            process = subprocess.Popen([str(binary), '--config', str(config)], env=env,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            for _ in range(500):
                for p in runtime.glob('*/discovery.json'):
                    candidate = json.loads(p.read_text())
                    if candidate['pid'] == process.pid:
                        record = candidate
                        code, value = call('/sessions', 'POST', {})
                        assert code == 200, (code, value)
                        session = value['session_id']
                        return
                assert process.poll() is None, process.stderr.read()
                time.sleep(0.02)
            raise AssertionError('server readiness deadline')

        def stop():
            nonlocal process
            if process is not None and process.poll() is None:
                process.send_signal(signal.SIGINT)
                process.wait(timeout=10)
                assert process.returncode == 0, process.stderr.read()
            process = None

        def call(path, method='GET', payload=None):
            headers = {'Authorization': 'Bearer ' + record['token']}
            if session:
                headers['X-Diary-Session'] = session
            if payload is not None:
                headers['Content-Type'] = 'application/json'
            request = urllib.request.Request(record['origin'] + '/api/v1' + path,
                                             data=None if payload is None else json.dumps(payload).encode(),
                                             method=method, headers=headers)
            try:
                with opener.open(request, timeout=30) as response:
                    return response.status, json.loads(response.read())
            except urllib.error.HTTPError as error:
                return error.code, json.loads(error.read())

        def imported():
            code, ack = call('/imports', 'POST', {'date': '2024-02-29', 'request_id': str(uuid.uuid4())})
            assert code == 202, (code, ack)
            for _ in range(500):
                code, job = call('/jobs/' + ack['import_id'])
                if job['state'] in ['succeeded', 'failed', 'cancelled']:
                    assert job['state'] == 'succeeded', job
                    break
                time.sleep(0.02)
            else:
                raise AssertionError('import deadline')
            code, page = call('/sources?import_id=' + ack['import_id'])
            assert code == 200 and not page['incomplete'], (code, page)
            return ack['import_id'], page

        admitted = False
        try:
            start()
            old_import, old_page = imported()
            rows = [i['source'] for i in old_page['items'] if i['source']]
            assert len(rows) == 2
            target = next(r for r in rows if r['relative_key'] == relative)
            source_id = target['id']
            prefix = '/sources/' + source_id
            code, preview = call(prefix + '/trash-preview', 'POST', {'kind': 'initial', 'expected_checkpoint': 0})
            assert code == 200, (code, preview)
            assert md.exists() and recording.exists()
            assert [m['relative_label'] for m in preview['members']] == [relative, str(recording.relative_to(source))]
            request = {'kind': 'initial', 'operation_id': '00000000-0000-4000-8000-000000000001',
                       'step_id': '00000000-0000-4000-8000-000000000002', 'expected_checkpoint': 0,
                       'plan_hash': preview['plan_hash'], 'ordinals': preview['ordinals'], 'token': preview['token']}
            code, ack = call(prefix + '/trash', 'POST', request)
            admitted = True
            assert code == 200 and ack['admitted_checkpoint'] == 1, (code, ack)
            code, outcome = call('/operations/' + request['operation_id'])
            assert code == 200 and outcome['trash']['state'] == 'succeeded', (code, outcome)
            code, unchanged = call('/sources?import_id=' + old_import)
            assert code == 200 and unchanged == old_page, (code, unchanged)
            _, fresh = imported()
            assert [r['source']['id'] for r in fresh['items'] if r['source']] == [next(r['id'] for r in rows if r['relative_key'] != relative)]
            stop()
            start()
            code, duplicate = call(prefix + '/trash', 'POST', request)
            assert code == 200 and duplicate == ack, (code, duplicate)
            code, recovered = call('/operations/' + request['operation_id'])
            assert code == 200 and recovered['trash'] == outcome['trash'], (code, recovered)
            code, old_after_restart = call('/sources?import_id=' + old_import)
            assert code == 200 and old_after_restart == old_page
            _, refreshed = imported()
            assert len([r for r in refreshed['items'] if r['source']]) == 1
            assert not md.exists() and not recording.exists() and assets.is_dir()
            for p, digest in preserved.items():
                assert hashlib.sha256(p.read_bytes()).digest() == digest
            result = subprocess.run([str(producer), 'catalog', str(source)], capture_output=True, text=True, check=True)
            catalog = json.loads(result.stdout)
            assert len(catalog['summaries']) == 1 and catalog['summaries'][0]['asset_id'] == neighbor_id
            assert catalog['recovery'] == [{'issue': 'changed', 'has_text': False, 'has_audio': False}]
            print('PASS normal server HTTP preview/confirm/immutable snapshots/fresh imports/restart/duplicate', flush=True)
            return {'scenario': 'enabled-server', 'outcome': outcome['trash'], 'catalog': catalog}
        finally:
            stop()
            if admitted:
                env = dict(os.environ, DIARY_TRASH_FIXTURE=str(base), DIARY_TRASH_ACTION='cleanup-database')
                env.pop('DIARY_FAULT', None)
                cleanup = subprocess.run([worker, 'source::trash::service::native_harness::native_process_worker',
                                          '--exact', '--ignored'], env=env, capture_output=True, text=True)
                assert cleanup.returncode == 0, cleanup.stdout + cleanup.stderr
