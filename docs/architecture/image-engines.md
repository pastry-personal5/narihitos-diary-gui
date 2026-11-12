# Image engines

Status: Active

Standalone local PNG generation uses an application port and a trusted command adapter. Diary creation and GUI integration follow separately.

`diary-application::image` defines requests, disposable results, typed errors and a registry. A process-wide reservation rejects concurrent renders across registry instances. A registry locks its configuration through a render or probe; reload returns Busy rather than replacing that snapshot. Listing reads cached views and starts no process. Probes never receive a prompt. External GPU users are outside this reservation.

## Configuration

The optional private YAML map configures multiple independent engines:

```yaml
image_engines:
  krea2:
    label: "Krea 2 (local)"
    program: "/Volumes/Work_Volume/narihitos-diary-gui/scripts/run-krea2-diary.sh"
    timeout_seconds: 1800
    trigger: "v02realphoto"
    prompt_directives:
      - "photo of a woman in her early 20s"
```

IDs use ASCII letters, digits, underscores and hyphens, up to 64 bytes. Labels are nonempty and bounded to 128 bytes. Programs must be absolute paths. Timeouts are 1–3600 seconds, default 1800. Optional trigger defaults to empty (up to 1024 bytes, no NUL/newline); prompt_directives defaults to an empty list (up to 64 nonempty directives, each at most 4096 bytes and NUL-free). Schema errors reject the entire snapshot. Missing programs affect only that engine's availability.

## Command contract

Programs accept no render arguments. The adapter supplies only `DIARY_IMAGE_PROMPT`, `DIARY_IMAGE_SEED` (unsigned 32-bit decimal) and `DIARY_IMAGE_OUT` (absolute PNG output). No shell evaluates these values. Programs must use absolute runtime paths; the working directory is disposable 0700 staging. `--check` must inspect local prerequisites without rendering, loading weights or downloading; it is bounded to five seconds. The executable is trusted code with the owner's filesystem privileges.

The existing process supervisor owns the process group, bounded pipe draining, descendant tracking, cancellation, timeout and reaping. Render output is bounded to 1 MiB and discarded. Exit zero is required. The adapter opens output without following symlinks, bounds encoded size to 32 MiB, checks chunk bounds, CRC, chunk names/reserved bits, supported IHDR fields and palette presence/order, rejects unknown critical chunks, requires IHDR/IDAT/IEND and limits dimensions to 8192 per axis and 32 million pixels. macOS ImageIO supplies decode validation without a new dependency. Validation requests [immediate decoding](https://developer.apple.com/documentation/imageio/kcgimagesourceshouldcacheimmediately) and accesses pixel data rather than accepting a lazy image handle. Output becomes 0600; auxiliary staging files are removed. Dropping a result disposes its directory; consumers must copy/adopt the PNG before dropping it. Failures dispose staging after supervision returns. No durable diary asset is created.

## Discovery

Authenticated `GET /api/v1/image-engines` returns `image_engines` containing ID, label, cached status and a content-free reason. `POST /api/v1/image-engines/{id}/check` explicitly refreshes one entry. Unknown IDs return 404 and concurrent probes return 409. No render route is exposed.

## Krea wrapper and evidence limits

`scripts/run-krea2-diary.sh` sets a private umask and invokes the repository Python wrapper, which targets the owner's local `~/work/krea-2` runtime and lexically selects one adapter. It calls the installed CLI directly with a supplied prompt, 1248 × 1664, eight steps, Euler, LoRA scale 1.0 and no upscaler. It reads no private prompt defaults. Local descendant argv may expose the prompt, as approved. The wrapper inspects local native Turbo model snapshots and forces offline model use. It does not prove model integrity or sufficient GPU memory.

The owner authorized real GPU proof on 2026-11-13, superseding the earlier exemption. Stub processes validate the adapter contract; they do not establish Krea rendering success. Startup cleanup removes only image-run directories beneath the locked workspace’s private image staging root. The ignored `local_render_and_cancel` test requires `DIARY_IMAGE_LIVE=1` and an absolute `DIARY_IMAGE_LIVE_PROGRAM`; it is available for future explicitly authorized proof. The end-to-end harness is `DIARY_IMAGE_LIVE=1 python3 scripts/smoke/image-generation-live.py`. It builds a standalone Rust example, checks and renders through the application registry and production command adapter, validates the PNG with ImageIO, copies it into a new private evidence directory under `tmp/`, then cancels a separate real renderer after ten seconds. It asserts owned-process exit, empty staging after both outcomes and unrelated sentinel-process survival. No render API or GUI exists in this milestone.

The harness requires the installed runtime and model cache, real Metal access and process-table visibility. It uses the same offline environment as the production wrapper, checks MLX's GPU device, samples Metal accelerator statistics and requires a stack sample from the owned renderer showing MLX GPU evaluation and the Metal driver. Machine-wide utilization may include unrelated GPU users; the driver submission PID can be stale and is recorded only as supplemental evidence. It records runtime versions, source/binary/model/adapter hashes, dimensions, seed, timing and a retained PNG; failed runs retain diagnostic evidence and exit nonzero. GPU/model weights are never downloaded. The wrapper selects the owner's existing cache under `/Volumes/Reserved_For_Future_Volume/.cache/huggingface` and passes a local snapshot via the installed CLI's `--model` and `--base-model krea-2` options. The default gate does not run this live harness.

The live example's control stdin remains open while evidence is collected.
Interruption, timeout or telemetry failure closes it, which requests cancellation
through the real command supervisor before the harness waits for exit. This is
necessary because renderer processes have their own supervised group. A bounded
emergency fallback checks observed process start times before signalling any
remaining descendants. The example also owns sentinel cleanup on panic. Failure
reports retain exception type/message. Wrapper and harness share local runtime
selection, and live proof requires matching selection and content hashes before
and after inference. `scripts/tests/test_image_generation_live.py` verifies
interruption cleanup with the real Rust owner and a fake renderer, without GPU
weights or accounts. See the [implementation review](../archive/milestones/p1-m33/milestone-33-implementation-review.md).

Illustrated creation captures these prompt settings before deriving scenes, then prefixes the trigger in the application before rendering; the command adapter receives the complete prompt and does not prefix it. `illustrated.max_photos` defaults to 4 (1–99); the agent combines related theme groups to meet that cap. `illustrated.engine` defaults to the sole configured engine; multiple engines require explicit selection. Illustrated settings changes require restart.
