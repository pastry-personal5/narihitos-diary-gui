"""GPU-free interruption proof against the real Rust owner and command adapter."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest
from types import SimpleNamespace
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    "image_generation_live", ROOT / "scripts/smoke/image-generation-live.py")
live = importlib.util.module_from_spec(spec)
spec.loader.exec_module(live)


def alive(pid):
    result = subprocess.run(["/bin/ps", "-o", "stat=", "-p", str(pid)],
                            capture_output=True, text=True, check=False)
    return bool(result.stdout.strip()) and not result.stdout.strip().startswith("Z")


class ImageHarnessCleanup(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        output = subprocess.check_output([
            "cargo", "build", "--quiet", "--manifest-path", str(ROOT / "src-tauri/Cargo.toml"),
            "-p", "diary-infrastructure", "--example", "image_engine_live", "--message-format=json"
        ], cwd=ROOT, text=True, timeout=120)
        cls.binary = next(item["executable"] for line in output.splitlines()
                          if (item := json.loads(line)).get("executable")
                          and item.get("target", {}).get("name") == "image_engine_live")

    def test_preflight_failure_is_retained_in_machine_readable_report(self):
        def unavailable():
            raise ValueError("local prerequisites unavailable")

        with tempfile.TemporaryDirectory(prefix="image preflight ") as directory:
            root = Path(directory)
            (root / "tmp").mkdir()
            spec = SimpleNamespace(loader=SimpleNamespace(exec_module=lambda _module: None))
            with patch.object(live, "ROOT", root), patch.dict(os.environ, DIARY_IMAGE_LIVE="1"), \
                    patch.object(live.importlib.util, "spec_from_file_location", return_value=spec), \
                    patch.object(live.importlib.util, "module_from_spec", return_value=SimpleNamespace(selection=unavailable)):
                with self.assertRaisesRegex(ValueError, "local prerequisites unavailable"):
                    live.main()
            report = json.loads(next((root / "tmp").glob("*/report.json")).read_text())
            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["failure"], {
                "type": "ValueError", "message": "local prerequisites unavailable"})
            self.assertIn("finished_utc", report)

    def test_monitor_failure_cancels_separately_grouped_renderer_and_preserves_sentinel(self):
        with tempfile.TemporaryDirectory(prefix="image interruption ") as directory:
            root = Path(directory).resolve()
            program = root / "fake renderer.sh"
            program.write_text('''#!/bin/sh
[ "${1-}" = --check ] && exit 0
echo $$ > "$DIARY_IMAGE_OUT.started"
/bin/sleep 120 &
echo $! > "$DIARY_IMAGE_OUT.child"
wait
''')
            program.chmod(0o700)
            sentinel = subprocess.Popen(["/bin/sleep", "120"], start_new_session=True)
            with (root / "run.log").open("w") as log:
                child = subprocess.Popen([self.binary], stdin=subprocess.PIPE,
                                         stdout=log, stderr=log, start_new_session=True,
                                         env=dict(os.environ, DIARY_IMAGE_LIVE="1",
                                                  DIARY_IMAGE_LIVE_PROGRAM=str(program),
                                                  DIARY_IMAGE_EVIDENCE=str(root)))
                try:
                    deadline = time.monotonic() + 10
                    markers = []
                    while time.monotonic() < deadline:
                        markers = list(root.rglob("*.child"))
                        if markers and markers[0].read_text().strip():
                            break
                        if child.poll() is not None:
                            self.fail("fake renderer failed to start")
                        time.sleep(0.02)
                    self.assertTrue(markers, "renderer did not start")
                    wrapper = int(next(root.rglob("*.started")).read_text())
                    renderer = int(markers[0].read_text())
                    self.assertNotEqual(os.getpgid(wrapper), child.pid)
                    with self.assertRaisesRegex(RuntimeError, "telemetry failed"):
                        try:
                            raise RuntimeError("telemetry failed")
                        finally:
                            live.stop_child(child)
                    self.assertNotEqual(child.returncode, 0)
                    deadline = time.monotonic() + 5
                    while any(alive(pid) for pid in (wrapper, renderer)) and time.monotonic() < deadline:
                        time.sleep(0.02)
                    self.assertFalse(alive(wrapper))
                    self.assertFalse(alive(renderer))
                    self.assertIsNone(sentinel.poll())
                    self.assertFalse(list(root.rglob("image-run-*")))
                    self.assertIn("Cancelled", (root / "run.log").read_text())
                finally:
                    live.stop_child(child)
                    sentinel.terminate()
                    sentinel.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
