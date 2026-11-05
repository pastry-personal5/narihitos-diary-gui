# P1-M3: Scaffolding overview

Status: Done (archived)

Completed 2026-11-05; evidence below is retained as the completion record.

Create a buildable, independently launched server/Tauri client skeleton from the completed [P1-M2 architecture](../p1-m2/milestone-02-architecture-overview.md). This plan covers scaffolding and synthetic transport evidence; completing the plan review does not complete the milestone or authorize dependency changes.

## Entry conditions

P1-M1 and P1-M2 are Done; P1-M3 is Active following owner dependency/tool approval. Read the [work plan](milestone-03-scaffolding-architecture.md), its contract links and the [handoff evidence register](../../../phase-1/implementation-handoff.md#capability-evidence-register) before implementation. No owner product question currently blocks scaffolding.

The first implementation prerequisite is approval of the concrete dependency/build-tool inventory in S01. Prepare that review before asking for approval; do not infer approval from the fixed stack or architecture recommendations. The owner subsequently approved additional dependencies/build tools in the implementation conversation; the [setup inventory](milestone-03-setup.md) records selections and remaining license/runtime evidence.

## Scope

| Included in P1-M3                                                                                                                     | Deferred capability and owner milestone                                                                                                                     |
| ------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Approved tool setup, inward Cargo workspace, two executables, frontend build/type checks and test infrastructure                      | Feature algorithms/adapters remain in their consuming milestones; do not prebuild every future port or add unused dependencies                              |
| Startup config validation, OS lock exclusion, port selection, private discovery, authentication, one-client lease and redacted status | Workspace/SQLite/durable operations: P1-M5; Authentic persistence/recovery: P1-M6; client autosave and Create Empty: P1-M7                                  |
| Narrow typed Tauri bridge, bounded HTTP/SSE and deterministic in-memory fake jobs in a test harness                                   | Sources/imports/navigation: P1-M9–P1-M11; source saves/Trash: P1-M14/P1-M15; agent/AI/review features: P1-M16–P1-M24; exports: P1-M25–P1-M27                |
| Text-labeled Standard Layout, dedicated CodeMirror module and explicitly synthetic editor state                                       | Logging: P1-M4; preferences/reload: P1-M8; full layout: P1-M11; shortcuts: P1-M12; native symbols/interaction: P1-M13; complete AI activity/results: P1-M23 |
| Version/protocol checks, full code gate and packaged synthetic server/client smoke evidence                                           | Personal release and complete workflow/platform qualification: P1-M28                                                                                       |

Architecture design/review and owner interviews are completed [P1-M2 inputs](../p1-m2/milestone-02-architecture-overview.md#architecture-clarifications-and-review). History, remote/MCP clients and public distribution remain outside Phase 1. The [work plan](milestone-03-scaffolding-architecture.md#startup-and-fixture-boundaries) defines the storage and fake-job limits so scaffolding cannot be mistaken for a working Diary feature.

## Completion checklist

Check an item only with a linked result identifying the tested revision, command/test or manual procedure, observed outcome and limitations. [Acceptance matrix](milestone-03-scaffolding-architecture.md#acceptance-matrix) rows use the same S IDs; G IDs are the unchanged repository gate.

- [x] **S01 — Approved setup:** Record owner approval of exact dependency/tool versions, licenses/notices, features, scripts and build integration before changing manifests or installing tools.
- [x] **S02 — Assembly:** Build the specified inward crate/client structure; prove workspace membership and unchanged gate paths cover both binaries and every Rust crate. Frontend production build/type checking succeeds.
- [x] **S03 — Configuration:** Pass strict schema, path/permission/root validation and tagged CLI/Apple profile fixtures without invoking an agent or loading real credentials.
- [x] **S04 — Server lifecycle:** Demonstrate independent launch, OS lock exclusion, first-available port selection/range exhaustion, private discovery publication/removal, stale/crashed-server handling and restart credential rotation.
- [x] **S05 — Access boundary:** Pass authenticated status/session/lease tests, hostile discovery/Host/Origin/proxy cases, redaction checks and matching Tauri command/permission/capability inventories. Deny secondary-window and generic input access.
- [x] **S06 — Transport harness:** Pass bounded fake-job admission/deduplication/cancellation, event ordering and replay/resync tests, including a lost gap notification. Prove fixture composition is absent from the ordinary build and no fake-admission endpoint/command is exposed.
- [x] **S07 — Client skeleton:** Render the canonical text-labeled regions and dedicated CodeMirror module; verify typed DTO validation, subscription/editor cleanup and disconnected state. Synthetic edits never claim persistence; unavailable feature controls explain why.
- [x] **S08 — Versions:** Derive UI metadata from the app, show `Version v1.0.0`, reject package/Cargo/Tauri version drift and reject incompatible protocol/capabilities during handshake. Version text is not a release-completion claim.
- [x] **S09 — Native smoke:** Record independently packaged macOS server/client identities, separate shell launch, allowed/denied IPC, readable/denied synthetic folders and cleanup/restart. Test actual packaged Origin/CSP behavior; mocks do not establish native access.
- [x] **S10 — Handoff:** Update run/contribution instructions, document the approved setup and evidence, and update the index/phase/roadmap. Leave E2–E9 and E11–E13 feature/runtime evidence with their assigned milestones.
- [x] **G01:** Pass `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
- [x] **G02:** Pass `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [x] **G03:** Pass `cargo test --manifest-path src-tauri/Cargo.toml`.
- [x] **G04:** Pass `npx prettier --check .`.
- [x] **G05:** Pass `npx eslint . --max-warnings 0`.
- [x] **G06:** Pass `npx vitest run`.

## Evidence record

2026-11-05: The owner explicitly approved additional dependencies and build tools in the implementation conversation. Exact selected versions and features will be recorded in manifests and the setup inventory; license/notice review and runtime evidence remain pending. Implementation has begun; no completion item is checked yet.

S01 | 2026-11-05, owner review in the implementation conversation | owner reviewed [third-party notices](../../../../third-party-notices.md), the inventory and the setup record | owner replied "approve" to the dependency/tool versions, selected licenses and notices (711 package records, 389 notice assets; objc2 SDK caveat preserved, no independent SDK clearance claimed) | [setup inventory](milestone-03-setup.md). Limit: personal macOS/Tauri scope only; public distribution is not cleared.

S02–S08 | base `9553a7f` plus working tree, macOS 26.7.1 arm64 | row-by-row trace of the [acceptance matrix](milestone-03-scaffolding-architecture.md#acceptance-matrix) to Rust/Vitest tests; five gap tests added (root-pair overlap, no CLI execution, missing/wrong token on status and SSE, in-memory editor edits, no console leakage); `make gate` | all six gate commands PASS including the new tests; process-level rows (discovery after readiness, signal/crash/restart, client close, secondary-window and generic IPC denial, keyboard/focus, exclusion) are evidenced by the passing native smoke under S09 | [native evidence](milestone-03-native-evidence.md). Limits: no unit test for frontend type-error failure (`npm run build` type-checks), no HTTP-level `Last-Event-ID` reconnect test (coordinator level only), no assertion pinning the 5 s heartbeat constant. G01 omits `--all`, so Vitest's architecture test is what enforces formatting of every crate.

S10 | base `9553a7f` plus working tree, 2026-11-05 | run/contribution instructions (README, contribution guide, AGENTS.md commands incl. `scripts/gate.sh`/`Makefile`), setup inventory and native evidence, then index/phase/roadmap/changelog updates and archive move of the P1-M3 plans; link check over docs, README, AGENTS.md and third-party notices; final `make gate` | no broken relative links; all six gate commands PASS after the archive; P1-M3 status Done in the phase plan and roadmap | [phase plan](../../../phase-1/phase-1.md#p1-m3-scaffolding), [roadmap](../../../roadmap.md), [changelog](../../../phase-1/changelog.md). Limit: E2–E9 and E11–E13 runtime evidence stays with later milestones per the [handoff](../../../phase-1/implementation-handoff.md).

G01–G06 | base `9553a7f` plus uncommitted working tree, Apple Silicon macOS 26.7.1 | the six unchanged gate commands, run sequentially with raw output captured | all exit 0; Rust: api 12, application 8, infrastructure 21, domain 1, plus a 17-test and a 1-test `diary_client` run; Vitest 6 files/24 tests | session logs, not retained in the repo. Limit: sources may change before S01/S09/S10 close; rerun then.

Native smoke rerun (same revision, fixture build): editor focus/keyboard probe, fake-job channel, normal close/release and secondary-window denial markers and lifecycle smoke passed; the script's `client_busy_rejected` competing-client assertion passed. Ordinary rebuild exclusion and hashes: [final S09 observation](milestone-03-native-evidence.md#final-s09-observation--2026-11-05). S09 checked; limits (debug, ad hoc, no TCC/iCloud) are recorded there.

Partial implementation evidence is recorded in the [setup inventory](milestone-03-setup.md), [native evidence](milestone-03-native-evidence.md) and [acceptance audit](milestone-03-acceptance-audit.md). Final completion evidence remains pending. Record results here as implementation proceeds, linking longer reports from the documentation index if needed. A failed/unavailable command or missing native observation leaves its item unchecked; successful documentation review or mocked tests cannot substitute for the full gate or S09.

For each result record: `S/G ID | revision and environment | command/test/procedure | observed result | evidence link or blocker`. Native results also identify the OS/CPU, executable/bundle identities, fixture paths by role (without private discovery contents), and ordinary versus fixture build. Retain no credentials, private source text or real config in evidence.

## Acceptance boundary

E1/E10 and E11's configuration-schema preparation apply here. Fake jobs establish transport behavior only: no durable-save, producer compatibility, model supervision, native inference, service recovery or measured AI responsiveness claim follows. The [handoff](../../../phase-1/implementation-handoff.md#delivery-order-and-scope) preserves those later gates.

P1-M3 is Done only when S01–S10 and G01–G06 have evidence and the [process requirements](../../../development-process.md#definition-of-done) hold. Archive the completed milestone plans and fix links at that point. Implementation is Active and all completion items remain unchecked; unfinished tooling/integration and native evidence remain gate blockers, not a docs-only exemption.

Initial assembly and tool selection are tracked in the [setup inventory](milestone-03-setup.md).
