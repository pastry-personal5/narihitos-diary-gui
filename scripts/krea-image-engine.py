#!/usr/bin/python3
"""Repository command-engine wrapper. Configure KREA_RUNTIME below in a private copy.

The runtime is trusted. No private prompt defaults are read. --check only inspects
local files; it never imports MLX, loads weights, downloads, or renders.
"""
import glob
import os
from pathlib import Path
import subprocess
import sys

# Absolute owner runtime; edit a private copy to select another installation.
KREA_RUNTIME = Path.home() / "work/krea-2"
MODEL_CACHE = Path("/Volumes/Reserved_For_Future_Volume/.cache/huggingface/hub/models--krea--Krea-2-Turbo/snapshots")


def selection():
    """Select the same local prerequisites for checks, rendering and provenance."""
    runtime = KREA_RUNTIME
    adapters = sorted(p for p in glob.glob(str(runtime / "output_*/lora/*_adapter.safetensors"))
                      if Path(p).is_file())
    python = runtime / ".venv/bin/python"
    cli = runtime / ".venv/bin/mflux-generate-krea2"
    snapshots = sorted(MODEL_CACHE.glob("*"))
    models = [p for p in snapshots if (p / "turbo.safetensors").is_file()
              and any((p / "vae").glob("*.safetensors"))
              and any((p / "text_encoder").glob("*.safetensors"))
              and (p / "tokenizer/tokenizer_config.json").is_file()
              and (p / "tokenizer/tokenizer.json").is_file()]
    if not models or not adapters or not python.is_file() or not os.access(python, os.X_OK) or not cli.is_file():
        raise ValueError("local prerequisites unavailable")
    return python, cli, models[-1], Path(adapters[-1])


def main():
    try:
        python, cli, model, adapter = selection()
    except ValueError:
        return 1
    if sys.argv[1:] == ["--check"]:
        return 0
    if sys.argv[1:]:
        return 1
    output = Path(os.environ["DIARY_IMAGE_OUT"])
    if not output.is_absolute() or os.path.lexists(output):
        return 1
    # Factory portrait size; no upscaler, extra LoRA or private trigger.
    command = [str(python), str(cli), "--prompt", os.environ["DIARY_IMAGE_PROMPT"],
               "--model", str(model), "--base-model", "krea-2", "--lora", str(adapter), "1.0", "--width", "624", "--height", "832",
               "--steps", "8", "--scheduler", "euler", "--seed",
               os.environ["DIARY_IMAGE_SEED"], "--output", str(output)]
    return subprocess.call(command, cwd=output.parent, stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL,
                           env={"HOME": str(Path.home()), "PATH": "/usr/bin:/bin", "HF_HUB_OFFLINE": "1", "TRANSFORMERS_OFFLINE": "1"})


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, KeyError, ValueError):
        sys.exit(1)
