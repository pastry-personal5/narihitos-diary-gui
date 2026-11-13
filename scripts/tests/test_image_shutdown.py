"""GPU-free server signal proof with real supervised image processes."""
import concurrent.futures
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.error
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[2]
TARGET = Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'src-tauri/target')).resolve()
ENGINE = '''import os, signal, subprocess, sys, time
from pathlib import Path
base = Path(__file__).parent
if sys.argv[1:] == ['--check'] and not (base / 'block-probe').exists(): sys.exit(0)
if (base / 'partial').exists() and not (base / 'first-photo').exists():
    import struct, zlib
    def chunk(k, b): return struct.pack('>I', len(b)) + k + b + struct.pack('>I', zlib.crc32(k + b))
    png = bytes([137,80,78,71,13,10,26,10]) + chunk(b'IHDR', struct.pack('>IIBBBBB', 1,1,8,2,0,0,0)) + chunk(b'IDAT', zlib.compress(bytes([0,255,0,0]))) + chunk(b'IEND', b'')
    Path(os.environ['DIARY_IMAGE_OUT']).write_bytes(png)
    (base / 'first-photo').touch()
    sys.exit(0)
stubborn = (base / 'stubborn').exists()
def stop(*_):
    (base / 'engine.term').write_text('TERM')
    child.wait(timeout=2)
    sys.exit(0)
signal.signal(signal.SIGTERM, signal.SIG_IGN if stubborn else stop)
child_code = """import os,signal,sys,time
from pathlib import Path
base=Path(sys.argv[1])
def stop(*_):
    (base/'child.term').write_text('TERM')
    sys.exit(0)
signal.signal(signal.SIGTERM,signal.SIG_IGN if (base/'stubborn').exists() else stop)
(base/'child.pid').write_text(str(os.getpid()))
time.sleep(30)
"""
child = subprocess.Popen([sys.executable, '-c', child_code, str(base)])
while not (base / 'child.pid').exists(): time.sleep(0.01)
(base / 'engine.mode').write_text('probe' if sys.argv[1:] == ['--check'] else 'render')
(base / 'engine.pid').write_text(str(os.getpid()))
time.sleep(30)
'''


def until(predicate, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.02)
    raise AssertionError('timed out waiting for isolated fixture')


def alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False


class ImageShutdown(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        for package, extra in [('diary-server', []), ('diary-infrastructure', ['--example', 'fake_cli'])]:
            subprocess.run(['cargo', 'build', '--quiet', '--manifest-path', str(ROOT / 'src-tauri/Cargo.toml'), '-p', package, *extra], check=True)

    def scenario(self, sig, mode, stubborn=False, reload=False):
        with tempfile.TemporaryDirectory(prefix='diary-image-shutdown-') as tmp:
            base = Path(tmp).resolve()
            workspace, runtime, home = [base / n for n in ('workspace', 'runtime', 'home')]
            for directory in (workspace, runtime, home):
                directory.mkdir(mode=0o700)
            (home / 'scenario').write_text('illustrated')
            program = base / 'engine'
            program.write_text('#!' + sys.executable + '\n' + ENGINE)
            program.chmod(0o700)
            if mode == 'probe':
                (base / 'block-probe').touch()
            if mode == 'partial':
                (base / 'partial').touch()
            if stubborn:
                (base / 'stubborn').touch()
            with socket.socket() as probe:
                probe.bind(('127.0.0.1', 0))
                port = probe.getsockname()[1]
            config = base / 'config.yaml'
            config.write_text(f'''schema_version: 1
server:
  port: {port}
  port_fallback_count: 0
paths:
  entries_root: {workspace}
  runtime_root: {runtime}
agents:
  profiles:
    - alias: synthetic
      type: codex
      profile_home: {home}
      executable: {TARGET / 'debug/examples/fake_cli'}
image_engines:
  synthetic:
    label: Synthetic
    program: {program}
    timeout_seconds: 30
''')
            config.chmod(0o600)
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
            sentinel = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'])
            record, session = None, None
            def call(path, method='GET', body=None):
                headers = {'Authorization': 'Bearer ' + record['token'], 'Content-Type': 'application/json'}
                if session:
                    headers['X-Diary-Session'] = session
                request = urllib.request.Request(record['origin'] + '/api/v1' + path, method=method, headers=headers, data=None if body is None else json.dumps(body).encode())
                with opener.open(request, timeout=10) as response:
                    raw = response.read()
                    return json.loads(raw) if raw else None
            with (base / 'server.stderr').open('w') as log:
                server = subprocess.Popen([str(TARGET / 'debug/diary-server'), '--config', str(config)], stdout=subprocess.DEVNULL, stderr=log)
                try:
                    def discovery():
                        nonlocal record
                        self.assertIsNone(server.poll(), 'isolated server exited before discovery')
                        for file in runtime.glob('*/discovery.json'):
                            candidate = json.loads(file.read_text())
                            if candidate['pid'] == server.pid:
                                record = candidate
                                return True
                        return False
                    until(discovery)
                    session = call('/sessions', 'POST', {})['session_id']
                    if reload:
                        config.write_text(config.read_text().replace('label: Synthetic', 'label: Reloaded'))
                        call('/settings/reload', 'POST', {})
                        self.assertEqual(call('/image-engines')['image_engines'][0]['label'], 'Reloaded')
                    def start():
                        if mode == 'probe':
                            try:
                                call('/image-engines/synthetic/check', 'POST')
                            except (OSError, urllib.error.URLError):
                                pass  # Shutdown may close this in-flight response.
                        else:
                            date = '2026-11-13'
                            call('/authentic/' + date, 'PUT', {'operation_id': str(uuid.uuid4()), 'expected_revision': None, 'body': 'I had tea. Later I walked outside.\n', 'buffer_generation': 1})
                            return call('/illustrated/' + date, 'POST', {'request_id': str(uuid.uuid4()), 'profile': 'synthetic', 'locale': 'en-US', 'engine': 'synthetic'})
                    with concurrent.futures.ThreadPoolExecutor() as pool:
                        request = pool.submit(start)
                        try:
                            until(lambda: (base / 'engine.pid').exists())
                        except AssertionError:
                            result = request.result(timeout=1)
                            job = call('/jobs/' + result['job']['id']) if result else None
                            self.fail(f'engine never started: job={job}, first_photo={(base / "first-photo").exists()}, staging={list(runtime.glob("*/image-runs/image-run-*"))}')
                        self.assertEqual((base / 'engine.mode').read_text(), 'probe' if mode == 'probe' else 'render')
                        if mode == 'partial':
                            self.assertTrue((base / 'first-photo').exists())
                        pids = [int((base / name).read_text()) for name in ('engine.pid', 'child.pid')]
                        self.assertTrue(all(alive(pid) for pid in pids))
                        began = time.monotonic()
                        server.send_signal(sig)
                        self.assertEqual(server.wait(timeout=3), 0)
                        self.assertLess(time.monotonic() - began, 3)
                        request.result(timeout=3)
                    for pid in pids:
                        until(lambda: not alive(pid), timeout=3)
                    self.assertEqual((base / 'engine.term').exists(), not stubborn)
                    self.assertEqual((base / 'child.term').exists(), not stubborn)
                    self.assertIsNone(sentinel.poll(), 'shutdown killed an unrelated process')
                    self.assertFalse(list(runtime.glob('*/image-runs/image-run-*')))
                    self.assertFalse(list(runtime.glob('*/discovery.json')))
                    self.assertFalse(list((workspace / 'assets/diary/illustrated').rglob('*.md')))
                finally:
                    if server.poll() is None:
                        server.terminate()
                        try:
                            server.wait(timeout=8)
                        except subprocess.TimeoutExpired:
                            server.kill()
                            server.wait(timeout=3)
                    sentinel.terminate()
                    sentinel.wait(timeout=3)
                    # Only kill a remaining fixture process if its argv still names
                    # this unique temporary fixture, avoiding reused unrelated PIDs.
                    for name in ('engine.pid', 'child.pid'):
                        path = base / name
                        if path.exists():
                            pid = int(path.read_text())
                            command = subprocess.run(['/bin/ps', '-p', str(pid), '-o', 'command='], capture_output=True, text=True).stdout
                            if str(base) in command:
                                try:
                                    os.kill(pid, signal.SIGKILL)
                                except ProcessLookupError:
                                    pass

    def test_signals_stop_render_and_probe_process_groups(self):
        for sig in (signal.SIGTERM, signal.SIGINT):
            for mode in ('render', 'probe'):
                with self.subTest(signal=sig.name, mode=mode):
                    self.scenario(sig, mode)

    def test_shutdown_escalates_for_stubborn_descendants(self):
        self.scenario(signal.SIGTERM, 'render', stubborn=True)

    def test_shutdown_disposes_already_completed_photo_staging(self):
        self.scenario(signal.SIGTERM, 'partial')

    def test_reloaded_engine_keeps_server_shutdown_signal(self):
        self.scenario(signal.SIGTERM, 'probe', reload=True)


if __name__ == '__main__':
    unittest.main()
