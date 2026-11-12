# P1-M33: Pluggable image engines overview

Status: Done

Add an image-engine port with a registry of configured engines, and prove it standalone with a fake engine and the owner's local Krea 2 engine. No diary feature and no GUI yet. First of three stages toward the Illustrated diary ([P1-M34](../../../phase-1/milestone-34-illustrated-diary-overview.md), [P1-M35](../../../phase-1/milestone-35-illustrated-diary-gui-overview.md)).

## Entry conditions

Owner request 2026-11-12: P1-M33, P1-M34 and P1-M35 run before P1-M32 closes. P1-M32 stays Active; its release checklist is not changed by this plan.

## Owner decisions (2026-11-12)

- **O1 - Feature:** when a day has an Authentic diary, the owner can create another version of that day's diary: one Markdown file plus a dedicated directory of `.png` photos.
- **O2 - Text:** an AI Agent rewrites the Authentic entry. The result is a separate asset; the Authentic entry is never modified.
- **O3 - Photo prompts:** the AI Agent derives them.
- **O4 - Count:** one version per day.
- **O5 - Engines:** image generation is pluggable and several engines can be configured. One engine is the owner's `~/work/krea-2` (see `run-07.sh`).
- **O6 - Stages:** engine first. M33 engines, M34 format and creation, M35 GUI.
- **O7 - Configuration:** owner approved the YAML format and exact `.sh` wrapper path in the [work plan](milestone-33-image-engines-architecture.md#configuration-and-availability) on 2026-11-12: engine ID `krea2`, label `Krea 2 (local)`, program `/Volumes/Work_Volume/narihitos-diary-gui/scripts/run-krea2-diary.sh`, timeout 1800 seconds.

## Scope

- Stage A: `ImageEngine` port, request/result/error types, engine registry and a fake engine in `diary-application`.
- Stage B: one generic **command engine** in `diary-infrastructure` (runs a configured local program, cancellable, time-bounded, validates the PNG it produced) and the existing private YAML configuration (`image_engines.<id>`).
- Stage C: Krea 2 wired as a command engine through a small wrapper script; `GET /api/v1/image-engines` and a check route; env-gated live render.
- Stage D: tests, smoke leg, docs, full gate.

Out of scope: any diary kind, file format or agent prompt (P1-M34); any GUI or Settings card (P1-M35); remote/cloud engines; upscaler or LoRA management; image editing; non-PNG output; running more than one render at a time.

## Review completion checklist (2026-11-12)

This review changes planning documents only; implementation and live rendering
are separate work. Existing uncommitted M33–M35 planning is preserved.

- [x] P01 - Compare the plan with current YAML parsing/reload, profile routes and child-process ownership; document corrections and verifiable failure cases.
- [x] P02 - Interview the owner on wrapper location, render defaults, live proof and prompt exposure; record answers or explicitly pending decisions without inventing approval.
- [x] P03 - Check formatting, local links and whitespace, and record the full repository gate result.

Review evidence: inspected `config.rs`, `config/reload.rs`, `profiles.rs`,
`agent/process.rs`, `run-07.sh` and relevant `07_render.sh` rendering/output
commands on 2026-11-12. Corrections and the test matrix are recorded in the
[work plan](milestone-33-image-engines-architecture.md#source-review-2026-11-12).
No implementation or live render was performed. The owner answered Q1–Q5 on
2026-11-12; the decisions are recorded below. Proposed technical limits
in the work plan are design proposals, not recorded owner decisions.

Validation evidence (2026-11-12): M33 local link targets, changed-document
Prettier and `git diff --check` pass. Full `scripts/gate.sh` G01–G07 passes
with local socket access (`run.zU7lMu`). The initial sandboxed run
(`run.p1UlQs`) failed G03 because bridge listeners received `PermissionDenied`;
the successful rerun supersedes that result. Final documentation evidence was
checked again after recording these results. These checks validate the plan
revision only, not any engine implementation.

## Interview decisions (2026-11-12)

The owner answered all five questions. No owner interview item remains open for M33.

- **Q1 - Krea wrapper location:** keep the wrapper under `scripts/` in this repository. The external runtime remains configured by absolute path.
- **Q2 - Upscale:** disable upscaling for now (`UPSCALE=0`); retain the proposed default size of 1248 × 1664.
- **Q3 - Time budget:** default to 30 minutes per image (1800 seconds), configurable per engine.
- **Q4 - Live proof:** exempt at the owner's request. No additional reason was supplied. Do not run the live GPU render or cancellation test; successful Krea rendering and real-runtime cancellation remain unverified.
- **Q5 - Prompt exposure:** allow prompts in local descendant command arguments. The outer adapter still passes the prompt by environment; app logs/API errors must not contain prompt text, and temporary prompt/config sidecars stay inside disposable private staging.

## Completion checklist

Check only when evidence exists.

### Plan review

- [x] R01 - Owner answered Q1-Q5; answers recorded here, in [decisions](../../../decisions.md) and the [changelog](../../../phase-1/changelog.md).
- [x] R02 - Work plan re-read against current code (config loader, job coordinator, cancel token, child-process handling in the agent adapters); corrections recorded in the [work plan](milestone-33-image-engines-architecture.md).

### Stage A - port and registry

- [x] C01 - `ImageEngine` port, `RenderRequest`, `Rendered`, typed `EngineError` and a registry keyed by engine ID exist in `diary-application` with no infrastructure dependency. Evidence: unit tests; dependency-enforcement test still passes.
- [x] C02 - Fake engine renders a deterministic valid PNG and supports scripted delay, failure and cancel. Evidence: unit tests.

### Stage B - command engine and config

- [x] C03 - Command engine passes prompt, seed and output path by environment at the adapter boundary (no shell evaluation; descendant argv policy follows Q5; no prompt in logs), uses an application-owned private staging directory and returns success only for exit 0 plus a bounded, regular, non-symlink PNG that passes structural and decode validation. The trusted configured executable is not sandboxed; do not claim arbitrary filesystem writes are prevented. Evidence: stub-process and output-validation tests in the work-plan matrix, including malformed PNG, symlink and output-flood failures.
- [x] C04 - Cancel and timeout stop and reap owned children before deleting only the owned staging directory; release the render slot on every terminal path. Evidence: sleeping/descendant stubs, timeout and cancel/success races; unrelated sentinel survives.
- [x] C05 - `[image_engines.<id>]` YAML config preserves existing configs when omitted, rejects malformed schema atomically, and reports missing runtime prerequisites per engine without disabling others. Guarded reload cannot replace a running render snapshot. Evidence: config tests.

### Stage C - Krea 2 and API

- [x] C06 - Krea 2 wrapper renders one PNG for a given prompt, seed and output path. Evidence: wrapper contract tests and an agent-run synthetic smoke (or explicitly exempt live disposition); record wrapper/runtime/model identity. Follow the recorded Q1/Q5 decisions.
- [x] C07 - `GET /api/v1/image-engines` lists ID, label and availability with a typed reason; `POST /api/v1/image-engines/{id}/check` re-probes. Evidence: authenticated API tests, unknown-ID handling and proof that list/check never renders or consumes the GPU.
- [x] C08 - Live proof disposition: initially owner-exempt under Q4 (2026-11-12); superseded by the authorized real GPU render/cancel follow-up below, which passed on 2026-11-13.

### Stage D - proof and docs

- [x] C09 - `docs/architecture/` describes the port, the command-engine contract and the config section; `docs/decisions.md`, changelog, `docs/README.md` and the user guide's configuration part are updated.
- [x] C11 - Process-wide admission rejects a concurrent render with `Busy`, cancellation before admission never spawns, and failure/timeout/cancel free capacity. Evidence: registry concurrency tests. GPU coordination with separately launched Qwen/Krea processes is an explicit limitation.
- [x] C10 - `scripts/gate.sh` passes (all seven lanes).

## Implementation verification

- [x] I01 - Port, adapter, configuration and authenticated status API have automated coverage.
- [x] I02 - Full gate passes; authorized live GPU follow-up also passes (see below).

## Implementation evidence (2026-11-13, before GPU follow-up)

- Application image tests cover a deterministic valid-PNG fake, request bounds, failure release, global concurrent admission across registries, reload/probe refusal during rendering, pre-cancel and cancellation winning a successful handoff.
- Infrastructure tests cover actual PNG decoding and pixel access, malformed CRC/chunks/compressed data, dimension bounds, symlink/directory/oversized/truncated output, paths containing spaces, prompt-bearing stderr, timeout, cancellation, output flood and disposal with a surviving sentinel. Process ownership uses the existing supervisor and its `tests/agent_process.rs` descendant/escape regressions.
- Strict schema and reload tests cover omitted configuration, defaults, duplicate/unknown fields, invalid limits, rejected snapshots, engine-specific missing prerequisites and reset of stale cached availability. The registry lock keeps the admitted engine snapshot stable through validation and cleanup.
- Authenticated image API tests prove list does not probe/render, explicit check probes only, and unknown IDs fail. The ordinary built-server lifecycle smoke passes with local socket access. `python3 scripts/smoke/image-engines.py` passes without GPU use.
- Wrapper SHA-256: Python adapter `a2eb81a9042aa64c32a1e3f121f6c3f11d387f38f28911a07e37d4559f8bb1d2`; approved shell entry `35338c675cc45aaa1057961da828baeb0052592dd107fbd186ce70cb8cc8e942`. External runtime inspected at `~/work/krea-2`; no installed mflux dist-info metadata was available to establish a version. No model digest, real render or real-runtime cancellation is claimed under Q4. The ignored env-gated harness remains available for future authorized proof.
- Maintained contract: [image engines](../../../architecture/image-engines.md). No dependency, build-tool, diary-format or GUI change.
- Final `scripts/gate.sh`: G01–G07 PASS, log `run.xKXuN1` (local socket access enabled). Real GPU proof remains exempt.

## Real GPU follow-up (2026-11-13)

Owner explicitly requested implementing and running an end-to-end Krea 2 test on
a real GPU and gathering evidence. This supersedes Q4's live-proof exemption
for this follow-up. Existing implementation work is preserved.

- [x] L01 - Add an opt-in standalone script through the application registry and production command adapter/wrapper; retain a validated PNG and runtime/model/wrapper provenance.
- [x] L02 - Execute one real Metal GPU render and a separate in-flight cancellation; verify staging/process cleanup and unrelated sentinel survival, with machine-readable evidence.
- [x] L03 - Record exact commands, results and limitations, update maintained documentation, and pass the full gate.

Live evidence: [execution record](milestone-33-gpu-evidence.md),
[machine-readable report](evidence/report.json), [generated PNG](evidence/krea2-cup.png)
and [owned-renderer Metal stack excerpt](evidence/gpu-stack-excerpt.txt).

Follow-up validation: `scripts/gate.sh` G01–G07 PASS (`run.GTBGTJ`),
GPU-free wrapper contract PASS, Python harness regressions 13/13 PASS,
opt-in skip PASS, evidence/source hashes and local links PASS. The
[execution record](milestone-33-gpu-evidence.md) records failed initial attempts,
fixes, exact settings and remaining scope limits.

## Implementation review follow-up (2026-11-13)

Owner requested reviewing and improving the implementation after the real GPU
proof. Preserve existing artifacts as evidence of their original source hashes.

- [x] V01 - Review port/admission, configuration reload, API authorization, PNG validation, wrapper and live harness; record concrete findings and regressions.
- [x] V02 - Reject malformed PNG header/chunk forms before system decoding and add real-byte regression coverage.
- [x] V03 - Make live-harness interruption/telemetry-failure cleanup cancel the separately supervised renderer; retain failure diagnostics and test cleanup without a GPU.
- [x] V04 - Run the affected regressions and full gate, update the contract and evidence, and distinguish historical GPU proof from changed-source validation.

Findings, fixes and regression evidence are recorded in the
[implementation review](milestone-33-implementation-review.md).

Review completion: all four findings fixed; real GPU render/cancel PASS
(`tmp/krea GPU evidence nkkojta8/`), all 15 Python regressions PASS, affected Rust
image/API tests PASS, final full gate G01–G07 PASS (`run.R4C6Zo`). Separate
[review artifacts](evidence/review/report.json) retain the new source/binary
provenance without changing the original run's artifacts.
