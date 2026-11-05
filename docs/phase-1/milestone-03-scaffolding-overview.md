# P1-M3: Scaffolding overview

Status: Planned

Create a buildable, independently launched server/Tauri client skeleton from the completed [P1-M2 architecture](../archive/milestones/p1-m2/milestone-02-architecture-overview.md). This plan covers scaffolding and synthetic transport evidence; completing the plan review does not complete the milestone or authorize dependency changes.

## Entry conditions

P1-M1 and P1-M2 are Done; P1-M3 remains Planned until scaffold implementation begins. Read the [work plan](milestone-03-scaffolding-architecture.md), its contract links and the [handoff evidence register](implementation-handoff.md#capability-evidence-register) before implementation. No owner product question currently blocks scaffolding.

The first implementation prerequisite is approval of the concrete dependency/build-tool inventory in S01. Prepare that review before asking for approval; do not infer approval from the fixed stack or architecture recommendations. This documentation change selects no package versions, installs nothing and starts no implementation.

## Scope

| Included in P1-M3 | Deferred capability and owner milestone |
|---|---|
| Approved tool setup, inward Cargo workspace, two executables, frontend build/type checks and test infrastructure | Feature algorithms/adapters remain in their consuming milestones; do not prebuild every future port or add unused dependencies |
| Startup config validation, OS lock exclusion, port selection, private discovery, authentication, one-client lease and redacted status | Workspace/SQLite/durable operations: P1-M5; Authentic persistence/recovery: P1-M6; client autosave and Create Empty: P1-M7 |
| Narrow typed Tauri bridge, bounded HTTP/SSE and deterministic in-memory fake jobs in a test harness | Sources/imports/navigation: P1-M9–P1-M11; source saves/Trash: P1-M14/P1-M15; agent/AI/review features: P1-M16–P1-M24; exports: P1-M25–P1-M27 |
| Text-labeled Standard Layout, dedicated CodeMirror module and explicitly synthetic editor state | Logging: P1-M4; preferences/reload: P1-M8; full layout: P1-M11; shortcuts: P1-M12; native symbols/interaction: P1-M13; complete AI activity/results: P1-M23 |
| Version/protocol checks, full code gate and packaged synthetic server/client smoke evidence | Personal release and complete workflow/platform qualification: P1-M28 |

Architecture design/review and owner interviews are completed [P1-M2 inputs](../archive/milestones/p1-m2/milestone-02-architecture-overview.md#architecture-clarifications-and-review). History, remote/MCP clients and public distribution remain outside Phase 1. The [work plan](milestone-03-scaffolding-architecture.md#startup-and-fixture-boundaries) defines the storage and fake-job limits so scaffolding cannot be mistaken for a working Diary feature.

## Completion checklist

Check an item only with a linked result identifying the tested revision, command/test or manual procedure, observed outcome and limitations. [Acceptance matrix](milestone-03-scaffolding-architecture.md#acceptance-matrix) rows use the same S IDs; G IDs are the unchanged repository gate.

- [ ] **S01 — Approved setup:** Record owner approval of exact dependency/tool versions, licenses/notices, features, scripts and build integration before changing manifests or installing tools.
- [ ] **S02 — Assembly:** Build the specified inward crate/client structure; prove workspace membership and unchanged gate paths cover both binaries and every Rust crate. Frontend production build/type checking succeeds.
- [ ] **S03 — Configuration:** Pass strict schema, path/permission/root validation and tagged CLI/Apple profile fixtures without invoking an agent or loading real credentials.
- [ ] **S04 — Server lifecycle:** Demonstrate independent launch, OS lock exclusion, first-available port selection/range exhaustion, private discovery publication/removal, stale/crashed-server handling and restart credential rotation.
- [ ] **S05 — Access boundary:** Pass authenticated status/session/lease tests, hostile discovery/Host/Origin/proxy cases, redaction checks and matching Tauri command/permission/capability inventories. Deny secondary-window and generic input access.
- [ ] **S06 — Transport harness:** Pass bounded fake-job admission/deduplication/cancellation, event ordering and replay/resync tests, including a lost gap notification. Prove fixture composition is absent from the ordinary build and no fake-admission endpoint/command is exposed.
- [ ] **S07 — Client skeleton:** Render the canonical text-labeled regions and dedicated CodeMirror module; verify typed DTO validation, subscription/editor cleanup and disconnected state. Synthetic edits never claim persistence; unavailable feature controls explain why.
- [ ] **S08 — Versions:** Derive UI metadata from the app, show `Version v1.0.0`, reject package/Cargo/Tauri version drift and reject incompatible protocol/capabilities during handshake. Version text is not a release-completion claim.
- [ ] **S09 — Native smoke:** Record independently packaged macOS server/client identities, separate shell launch, allowed/denied IPC, readable/denied synthetic folders and cleanup/restart. Test actual packaged Origin/CSP behavior; mocks do not establish native access.
- [ ] **S10 — Handoff:** Update run/contribution instructions, document the approved setup and evidence, and update the index/phase/roadmap. Leave E2–E9 and E11–E13 feature/runtime evidence with their assigned milestones.
- [ ] **G01:** Pass `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
- [ ] **G02:** Pass `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [ ] **G03:** Pass `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] **G04:** Pass `npx prettier --check .`.
- [ ] **G05:** Pass `npx eslint . --max-warnings 0`.
- [ ] **G06:** Pass `npx vitest run`.

## Evidence record

Implementation evidence is pending for every S/G item. Record results here as implementation proceeds, linking longer reports from the documentation index if needed. A failed/unavailable command or missing native observation leaves its item unchecked; successful documentation review or mocked tests cannot substitute for the full gate or S09.

For each result record: `S/G ID | revision and environment | command/test/procedure | observed result | evidence link or blocker`. Native results also identify the OS/CPU, executable/bundle identities, fixture paths by role (without private discovery contents), and ordinary versus fixture build. Retain no credentials, private source text or real config in evidence.

## Acceptance boundary

E1/E10 and E11's configuration-schema preparation apply here. Fake jobs establish transport behavior only: no durable-save, producer compatibility, model supervision, native inference, service recovery or measured AI responsiveness claim follows. The [handoff](implementation-handoff.md#delivery-order-and-scope) preserves those later gates.

P1-M3 is Done only when S01–S10 and G01–G06 have evidence and the [process requirements](../development-process.md#definition-of-done) hold. Archive the completed milestone plans and fix links at that point. This plan review leaves P1-M3 Planned and all implementation items unchecked; absent manifests/tooling are current gate blockers, not a docs-only exemption.
