"""Ordinary copied-server qualification. No builds, Vite, fixtures or private config."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from isolation import server_configuration


def stop(process):
    """Only signal a session created by this harness; bound graceful shutdown."""
    # The leader can exit while a child still owns its session/process group.
    # Every caller must have created this process with start_new_session=True.
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        process.wait(timeout=3)
        return
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        process.poll()  # Reap the leader, allowing its process group to disappear.
        try:
            os.killpg(process.pid, 0)
        except ProcessLookupError:
            break
        time.sleep(0.02)
    else:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    process.wait(timeout=3)


def owned_record(record, pid):
    if not isinstance(record, dict) or type(record.get('pid')) is not int or record['pid'] != pid:
        return False
    from urllib.parse import urlsplit
    try:
        origin = urlsplit(record.get('origin', ''))
        return (origin.scheme == 'http' and origin.hostname == '127.0.0.1'
                and origin.port is not None and not origin.username and not origin.password
                and origin.path == '' and not origin.query and not origin.fragment
                and all(isinstance(record.get(key), str) and record[key]
                        for key in ('token', 'instance_id')))
    except (ValueError, TypeError, AttributeError):
        return False


def ready(process, runtime, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError('server exited before readiness')
        for path in runtime.glob('*/discovery.json'):
            try:
                record = json.loads(path.read_text())
            except (OSError, ValueError):
                continue
            if owned_record(record, process.pid):
                return record
        time.sleep(0.02)
    raise TimeoutError('owned server readiness timeout')


def run(server):
    server = Path(server).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='diary release install ') as temporary:
        base = Path(temporary).resolve()
        base.chmod(0o700)
        copied = base / 'diary-server'
        shutil.copy2(server, copied)
        runtime, workspace = base / 'runtime', base / 'workspace'
        for directory in (runtime, workspace):
            directory.mkdir(mode=0o700)
        config = base / 'private config.yaml'
        config.write_text(server_configuration() + f'schema_version: 1\npaths:\n  entries_root: {json.dumps(str(workspace))}\n  runtime_root: {json.dumps(str(runtime))}\n')
        config.chmod(0o600)
        processes = []
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

        def start():
            # File-backed output cannot deadlock readiness or shutdown on a full pipe.
            with (base / 'server.log').open('ab') as log:
                process = subprocess.Popen([str(copied), '--config', str(config)], cwd=base,
                                           stdout=log, stderr=log, start_new_session=True)
            processes.append(process)
            return process, ready(process, runtime)

        def call(record, path, method='GET', session=None):
            headers = {'Authorization': 'Bearer ' + record['token'], 'Content-Type': 'application/json'}
            if session:
                headers['X-Diary-Session'] = session
            req = urllib.request.Request(record['origin'] + '/api/v1' + path,
                                         data=b'{}' if method == 'POST' else None,
                                         headers=headers, method=method)
            try:
                with opener.open(req, timeout=3) as response:
                    return response.status, json.loads(response.read() or b'null')
            except urllib.error.HTTPError as error:
                return error.code, None

        sentinel = subprocess.Popen(['/bin/sleep', '120'], start_new_session=True)
        processes.append(sentinel)
        try:
            first, record = start()
            status = call(record, '/status')[1]
            assert status['instance_id'] == record['instance_id']
            assert status['server_version'] == '1.0.0'
            assert status['api_version'] == 1 and status['config_schema_version'] == 1
            assert call(dict(record, token='invalid'), '/status')[0] == 401
            code, lease = call(record, '/sessions', 'POST')
            assert code == 200
            session = lease['session_id']
            # scaffold-smoke admits a synthetic job on session open; production must not.
            code, jobs = call(record, '/jobs', session=session)
            assert code == 200 and jobs['jobs'] == [], jobs
            assert call(record, '/sessions', 'POST')[0] == 409
            assert call(record, '/sessions/' + session, 'DELETE', session)[0] == 204
            assert first.poll() is None and sentinel.poll() is None
            stop(first)
            second, fresh = start()
            assert fresh['instance_id'] != record['instance_id'] and fresh['token'] != record['token']
            assert call(dict(fresh, token=record['token']), '/status')[0] == 401
            assert sentinel.poll() is None
            second.kill()
            second.wait(timeout=3)
            assert list(runtime.glob('*/discovery.json')), 'crash fixture must retain stale discovery'
            third, recovered = start()
            assert recovered['instance_id'] != fresh['instance_id']
            assert call(dict(recovered, token=fresh['token']), '/status')[0] == 401
            stop(third)
            assert not list(runtime.glob('*/discovery.json'))
            return {'result': 'passed', 'mode': 'ordinary copied server; HTTP lease client',
                    'server_sha256': hashlib.sha256(copied.read_bytes()).hexdigest(),
                    'gui': 'unverified; no native client actions in this lane'}
        finally:
            for process in reversed(processes):
                stop(process)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', required=True, type=Path)
    args = parser.parse_args()
    print(json.dumps(run(args.server), indent=2))
