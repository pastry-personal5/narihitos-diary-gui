"""Build orchestration regressions using fake tools; no compilation or GUI."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class BuildHarnessTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='diary build harness ')
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        tools = self.base / 'tools'
        tools.mkdir()
        for name in ('npm', 'cargo', 'npx', 'python3', 'node'):
            tool = tools / name
            tool.write_text('''#!/bin/sh
printf '%s %s\\n' "$(basename "$0")" "$*" >> "$HARNESS_COMMAND_LOG"
if [ "${HARNESS_FAIL_TEST:-}" = 1 ] && [ "$1" = test ]; then exit 7; fi
''')
            tool.chmod(0o700)
        self.log = self.base / 'commands'
        self.env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ['PATH'],
                        HARNESS_COMMAND_LOG=str(self.log))
        # Exercise the actual scripts from an isolated checkout-shaped tree.
        (self.base / 'scripts').mkdir()
        shutil.copy2(ROOT / 'scripts/gate.sh', self.base / 'scripts/gate.sh')
        shutil.copy2(ROOT / 'Makefile', self.base / 'Makefile')

    def make(self, target, **environment):
        return subprocess.run(['make', target], cwd=self.base,
                              env=dict(self.env, **environment), capture_output=True,
                              text=True, check=True)

    def test_release_targets_remain_phony_and_use_ordinary_builds(self):
        for target in ('package', 'release-server', 'release', 'versions'):
            (self.base / target).touch()
        self.make('release')
        self.assertEqual(self.log.read_text().splitlines(), [
            'npm run check:versions',
            'cargo build --release --manifest-path src-tauri/Cargo.toml -p diary-server',
            'npm run tauri build -- --bundles app',
        ])

    def test_smoke_builds_fixture_server_and_passes_custom_artifact_paths(self):
        target = str(self.base / 'custom target')
        self.make('smoke', CARGO_TARGET_DIR=target)
        commands = self.log.read_text().splitlines()
        self.assertIn('cargo build --manifest-path src-tauri/Cargo.toml -p diary-server --features scaffold-smoke', commands)
        self.assertEqual(commands[-1], f"python3 scripts/smoke/server.py --server {target}/debug/diary-server {target}/debug/bundle/macos/Narihito's Diary.app/Contents/MacOS/diary-client")

    def test_exclusion_rebuilds_ordinary_artifacts_in_custom_target(self):
        target = str(self.base / 'custom target')
        self.make('exclusion', CARGO_TARGET_DIR=target)
        commands = self.log.read_text().splitlines()
        self.assertEqual(commands[0], 'npm run tauri build -- --debug --bundles app')
        self.assertEqual(commands[1], 'cargo build --manifest-path src-tauri/Cargo.toml -p diary-server')
        self.assertEqual(commands[2], f'node scripts/check-fixture-exclusion.mjs --target-dir {target}')

    def test_concurrent_gates_preserve_existing_logs_and_propagate_failures(self):
        log_root = self.base / 'gate logs'
        log_root.mkdir()
        sentinel = log_root / 'unrelated.log'
        sentinel.write_text('preserve')
        env = dict(self.env, GATE_LOG_DIR=str(log_root))
        passed = subprocess.Popen(['sh', 'scripts/gate.sh'], cwd=self.base, env=env,
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        failed = subprocess.Popen(['sh', 'scripts/gate.sh'], cwd=self.base,
                                  env=dict(env, HARNESS_FAIL_TEST='1'),
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            success_output, _ = passed.communicate(timeout=20)
            failure_output, _ = failed.communicate(timeout=20)
            self.assertEqual(passed.returncode, 0, success_output)
            self.assertEqual(failed.returncode, 1, failure_output)
            self.assertIn('G03 FAIL', failure_output)
            self.assertEqual(success_output.count(' PASS'), 7)
            self.assertEqual(sentinel.read_text(), 'preserve')
            runs = list(log_root.glob('run.*'))
            self.assertEqual(len(runs), 2)
            self.assertTrue(all(len(list(run.glob('*.rc'))) == 7 for run in runs))
        finally:
            for process in (passed, failed):
                if process.poll() is None:
                    process.kill()
                process.wait(timeout=3)


if __name__ == '__main__':
    unittest.main()
