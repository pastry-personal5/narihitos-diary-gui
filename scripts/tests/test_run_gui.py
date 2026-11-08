"""Process-level launcher regression tests; only synthetic processes/workspaces."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = r'''
import json, os, pathlib, signal, subprocess, sys, time
base = pathlib.Path(os.environ['GUI_TEST_ROOT'])
role = sys.argv[1] if len(sys.argv) > 1 and sys.argv[1] in ('child', 'grandchild') else 'npm'
if role == 'npm':
    (base / 'arguments.json').write_text(json.dumps(sys.argv[1:]))
if os.environ.get('GUI_TEST_RESIST') == '1' and role != 'npm':
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
if role != 'grandchild':
    subprocess.Popen([sys.executable, __file__, 'child' if role == 'npm' else 'grandchild'])
(base / (role + '.pid')).write_text(str(os.getpid()))
while True:
    if role == 'npm' and (base / 'close').exists():
        sys.exit(int(os.environ.get('GUI_TEST_STATUS', '0')))
    time.sleep(.01)
'''


def alive(pid):
    # A zombie is already exited, awaiting its OS parent to reap it.
    result = subprocess.run(['ps', '-o', 'stat=', '-p', str(pid)],
                            capture_output=True, text=True, check=False)
    return result.returncode == 0 and not result.stdout.strip().startswith('Z')


def until(predicate, timeout=8):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.02)
    raise AssertionError('process lifecycle deadline exceeded')


class LauncherCleanup(unittest.TestCase):
    def exercise(self, *, status=0, interrupt=None, resist=False):
        with tempfile.TemporaryDirectory(prefix='diary-launch-test-') as directory:
            base = Path(directory).resolve()
            discovery = base / 'discovery.json'
            discovery.write_text('{}')
            npm = base / 'npm'
            npm.write_text(f'#!{sys.executable}\n' + FIXTURE)
            npm.chmod(0o700)
            env = dict(os.environ, PATH=str(base) + os.pathsep + os.environ['PATH'],
                       GUI_TEST_ROOT=str(base), GUI_TEST_STATUS=str(status),
                       GUI_TEST_RESIST='1' if resist else '0')
            sentinel = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])
            launcher = None
            pids = []
            try:
                with (base / 'launcher.log').open('w+') as log:
                    launcher = subprocess.Popen(['bash', str(ROOT / 'run-gui.sh'), str(discovery)],
                                                env=env, stdout=log, stderr=log,
                                                start_new_session=True)
                    until(lambda: all((base / (role + '.pid')).exists()
                                      for role in ('npm', 'child', 'grandchild')))
                    pids = [int((base / (role + '.pid')).read_text())
                            for role in ('npm', 'child', 'grandchild')]
                    self.assertEqual(os.getpgid(pids[0]), pids[0])
                    self.assertTrue(all(os.getpgid(pid) == pids[0] for pid in pids))
                    args = json.loads((base / 'arguments.json').read_text())
                    self.assertEqual(args, ['run', 'tauri', '--', 'dev', '--exit-on-panic',
                                            '--', '--', '--connect', str(discovery)])
                    if interrupt:
                        launcher.send_signal(interrupt)
                    else:
                        (base / 'close').touch()
                    self.assertEqual(launcher.wait(timeout=10),
                                     128 + interrupt if interrupt else status)
                    until(lambda: all(not alive(pid) for pid in pids))
                    self.assertIsNone(sentinel.poll(), 'independent server sentinel was killed')
            finally:
                if launcher is not None and launcher.poll() is None:
                    launcher.kill()
                    launcher.wait(timeout=5)
                # Only the synthetic launch-owned group, never name/port-based cleanup.
                if pids:
                    try:
                        os.killpg(pids[0], signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                sentinel.terminate()
                sentinel.wait(timeout=5)

    def test_normal_close_cleans_descendants(self):
        self.exercise()

    def test_failure_preserves_status_and_cleans_descendants(self):
        self.exercise(status=7)

    def test_signals_clean_descendants(self):
        for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            with self.subTest(signal=sig):
                self.exercise(interrupt=sig)

    def test_term_resistant_descendants_are_killed(self):
        self.exercise(resist=True)


if __name__ == '__main__':
    unittest.main()
