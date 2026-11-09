"""Failure and cleanup regressions; never launch the diary or a service."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SMOKE = Path(__file__).resolve().parents[1] / 'smoke'
sys.path.insert(0, str(SMOKE))
spec = importlib.util.spec_from_file_location('release', SMOKE / 'release.py')
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def test_missing_artifact_fails(self):
        with tempfile.TemporaryDirectory() as root:
            with self.assertRaises(FileNotFoundError):
                release.run(Path(root) / 'absent')

    def test_early_exit_fails(self):
        process = subprocess.Popen(['/usr/bin/true'], start_new_session=True)
        process.wait(timeout=3)
        with tempfile.TemporaryDirectory() as root:
            with self.assertRaises(RuntimeError):
                release.ready(process, Path(root))

    def test_timeout_cleanup_preserves_unrelated_process(self):
        owned = subprocess.Popen(['/bin/sleep', '30'], start_new_session=True)
        other = subprocess.Popen(['/bin/sleep', '30'], start_new_session=True)
        try:
            with tempfile.TemporaryDirectory() as root:
                with self.assertRaises(TimeoutError):
                    release.ready(owned, Path(root), timeout=0.05)
            release.stop(owned)
            self.assertIsNotNone(owned.poll())
            self.assertIsNone(other.poll())
        finally:
            release.stop(owned)
            release.stop(other)

    def test_discovery_validation_rejects_malformed_unrelated_and_remote_records(self):
        valid = {'pid': 42, 'origin': 'http://127.0.0.1:12345', 'token': 'synthetic', 'instance_id': 'owned'}
        self.assertTrue(release.owned_record(valid, 42))
        for record in [None, [], {}, dict(valid, pid=43), dict(valid, pid='42'),
                       dict(valid, origin='http://example.com:80'), dict(valid, token=''),
                       dict(valid, origin='http://127.0.0.1:invalid')]:
            self.assertFalse(release.owned_record(record, 42), record)

    def test_cleanup_signals_descendants_after_leader_exit(self):
        with tempfile.TemporaryDirectory() as root:
            receipt = Path(root) / 'terminated'
            script = '''import os, signal, time, sys
path = sys.argv[1]
child = os.fork()
if child:
    while not os.path.exists(path + '.ready'): time.sleep(0.01)
    os._exit(0)
def terminated(*_):
    open(path, 'w').write('terminated')
    os._exit(0)
signal.signal(signal.SIGTERM, terminated)
open(path + '.ready', 'w').close()
while True: time.sleep(1)
'''
            leader = subprocess.Popen([sys.executable, '-c', script, str(receipt)], start_new_session=True)
            try:
                leader.wait(timeout=3)
                release.stop(leader)
                self.assertEqual(receipt.read_text(), 'terminated')
            finally:
                release.stop(leader)
