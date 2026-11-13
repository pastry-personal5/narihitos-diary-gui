"""No live calls: fixture server readiness failures must clean up only their child."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys
import unittest
from unittest.mock import MagicMock, patch

SCRIPT = Path(__file__).resolve().parents[1] / 'smoke/illustrated-notion.py'
spec = importlib.util.spec_from_file_location('illustrated_notion', SCRIPT)
live = importlib.util.module_from_spec(spec)
sys.path.insert(0, str(SCRIPT.parent))
try:
    spec.loader.exec_module(live)
finally:
    sys.path.pop(0)


class ReadinessCleanup(unittest.TestCase):
    def test_failed_session_admission_stops_its_server_before_returning_error(self):
        process = MagicMock(pid=42)
        process.poll.return_value = None
        record = MagicMock()
        record.read_text.return_value = json.dumps({'pid': 42})
        runtime = MagicMock()
        runtime.glob.return_value = [record]
        with patch.object(live.subprocess, 'Popen', return_value=process), \
                patch.object(live.Server, 'call', side_effect=RuntimeError('admission failed')):
            with self.assertRaisesRegex(RuntimeError, 'admission failed'):
                live.Server('fixture-server', 'fixture-config', runtime, {}, None)
        process.send_signal.assert_called_once_with(signal.SIGINT)
        process.wait.assert_called_once_with(timeout=15)
        process.kill.assert_not_called()

    def test_live_gate_precedes_build_token_read_and_remote_writes(self):
        with patch.dict(os.environ, {'DIARY_NOTION_LIVE': '0'}), \
                patch('sys.argv', ['probe', '--live']), \
                patch.object(live.subprocess, 'run') as run:
            with self.assertRaises(SystemExit) as error:
                live.main()
            self.assertEqual(error.exception.code, 2)
        run.assert_not_called()


if __name__ == '__main__':
    unittest.main()
