# P1-M33: Pluggable image engines work plan

Status: Done

Work plan for the [overview](milestone-33-image-engines-overview.md): an application port and registry, one local command adapter, and Krea 2 as the first configuration. The owner resolved Q1–Q5 on 2026-11-12; see the overview interview record. Implemented; completion evidence and live-proof limits are recorded in the overview.

## Source review (2026-11-12)

- `diary-infrastructure/src/config.rs` parses strict private **YAML**, not TOML. Unknown fields reject the whole configuration. `config/reload.rs` fingerprints fixed and guarded fields and adopts complete snapshots atomically. Add engines to this mechanism rather than maintaining a second config loader.
- `diary-infrastructure/src/agent/process.rs` owns process groups, tracks descendants, drains bounded output and reaps children. Its stderr tail is classification-only, never returned. Reuse its ownership patterns; do not inherit AI protocol parsing or inactivity deadlines that would kill a silent image renderer prematurely.
- `diary-application/src/lib.rs` owns session job admission and `ConfigReloader::reload(busy)`; `agent/run.rs` owns `CancelToken`. M33 standalone renders/probes are not session jobs, so include their active reservations in reload eligibility. M34 creation will own its parent job; avoid nested admission through `admit_agent`.
- `diary-api/src/profiles.rs` authenticates both routes, lists cached status and probes only on explicit check. The image routes should follow that distinction.
- `~/work/krea-2/run-07.sh` selects a LoRA by lexically sorted output/checkpoint names (not filesystem modification time), sources inference/private prompt variables, and invokes `07_render.sh`. It cannot take a diary prompt directly.
- `07_render.sh` uses `uv run --no-sync`, takes prompt/adapter/seeds in argv, writes `prompt.txt` and uses `--make-conf`. It produces `<base>_seed<seed>.png`; optional upscaling also writes under `x2/`. Default size is 1248 × 1664, upscale defaults on. A thin shell wrapper cannot meet a no-descendant-argv promise. Do not copy private prompt/trigger values into this repository or evidence.

## Application contract and ownership

`diary-application::image` owns `ImageEngine`, typed requests/results/errors and the registry. Infrastructure implements the port; server composition builds configured engines. Keep test fakes in test support or explicit fixture builds, never the ordinary production registry.

- Request: nonempty bounded UTF-8 prompt, seed and cancellation token. Infrastructure allocates a unique private staging directory beneath the configured private runtime root; callers cannot supply arbitrary output paths. Proposed limits: 32 KiB prompt, 32 MiB encoded PNG, dimensions 1–8192 per axis and at most 32 million pixels. Validate before allocation/spawn where possible. Seeds must round-trip through the chosen Krea backend; test its supported range before fixing the public type.
- Result: validated image handle/path, encoded size, dimensions and actual seed, owned by the caller until adoption or disposal. M33 creates no durable diary asset; M34 will adopt output through its own publication contract. Document explicit disposal on abandoned success and startup cleanup of owned staging only.
- Error: typed `Unavailable(reason)`, `Busy`, `InvalidRequest`, `Failed`, `Timeout`, `Cancelled`, `InvalidOutput` and `Io`. Never include raw stderr, prompt, filesystem path or environment. Keep OS/exit details private and bounded.
- Admission: one render process-wide across all engine IDs; reject concurrent admission with `Busy` rather than creating an unbounded queue. Hold the slot until child cleanup completes, including validation failures. A cancellation observed before success handoff wins and disposes output; later cancellation cannot revoke a transferred result.
- Snapshot: capture engine configuration and epoch at admission. Guard engine config reload while a render or explicit probe is active, using the existing reload guard. Never mix old program/model configuration with a new epoch. Cached probe results from a previous epoch cannot mark replacement configuration ready.

This registry does not coordinate other programs using the GPU. In particular, an independently launched Qwen or Krea process can contend for memory. Probe readiness means prerequisites were checked; it cannot guarantee the next render will fit memory or reproduce identical bytes on another model/runtime.

## Configuration and availability

Extend the existing schema with an optional map; absence means no engines and preserves existing configuration behavior. Owner decision (2026-11-12): approve the following YAML format, including the `krea2` ID, label, absolute program path with the `.sh` filename and 1800-second timeout. Add this section to the existing private server configuration, conventionally `config/global_config.yaml`:

```yaml
image_engines:
  krea2:
    label: "Krea 2 (local)"
    program: "/Volumes/Work_Volume/narihitos-diary-gui/scripts/run-krea2-diary.sh"
    timeout_seconds: 1800
```

Keep strict known-key, unique ID, nonempty label, absolute program path and bounded positive timeout validation. Proposed timeout range: 1–3600 seconds; default 1800 (owner-approved). Schema errors reject startup/reload as today; rejected reload retains the old complete snapshot. A valid configuration whose executable/model is missing produces an engine-specific unavailable status, leaving unrelated engines usable. No generic shell command string, arbitrary probe command or user-provided render arguments.

List returns cached `unchecked|ready|unavailable|busy` status, ID, label and typed reason, with no executable paths. Check is bounded, never renders, never loads GPU weights and never downloads anything. For generic engines check path/executable prerequisites; for Krea check the selected model/adapter/runtime prerequisites through its wrapper's fixed check mode. Check success is not successful-render evidence. Unknown IDs return 404; both routes require the existing session authorization. Slow probes run off the request executor and cannot pile up unbounded work.

M33 adds `GET /api/v1/image-engines` and `POST /api/v1/image-engines/{id}/check` only. Production render/job routes belong to M34; M33 standalone rendering uses an explicit smoke harness through the application port. Defer GUI bridge commands to M35 unless a concrete M33 consumer needs them.

## Command adapter lifecycle

1. Validate request, acquire the render slot and create an owned 0700 staging directory with restrictive umask. Choose a fresh expected output filename. Reject preexisting output and symlink traversal.
2. Spawn the configured absolute executable directly, with a private working directory and a documented minimal environment plus `DIARY_IMAGE_PROMPT`, `DIARY_IMAGE_SEED`, `DIARY_IMAGE_OUT`. No shell evaluation. Wrapper locates its runtime explicitly; secrets from the server configuration must not be inherited incidentally.
3. Drain stdout/stderr concurrently with bounded buffers and a total output limit (proposed 1 MiB). Raw content never reaches API errors, logs or evidence. Enforce the wall timeout, including silence. On cancellation, timeout, flood or failure, stop/reap the owned group and tracked descendants before staging deletion; never signal unrelated processes.
4. Exit 0 is necessary but insufficient. Open the expected regular file without following symlinks, enforce encoded size, PNG chunk bounds/order/CRC, IHDR dimensions/pixel budget and IEND completeness. Reject truncation and dimension bombs before decoding. Require successful bounded decode using an existing or macOS system decoder; signature/IHDR alone does not prove a valid PNG. Select and document that decoder during T03 feasibility review; a new dependency requires owner approval, not a hand-written image decoder.
5. On success normalize output permissions to 0600 and hand off the validated output. Remove auxiliary staging artifacts, including prompt/config sidecars. On failure/cancel remove the entire owned staging directory after process cleanup. No generic deletion outside that directory.

The configured local program is trusted code with the user's OS privileges. A private working directory does not sandbox it or prevent writes elsewhere. The adapter constrains its own reads, writes and cleanup; wrapper tests must verify its expected side effects.

## Krea wrapper decision and proof

Keep the executable shell wrapper at repository `scripts/run-krea2-diary.sh` (Q1 and the owner-approved configuration above). External project changes still require the owner's explicit authorization. Do not source private prompt files to discover a trigger. Inspect only the required inference variables and installed entry point, without printing private values.

Q5 permits local descendant argv exposure: the wrapper may use the existing CLI prompt argument. The outer command still receives its prompt via environment. Suppress raw tool output and keep temporary prompt/config files inside disposable private staging. No dedicated environment-reading Python entry point is required by this decision.

Use 1248 × 1664, `UPSCALE=0` for now, and a configurable 1800-second per-image timeout (Q2/Q3). Resolve one adapter once per invocation; do not reselect it mid-render. Record wrapper revision/hash, runtime version, selected adapter/model digest and effective dimensions/upscale/seed in private synthetic evidence. A seed alone is not a reproducibility guarantee. The engine must run from any working directory and support paths containing spaces.

Historical Q4 (2026-11-12) exempted live proof. The owner's 2026-11-13 request supersedes that exemption for the [real GPU follow-up](milestone-33-image-engines-overview.md#real-gpu-follow-up-2026-11-13) and explicitly authorizes a render and cancellation run. Retain an env-gated harness for future explicitly authorized proof: synthetic prompt, one successful render, then a distinct in-flight cancel. Assert PNG validation, process cleanup and unrelated sentinel survival; do not assert writing quality or a platform-wide GPU speed target. No default gate uses GPU, accounts or downloads. An owner-exempt live run closes only the live checklist disposition, not proof of successful Krea rendering.

## Stages and verification

| Stage | Tasks and completion evidence                                                                                                                                                                                                                                                                                                                                                                                                      |
| ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A     | T01 port/registry and T02 deterministic valid-PNG fake. Unit tests: unknown ID, empty/oversized prompt, seed bounds, admission across engines, pre-cancel, busy and slot release on all outcomes.                                                                                                                                                                                                                                  |
| B     | T03 adapter/output validation, T04 process ownership, T05 YAML/reload wiring. Test successful output, nonzero exit, missing/empty/oversized/truncated PNG, CRC/chunk/IHDR errors, undecodable IDAT, dimension bombs, symlink/directory output, spaces in paths, prompt echo on stderr, output flood, descendant survival, timeout/cancel races, cleanup and sentinel survival. Confirm decoder feasibility before implementing it. |
| B     | Config tests: omitted section, multiple engines, duplicate IDs/unknown keys/invalid timeout, per-engine missing executable, rejected reload preserves snapshot, active-render/probe reload refusal and stale probe result rejection.                                                                                                                                                                                               |
| C     | T06 wrapper contract tests, T07 authenticated list/check routes and T08 authorized live smoke or explicit exemption. Verify checks never render; list never spawns; invalid session and unknown ID fail; launch from outside checkout. Stub wrapper tests do not prove Krea output.                                                                                                                                                |
| D     | T09 maintained architecture/config/API/user-guide docs and T10 full gate plus harness tests. Keep synthetic and live evidence separate. Record exact commands, revision, results and limits before checking overview items.                                                                                                                                                                                                        |

No new dependency or build tool is authorized. The affected owner decisions and completion checklist are recorded; implementation remains separate work. Update M34/M35 references if the finalized port differs from their assumptions. After M35, M32 must qualify newly built ordinary artifacts including these features; existing release evidence does not cover them.
