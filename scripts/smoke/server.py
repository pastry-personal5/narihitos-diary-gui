"""Synthetic standalone server lifecycle smoke; never loads private app config."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import urllib.request
import urllib.error
from isolation import server_configuration

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('app', nargs='?', type=Path)
parser.add_argument('--server', type=Path, default=Path(os.environ.get(
    'CARGO_TARGET_DIR', str(Path(__file__).resolve().parents[2] / 'src-tauri/target')
)) / 'debug/diary-server')
args = parser.parse_args()
binary = args.server.resolve(strict=True)
with tempfile.TemporaryDirectory(prefix='diary-server-smoke-') as temporary:
    base = Path(temporary).resolve()
    workspace, runtime = base / 'workspace', base / 'runtime'
    for folder in [base, workspace, runtime]:
        folder.mkdir(exist_ok=True, mode=0o700)
        folder.chmod(0o700)
    readable, denied = base / 'readable-source', base / 'denied-source'
    for folder in [readable, denied]:
        folder.mkdir(mode=0o700)
        (folder / 'sentinel.txt').write_text('synthetic scaffold sentinel\n')
    denied.chmod(0o0)
    config = base / 'config.yaml' 
    config.write_text(server_configuration() + f'schema_version: 1\npaths:\n  entries_root: {workspace}\n  runtime_root: {runtime}\nsources:\n  - {{alias: smoke-readable, type: narihito, root: {readable}}}\n  - {{alias: smoke-denied, type: ayumi, root: {denied}}}\n')
    config.chmod(0o600)
    processes = []
    tokens = []
    TRACE = 'feedfacefeedfacefeedfacefeedface'
    def start():
        process = subprocess.Popen([str(binary), '--config', str(config)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        processes.append(process)
        for _ in range(200):
            found = list(runtime.glob('*/discovery.json'))
            if found and process.poll() is None:
                candidate = json.loads(found[0].read_text())
                if candidate['pid'] == process.pid:
                    tokens.append(candidate['token'])
                    return process, found[0], candidate
            if process.poll() is not None:
                raise AssertionError('server exited before readiness')
            time.sleep(0.01)
        raise AssertionError('server readiness timeout')
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    def call(record, path, method='GET', payload=None, session=None, authorized=True, trace=None):
        headers = {'Authorization': 'Bearer ' + record['token']} if authorized else {}
        if payload is not None:
            headers['Content-Type'] = 'application/json'
        if session:
            headers['X-Diary-Session'] = session
        if trace:
            headers['X-Diary-Trace-Id'] = trace
        request = urllib.request.Request(record['origin'] + '/api/v1' + path, data=None if payload is None else json.dumps(payload).encode(), headers=headers, method=method)
        try:
            with opener.open(request, timeout=2) as response:
                data = response.read()
                return response.status, json.loads(data) if data else None
        except urllib.error.HTTPError as error:
            return error.code, None
        except urllib.error.URLError:
            return 0, None
    try:
        # P1-M5: a non-empty folder without .diary is refused untouched (before any initialization).
        occupied = base / 'occupied'
        occupied.mkdir(mode=0o700)
        (occupied / 'notes.md').write_text('existing notes')
        bad = base / 'bad.yaml'
        bad.write_text(config.read_text().replace(str(workspace), str(occupied)))
        bad.chmod(0o600)
        refused = subprocess.run([str(binary), '--config', str(bad)], capture_output=True, text=True, timeout=5)
        assert refused.returncode != 0 and 'inspect' in refused.stderr
        assert sorted(p.name for p in occupied.iterdir()) == ['notes.md']
        assert not workspace.joinpath('.diary').exists()
        first, discovery, record = start()
        marker_bytes = (workspace / '.diary/workspace.json').read_bytes()
        assert call(record, '/status', authorized=False, trace=TRACE)[0] == 401
        code, status = call(record, '/status')
        assert code == 200 and status['instance_id'] == record['instance_id'] and status['storage_ready'] is True
        code, lease = call(record, '/sessions', 'POST', {})
        assert code == 200
        session = lease['session_id']
        assert call(record, '/sessions', 'POST', {})[0] == 409
        assert call(record, '/jobs', session=session)[0] == 200
        assert call(record, '/sessions/' + session + '/heartbeat', 'POST', session=session)[0] == 204
        second = subprocess.run([str(binary), '--config', str(config)], capture_output=True, text=True, timeout=3)
        assert second.returncode != 0
        assert json.loads(discovery.read_text())['instance_id'] == record['instance_id']
        assert call(record, '/sessions/' + session, 'DELETE', session=session)[0] == 204
        assert first.poll() is None
        first.send_signal(signal.SIGTERM)
        first.wait(timeout=3)
        assert first.returncode == 0 and not discovery.exists()
        restarted, discovery, fresh = start()
        assert fresh['instance_id'] != record['instance_id'] and fresh['token'] != record['token']
        restarted.kill()
        restarted.wait(timeout=3)
        assert discovery.exists()
        assert call(fresh, '/status')[0] == 0
        stale = fresh
        restarted, discovery, fresh = start()
        assert fresh['instance_id'] != stale['instance_id'] and fresh['token'] != stale['token']

        if args.app is not None:
            app = args.app.resolve(strict=True)
            with (base / 'client.log').open('w+') as client_log:
                client = subprocess.Popen([str(app), '--connect', str(discovery)], stdout=client_log, stderr=client_log, text=True)
                processes.append(client)
                for _ in range(180):
                    client_log.seek(0)
                    if 'SMOKE main_running_job_read' in client_log.read(): break
                    if client.poll() is not None: raise AssertionError('native client exited before editor readiness')
                    time.sleep(0.05)
                keyboard_script = f'tell application "System Events"\nset targetProcess to first application process whose unix id is {client.pid}\nset frontmost of targetProcess to true\ndelay 0.3\ntell targetProcess to keystroke "synthetic keyboard sentinel"\nreturn {{frontmost of targetProcess, count of windows of targetProcess}}\nend tell'
                keyboard = subprocess.run(['osascript', '-e', keyboard_script], check=True, capture_output=True, text=True, timeout=5)
                print('Native target activation/window count: ' + keyboard.stdout.strip())
                for _ in range(100):
                    client_log.seek(0)
                    if 'SMOKE main_cancelled_job_read' in client_log.read(): break
                    time.sleep(0.05)
                client_log.seek(0)
                current_log=client_log.read()
                if 'SMOKE main_cancelled_job_read' not in current_log:
                    print('Native editor diagnostics: ' + ', '.join(line for line in current_log.splitlines() if line.startswith('SMOKE editor_probe ')))
                    print('Native client log tail: ' + ' | '.join(l[:300] for l in current_log.splitlines() if 'editor_probe' not in l)[-3000:])
                    raise AssertionError('native editor did not observe typed sentinel')
                with (base / 'competitor.log').open('w+') as competitor_log:
                    competitor = subprocess.Popen([str(app), '--connect', str(discovery)], stdout=competitor_log, stderr=competitor_log, text=True)
                    processes.append(competitor)
                    for _ in range(80):
                        competitor_log.seek(0)
                        if 'SMOKE client_busy_rejected' in competitor_log.read(): break
                        time.sleep(0.05)
                    competitor_log.seek(0)
                    assert 'SMOKE client_busy_rejected' in competitor_log.read(), 'missing competing packaged-client denial'
                    # The main client holds the client log, so the competitor logs to stderr only.
                    competitor_trace = None
                    for _ in range(80):
                        competitor_log.seek(0)
                        rows = [json.loads(l) for l in competitor_log.read().splitlines() if l.startswith('{"ts"')]
                        competitor_trace = next((r['trace'] for r in rows if r['code'] == 'ui.bridge_error' and 'trace' in r), None)
                        if competitor_trace: break
                        time.sleep(0.05)
                    assert competitor_trace, 'missing competitor client trace on stderr'
                    competitor.terminate(); competitor.wait(timeout=5)
                expected = {'SMOKE main_window_close_released', 'SMOKE main_window_destroyed', 'SMOKE main_metadata_allowed' , 'SMOKE main_handshake_allowed', 'SMOKE main_session_released', 'SMOKE secondary_command_denied', 'SMOKE main_delivery_acknowledged', 'SMOKE main_cancelled_job_read'}
                observed = set()
                for _ in range(400):
                    client_log.seek(0)
                    observed = {line.strip() for line in client_log if line.startswith('SMOKE ')}
                    if expected <= observed or client.poll() is not None:
                        break
                    time.sleep(0.05)
                print('Native IPC observations: ' + ', '.join(sorted(observed)))
                assert expected <= observed, 'missing packaged IPC evidence'
                assert not any(line.startswith('SMOKE FAIL') for line in observed)
                client.wait(timeout=5)
                assert client.returncode == 0
                code, replacement = call(fresh, "/sessions", "POST", {})
                assert code == 200
                assert call(fresh, "/sessions/" + replacement["session_id"], "DELETE", session=replacement["session_id"])[0] == 204
                assert restarted.poll() is None

        restarted.send_signal(signal.SIGINT)
        restarted.wait(timeout=3)
        assert restarted.returncode == 0 and not discovery.exists()
        # P1-M5: private workspace tree, stable identity across restarts, no credential in it.
        mode = lambda path: path.stat().st_mode & 0o7777
        assert mode(workspace / '.diary') == 0o700
        for name in ['workspace.json', 'workspace.lock', 'state.sqlite3']:
            assert mode(workspace / '.diary' / name) == 0o600, name
        assert (workspace / '.diary/workspace.json').read_bytes() == marker_bytes
        assert not any(token.encode() in marker_bytes for token in tokens)
        assert not (workspace / '.diary/workspace.json.tmp').exists()
        # P1-M5 recovery: a lost database starts read-only and is never recreated.
        for name in ['state.sqlite3', 'state.sqlite3-wal', 'state.sqlite3-shm']:
            (workspace / '.diary' / name).unlink(missing_ok=True)
        lost, lost_discovery, lost_record = start()
        code, status = call(lost_record, '/status')
        assert code == 200 and status['storage_ready'] is False
        assert not (workspace / '.diary/state.sqlite3').exists(), 'recovery recreated the ledger'
        assert (workspace / '.diary/workspace.json').read_bytes() == marker_bytes
        lost.send_signal(signal.SIGINT)
        lost.wait(timeout=3)
        assert lost.returncode == 0 and not lost_discovery.exists()
        # P1-M4 diagnostics: private logs, no seeded secret in any log or trace, redacted trace.
        server_logs = list(runtime.glob('*/logs/server'))
        assert len(server_logs) == 1, 'missing server log directory'
        assert mode(server_logs[0].parent) == 0o700 and mode(server_logs[0]) == 0o700
        log_files = [p for p in server_logs[0].iterdir() if p.suffix == '.log']
        assert log_files and all(mode(p) == 0o600 for p in log_files)
        log_text = ''.join(p.read_text() for p in log_files)
        assert tokens and not any(token in log_text for token in tokens), 'token leaked into the log'
        assert 'synthetic scaffold sentinel' not in log_text
        codes = {json.loads(l)['code'] for l in log_text.splitlines()}
        assert {'server.started', 'workspace.opened', 'workspace.recovery', 'server.listener_bound', 'server.discovery_published', 'server.stopping', 'session.opened', 'session.closed', 'session.client_busy', 'http.auth_rejected'} <= codes, codes
        def trace(selector):
            done = subprocess.run([str(binary), 'trace', '--config', str(config), '--select', selector], capture_output=True, text=True, timeout=5)
            assert done.returncode == 0, done.stderr
            assert not any(token in done.stdout for token in tokens), 'token leaked into trace output'
            lines = [json.loads(l) for l in done.stdout.splitlines()]
            return lines[:-1], lines[-1]['summary']
        rows, summary = trace('trace:' + TRACE)
        assert [r['code'] for r in rows] == ['http.auth_rejected'] and rows[0]['trace'] == TRACE and rows[0]['role'] == 'server'
        assert summary['returned'] == 1 and summary['invalid'] == 0 and not summary['truncated']
        assert any(r.get('trace') == TRACE for r in trace('recent:10')[0])
        if args.app is not None:
            client_logs = list(runtime.glob('*/logs/client'))
            assert len(client_logs) == 1 and mode(client_logs[0]) == 0o700, 'missing client log directory'
            assert all(mode(p) == 0o600 for p in client_logs[0].iterdir()), 'client log file modes'
            client_text = ''.join(p.read_text() for p in client_logs[0].iterdir() if p.suffix == '.log')
            assert not any(token in client_text for token in tokens), 'token leaked into the client log'
            rows, _ = trace('trace:' + competitor_trace)
            assert any(r['code'] == 'session.client_busy' and r['role'] == 'server' for r in rows), 'competitor request not found in the server log'
            print('Trace correlation: competitor client stderr trace ' + competitor_trace + ' found in the server log')
        if args.app is not None:
            folder_messages = [line.strip() for line in restarted.stderr.read().splitlines() if line.startswith('SMOKE server_folder_')]
            assert 'SMOKE server_folder_readable' in folder_messages
            assert 'SMOKE server_folder_denied' in folder_messages
            assert any(line.startswith('SMOKE server_folder_denied errno=') for line in folder_messages)
            print('Server folder observations: ' + ', '.join(folder_messages))
        print('PASS: private 0700/0600 server logs, seeded-secret scan, redacted trace by trace/recent selectors, standalone launch, auth/status/session exclusion, OS lock exclusion, client release, SIGTERM/SIGINT cleanup, crash/stale rejection, restart rotation')
    finally:
        denied.chmod(0o700)
        for process in processes:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=3)
