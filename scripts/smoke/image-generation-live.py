#!/usr/bin/env python3
"""Opt-in real Krea GPU render/cancel proof; stdlib only, offline, no user data.

DIARY_IMAGE_LIVE=1 python3 scripts/smoke/image-generation-live.py
Evidence is retained in a new private directory, printed on startup.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import signal
import subprocess
import tempfile
import time
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def gpu_sample():
    data = subprocess.check_output(
        ["/usr/sbin/ioreg", "-a", "-r", "-c", "AGXAccelerator"], timeout=10)
    devices = plistlib.loads(data)
    return [{"device": device.get("model"),
             "statistics": device.get("PerformanceStatistics", {}),
             "submission": device.get("AGCInfo", {})} for device in devices]


def process_table():
    rows = subprocess.check_output(
        ["/bin/ps", "-axo", "pid=,ppid=,lstart="], text=True, timeout=10)
    return {int(pid): (int(parent), started) for line in rows.splitlines()
            for pid, parent, started in [line.strip().split(None, 2)]}


def descendants(pid, table=None):
    table = process_table() if table is None else table
    owned = {pid}
    while True:
        expanded = owned | {child for child, (parent, _) in table.items() if parent in owned}
        if expanded == owned:
            return sorted(owned)
        owned = expanded


def stop_child(child):
    """Cancel via stdin first; the Rust owner must reap its own process groups."""
    try:
        table = process_table() if child.poll() is None else {}
        owned = {pid: table[pid][1] for pid in descendants(child.pid, table) if pid in table}
    except (OSError, subprocess.SubprocessError):
        owned = {}
    if child.stdin is not None:
        child.stdin.close()
    try:
        child.wait(timeout=15)
        return
    except subprocess.TimeoutExpired:
        pass
    # Last resort if the controller itself is stuck: signal observed descendants
    # even though the command supervisor places them in a different group.
    for sig in (signal.SIGTERM, signal.SIGKILL):
        current = process_table()
        for pid, started in reversed(list(owned.items())):
            if pid in current and current[pid][1] == started:
                try:
                    os.kill(pid, sig)
                except ProcessLookupError:
                    pass
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            continue
        # A controller exit does not mean its separately grouped children exited.
        if sig == signal.SIGTERM:
            time.sleep(0.1)
    child.wait(timeout=5)


def interrupted(signum, _frame):
    raise SystemExit(128 + signum)


def main():
    if os.environ.get("DIARY_IMAGE_LIVE") != "1":
        print("image generation live: skipped (set DIARY_IMAGE_LIVE=1)")
        return
    os.umask(0o077)
    evidence = Path(tempfile.mkdtemp(prefix="krea GPU evidence ", dir=ROOT / "tmp"))
    print(f"Evidence: {evidence}", flush=True)
    report = {"status": "failed", "started_utc": datetime.now(timezone.utc).isoformat()}
    previous_term = signal.signal(signal.SIGTERM, interrupted)
    try:
        wrapper = ROOT / "scripts/krea-image-engine.py"
        spec = importlib.util.spec_from_file_location("krea_wrapper", wrapper)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        selected = module.selection()
        python, cli, model, adapter = selected
        # Same clean environment as the production wrapper; fail without Metal.
        env = {"HOME": str(Path.home()), "PATH": "/usr/bin:/bin",
               "HF_HUB_OFFLINE": "1", "TRANSFORMERS_OFFLINE": "1"}
        probe = subprocess.check_output([str(python), "-c", '''
import mlx.core as mx
import importlib.metadata as metadata
import json
assert mx.metal.is_available() and mx.default_device() == mx.gpu
a = mx.ones((1024, 1024))
mx.eval(a @ a)
mx.synchronize()
print(json.dumps({"device": mx.device_info(), "default_device": str(mx.default_device()),
"mlx": metadata.version("mlx"), "mflux": metadata.version("mflux"),
"gpu_probe_peak_bytes": mx.get_peak_memory()}))
'''], env=env, text=True, timeout=60)
        report["gpu_runtime"] = json.loads(probe)
        files = [wrapper, ROOT / "scripts/run-krea2-diary.sh", cli, adapter]
        files += [Path(__file__).resolve(), ROOT / "src-tauri/crates/diary-infrastructure/examples/image_engine_live.rs"]
        files += sorted(p for p in model.rglob("*") if p.is_file())
        print("Hashing selected local model, adapter and wrapper...", flush=True)
        report["provenance"] = [{"path": str(p), "bytes": p.stat().st_size,
                                 "sha256": digest(p)} for p in files]
        report["git_head"] = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        report["git_status"] = subprocess.check_output(
            ["git", "status", "--short"], cwd=ROOT, text=True).splitlines()
        report["settings"] = {"seed": 42, "width": 1248, "height": 1664,
                              "steps": 8, "scheduler": "euler", "upscale": False,
                              "lora_scale": 1.0, "offline": True,
                              "prompt": "A blue ceramic cup on a plain wooden table, soft daylight, still life photograph"}
        build = ["cargo", "build", "--manifest-path", str(ROOT / "src-tauri/Cargo.toml"),
                 "-p", "diary-infrastructure", "--example", "image_engine_live"]
        report["build_command"] = build
        with (evidence / "build.log").open("w") as log:
            subprocess.run(build, cwd=ROOT, stdout=log, stderr=log, check=True)
        # Ask Cargo for the exact binary path, including configured target directories.
        artifacts = subprocess.check_output(build + ["--message-format=json"], cwd=ROOT, text=True)
        executable = next(item["executable"] for line in artifacts.splitlines()
                          if (item := json.loads(line)).get("executable")
                          and item.get("target", {}).get("name") == "image_engine_live")
        report["binary_sha256"] = digest(Path(executable))
        live_env = dict(os.environ, DIARY_IMAGE_EVIDENCE=str(evidence),
                        DIARY_IMAGE_LIVE_PROGRAM=str(ROOT / "scripts/run-krea2-diary.sh"))
        samples = []
        gpu_stack = None
        next_stack_sample = 30
        with (evidence / "run.log").open("w") as log:
            child = subprocess.Popen([executable], cwd=evidence, env=live_env,
                                     stdin=subprocess.PIPE, stdout=log, stderr=log, start_new_session=True)
            start = time.monotonic()
            try:
                while child.poll() is None:
                    owned = descendants(child.pid)
                    samples.append({"elapsed_seconds": round(time.monotonic() - start, 3),
                                    "owned_pids": owned, "devices": gpu_sample()})
                    if gpu_stack is None and time.monotonic() - start >= next_stack_sample:
                        # Driver-wide submission counters can be stale. A stack of
                        # our actual renderer establishes per-process Metal use.
                        rows = subprocess.check_output(
                            ["/bin/ps", "-axo", "pid=,comm="], text=True, timeout=10)
                        for row in rows.splitlines():
                            pid, executable_name = row.strip().split(None, 1)
                            if int(pid) in owned and executable_name == str(module.KREA_RUNTIME / ".venv/bin/python"):
                                stack = evidence / f"renderer-stack-{pid}.txt"
                                sampled = subprocess.run(["/usr/bin/sample", pid, "1", "-file", str(stack)],
                                                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                                         timeout=30)
                                if sampled.returncode == 0:
                                    trace = stack.read_text()
                                    if "mlx::core::" in trace and "eval_gpu(" in trace and "AGXMetal" in trace:
                                        gpu_stack = {"pid": int(pid), "file": stack.name,
                                                     "sha256": digest(stack)}
                                        break
                        next_stack_sample += 30
                    if time.monotonic() - start > 1900:
                        raise TimeoutError("live proof exceeded render/cancel budget")
                    time.sleep(1)
            finally:
                try:
                    stop_child(child)
                finally:
                    (evidence / "gpu-samples.json").write_text(json.dumps(samples, indent=2) + "\n")
            assert child.returncode == 0, "render/cancel failed; inspect private run.log"
        report["adapter"] = json.loads((evidence / "adapter.json").read_text())
        report["image_sha256"] = digest(evidence / "image.png")
        report["owned_gpu_submission_samples"] = sum(
            any(device["submission"].get("fLastSubmissionPID") in sample["owned_pids"]
                for device in sample["devices"]) for sample in samples)
        report["renderer_gpu_stack"] = gpu_stack
        assert gpu_stack, "no MLX GPU evaluation/Metal stack captured from the owned renderer"
        assert module.selection() == selected, "runtime selection changed during the live run"
        for artifact in report["provenance"]:
            assert digest(Path(artifact["path"])) == artifact["sha256"], "provenance changed during the live run"
        report["provenance_unchanged_after_run"] = True
        report["status"] = "passed"
        print("PASS: real GPU render, validated PNG, in-flight cancel and cleanup", flush=True)
    except BaseException as error:
        report["failure"] = {"type": type(error).__name__, "message": str(error)}
        raise
    finally:
        report["finished_utc"] = datetime.now(timezone.utc).isoformat()
        (evidence / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        signal.signal(signal.SIGTERM, previous_term)


if __name__ == "__main__":
    (ROOT / "tmp").mkdir(exist_ok=True)
    main()
