"""Close an ordinary run-gui.sh window and verify its launch-owned tree exits.

Needs a macOS GUI session and System Events accessibility permission. Uses only
an isolated temporary server/workspace; never reads the user's configuration.
"""
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[2]


def snapshot():
    result = subprocess.run(['ps', '-axo', 'pid=,ppid=,pgid=,stat=,comm='],
                            check=True, capture_output=True, text=True)
    return {int(parts[0]): (int(parts[1]), int(parts[2]), parts[3], parts[4])
            for line in result.stdout.splitlines()
            if len(parts := line.strip().split(None, 4)) == 5}


def wait_for(predicate, timeout=180):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(.1)
    raise AssertionError('GUI close smoke deadline exceeded')


def descendants(processes, parent):
    found = {parent}
    while True:
        newer = found | {pid for pid, info in processes.items() if info[0] in found}
        if newer == found:
            return found
        found = newer


def main():
    # Refuse an occupied dev port rather than borrowing another launch's Vite.
    for host, family in [('127.0.0.1', socket.AF_INET), ('::1', socket.AF_INET6)]:
        with socket.socket(family) as probe:
            if probe.connect_ex((host, 1420)) == 0:
                raise AssertionError('dev port 1420 already in use; stop its owner first')
    with tempfile.TemporaryDirectory(prefix='diary-gui-close-') as directory:
        base = Path(directory).resolve()
        base.chmod(0o700)
        for name in ('workspace', 'runtime'):
            (base / name).mkdir(mode=0o700)
        config = base / 'config.yaml'
        config.write_text(f'schema_version: 1\npaths:\n  entries_root: {base / "workspace"}\n  runtime_root: {base / "runtime"}\nsources: []\n')
        config.chmod(0o600)
        launcher = None
        owned = set()
        groups = set()
        with (base / 'server.log').open('w') as server_log, (base / 'gui.log').open('w') as gui_log:
            server = subprocess.Popen([str(ROOT / 'src-tauri/target/debug/diary-server'),
                                       '--config', str(config), '--development-origin', 'http://localhost:1420'],
                                      stdout=server_log, stderr=server_log)
            try:
                discovery = wait_for(lambda: next((base / 'runtime').glob('*/discovery.json'), None), 10)
                record = json.loads(discovery.read_text())
                launcher = subprocess.Popen(['bash', str(ROOT / 'run-gui.sh'), str(discovery)],
                                            cwd=ROOT, stdout=gui_log, stderr=gui_log,
                                            start_new_session=True)

                def client_pid():
                    if launcher.poll() is not None:
                        gui_log.flush()
                        detail = (base / 'gui.log').read_text()[-4000:]
                        raise AssertionError('launcher exited before window readiness:\n' + detail)
                    processes = snapshot()
                    owned.update(descendants(processes, launcher.pid))
                    groups.update(processes[pid][1] for pid in owned if pid in processes)
                    return next((pid for pid in owned if pid in processes and
                                 Path(processes[pid][3]).name == 'diary-client'), None)

                pid = wait_for(client_pid)
                # Wait for automatic session admission before closing; tokens stay in memory.
                opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
                log_root = discovery.parent / 'logs'
                wait_for(lambda: any('session.opened' in path.read_text()
                                     for path in log_root.rglob('*.log')), 15)
                # AXCloseButton is the same close request as clicking the title-bar control.
                apple_script = f'''tell application "System Events"
set targetProcess to first application process whose unix id is {pid}
set frontmost of targetProcess to true
repeat 100 times
    if (count of windows of targetProcess) > 0 then exit repeat
    delay 0.1
end repeat
tell window 1 of targetProcess
    click (first button whose subrole is "AXCloseButton")
end tell
end tell'''
                subprocess.run(['osascript', '-e', apple_script], check=True,
                               capture_output=True, text=True, timeout=20)
                self_status = launcher.wait(timeout=15)
                assert self_status == 0, f'launcher status {self_status}'

                def tree_gone():
                    processes = snapshot()
                    return not any(info[1] in groups and not info[2].startswith('Z')
                                   for info in processes.values())

                wait_for(tree_gone, 10)
                assert server.poll() is None, 'independent server exited'
                request = urllib.request.Request(record['origin'] + '/api/v1/sessions',
                            data=json.dumps({'session_id': 'b6214bd8-566f-4d26-a522-ce416628eeca'}).encode(),
                            headers={'Authorization': 'Bearer ' + record['token'], 'Content-Type': 'application/json'})
                # Immediate admission proves the GUI released its lease instead of waiting for expiry.
                with opener.open(request, timeout=3) as response:
                    assert response.status in (200, 201)
                print('PASS: ordinary native close exits launcher/client/dev tree; independent server stays alive and admits a new session')
            finally:
                if launcher is not None and launcher.poll() is None:
                    launcher.terminate()
                    try:
                        launcher.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        launcher.kill()
                        launcher.wait(timeout=5)
                for group in groups:
                    try:
                        os.killpg(group, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                server.terminate()
                server.wait(timeout=15)


if __name__ == '__main__':
    main()
