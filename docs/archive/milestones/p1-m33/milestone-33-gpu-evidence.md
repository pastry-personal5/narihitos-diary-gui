# P1-M33: Real GPU image-engine evidence

Status: Archived

Owner-authorized follow-up executed on 2026-11-13 (Asia/Seoul; raw timestamps
are UTC on 2026-11-12). This supersedes the earlier live-proof exemption for
this follow-up. The pre-existing uncommitted implementation was preserved.

## Command and scope

```sh
DIARY_IMAGE_LIVE=1 python3 scripts/smoke/image-generation-live.py
```

The script builds `diary-infrastructure`'s `image_engine_live` example, discovers
its exact Cargo artifact, then launches outside the checkout working directory.
The application registry checks and renders using the production `CommandEngine`
and approved `scripts/run-krea2-diary.sh`, installed mflux CLI and cached local
Krea 2 Turbo weights. Both model-loading environment flags enforce offline use.
No dependency or build-tool change, model download, diary mutation or GUI feature
was needed.

## Observed successful run

- GPU: Apple M5 Max, 128 GiB unified memory; Metal available and MLX default device `Device(gpu, 0)`.
- Runtime metadata: MLX 0.32.4 and mflux 0.23.0.
- Settings: synthetic blue ceramic cup prompt, seed 42, 1248 × 1664, eight Euler steps, LoRA scale 1.0, no upscaling.
- Render: 144.484 seconds, 1,873,430 encoded bytes. The production validator checked PNG bounds/CRC and decoded pixels using ImageIO before the artifact was copied.
- Real GPU execution: owned renderer PID 27977 was sampled executing MLX `eval_gpu` through the AGX Metal driver. All 143 accelerator samples are retained privately; device utilization reached 100%, which is a machine-wide observation. The driver's last-submission PID was stale, so it is supplemental evidence only.
- Separate cancellation: requested after ten seconds with real wrapper/renderer descendants still alive; returned `Cancelled` in 10.295 seconds. PIDs 31501, 31507 and 31541 all disappeared within the bounded cleanup check. The unrelated sentinel process survived and was then explicitly stopped and reaped by the harness.
- Staging directories were empty after successful-result disposal and cancellation.
- PNG SHA-256: `fa8a0d240bcf4d29851df8b789065601ce82f2f367b01aa678a753d9e433c088`.

The [report](evidence/report.json) records source/binary, selected adapter and all
local model component hashes, settings, runtime metadata, timings and assertions.
The [PNG](evidence/krea2-cup.png) and [GPU stack excerpt](evidence/gpu-stack-excerpt.txt)
are retained alongside it. The complete private raw report, GPU samples, stack,
and build/run logs are in `tmp/krea GPU evidence rz5bxrs3/`; the generated
artifact is intentionally retained outside that ignored directory as well.
The Git revision alone is insufficient because the worktree was dirty; the
report includes the actual harness/wrapper/example/binary hashes.

## Findings corrected during execution

The initial preflight found that the wrapper assumed `~/.cache/huggingface`,
while this installation uses the existing cache on another volume. After that
was corrected, the real CLI rejected `--model-path`; the installed CLI requires
`--model <snapshot> --base-model krea-2`. The wrapper contract regression now
checks those arguments. Neither stub check had established real render success.

The initial cancellation assertion checked PID absence immediately after process
group termination and caught an exiting grandchild before OS reaping completed.
The final harness waits at most five seconds for all observed owned PIDs to
disappear and still fails if one survives. The successful run used this bounded
check. Earlier attempts are retained under the other private `tmp/krea GPU evidence *`
directories and are not counted as successful end-to-end runs.

## Validation and limits

The GPU-free wrapper contract passes, the opt-in skip path passes, and Python
harness regression tests pass (13 tests, with process visibility enabled).
Final `scripts/gate.sh`: G01–G07 PASS, log directory `run.GTBGTJ` (local socket/process visibility enabled, GPU inference stopped). Evidence hashes, actual source hashes, local links and whitespace checks also pass. Earlier full-gate
attempts observed a loopback fixture-port race and a frontend Settings test race;
one intervening run passed all seven lanes. No unrelated product behavior was
changed to accommodate those failures.

This proves standalone engine integration, real GPU rendering, PNG decoding and
in-flight process cancellation. It does not cover diary creation, a GUI workflow,
image-quality acceptance, or cancellation at a particular diffusion step. It
makes no guarantee of identical bytes across runtime/model changes or of
exclusive GPU use. The default repository gate does not render or download.

## Subsequent implementation review

The owner-requested [implementation review](milestone-33-implementation-review.md)
records stricter PNG checks, interruption-safe harness cleanup and a new real GPU
run with separate source/binary hashes. Artifacts on this page remain unchanged
and describe the earlier implementation.
