#!/usr/bin/env python3
"""GPU-free wrapper contract smoke with a synthetic local runtime."""
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

WRAPPER = Path(__file__).resolve().parents[1] / "krea-image-engine.py"
spec = importlib.util.spec_from_file_location("image_wrapper", WRAPPER)
wrapper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wrapper)


class WrapperContract(unittest.TestCase):
    def test_check_never_executes_and_render_uses_supplied_values(self):
        with tempfile.TemporaryDirectory(prefix="image engine ") as temp:
            root = Path(temp)
            runtime = root / "runtime with spaces"
            binaries = runtime / ".venv/bin"
            binaries.mkdir(parents=True)
            (binaries / "python").symlink_to(sys.executable)
            cli = binaries / "mflux-generate-krea2"
            cli.write_text("synthetic fixture")
            adapter = runtime / "output_001/lora/a_adapter.safetensors"
            adapter.parent.mkdir(parents=True)
            adapter.write_bytes(b"fixture")
            # A later-sorting directory must not replace the actual adapter.
            (adapter.parent / "z_adapter.safetensors").mkdir()
            model = root / "models/revision"
            for component in ["vae", "text_encoder"]:
                (model / component).mkdir(parents=True)
                (model / component / "weights.safetensors").write_bytes(b"fixture")
            (model / "turbo.safetensors").write_bytes(b"fixture")
            (model / "tokenizer").mkdir()
            for name in ["tokenizer_config.json", "tokenizer.json"]:
                (model / "tokenizer" / name).write_text("{}")
            with patch.object(wrapper, "KREA_RUNTIME", runtime), patch.object(wrapper, "MODEL_CACHE", root / "models"), patch.object(sys, "argv", [str(WRAPPER), "--check"]), patch.object(subprocess, "call") as call:
                self.assertEqual(wrapper.main(), 0)
                call.assert_not_called()
                with patch.object(sys, "argv", [str(WRAPPER)]), patch.dict(os.environ, {"DIARY_IMAGE_PROMPT": "synthetic ; $(literal)", "DIARY_IMAGE_SEED": "4294967295", "DIARY_IMAGE_OUT": str(root / "image.png")}):
                    call.return_value = 0
                    self.assertEqual(wrapper.main(), 0)
                    args, kwargs = call.call_args
                    self.assertIn("synthetic ; $(literal)", args[0])
                    self.assertIn("4294967295", args[0])
                    self.assertEqual(kwargs["cwd"], root)
                    self.assertEqual(kwargs["env"]["HF_HUB_OFFLINE"], "1")
                    self.assertNotIn("--make-conf", args[0])
                    self.assertNotIn("--shell", args[0])
                    self.assertEqual(args[0][args[0].index("--model") + 1], str(model))
                    self.assertEqual(args[0][args[0].index("--base-model") + 1], "krea-2")
                    self.assertNotIn("--model-path", args[0])
                    self.assertEqual(args[0][args[0].index("--lora") + 1], str(adapter))
                adapter.unlink()
                self.assertEqual(wrapper.main(), 1)
                adapter.mkdir()
                self.assertEqual(wrapper.main(), 1)
                adapter.rmdir()
                adapter.write_bytes(b"fixture")
                output = root / "dangling.png"
                output.symlink_to(root / "absent.png")
                with patch.object(sys, "argv", [str(WRAPPER)]), patch.dict(os.environ, {
                    "DIARY_IMAGE_OUT": str(output), "DIARY_IMAGE_PROMPT": "synthetic", "DIARY_IMAGE_SEED": "42"
                }):
                    call.reset_mock()
                    self.assertEqual(wrapper.main(), 1)
                    call.assert_not_called()


if __name__ == "__main__":
    unittest.main()
