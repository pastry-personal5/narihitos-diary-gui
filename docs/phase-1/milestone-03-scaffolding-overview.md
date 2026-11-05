# P1-M3: Scaffolding overview

Status: Planned

Create a buildable client/server skeleton from the completed [P1-M2 architecture](../archive/milestones/p1-m2/milestone-02-architecture-overview.md). This milestone implements scaffolding only and requires the full six-command gate.

## Scope

In: approved package/tool setup, Rust workspace/crate boundaries, independently launched server/Tauri client, configuration/discovery/auth/session/status wiring, typed bridge and fake-job HTTP/SSE, a minimal text-labeled Standard Layout with a dedicated CodeMirror module, version consistency and test infrastructure. Plan: [work plan](milestone-03-scaffolding-architecture.md).

Out: architecture design/review and owner product interviews, durable Diary/source operations, settings persistence and recovery, complete logging rotation/retention, shortcut editing, complete AI activity/results UI, live agents/prompts/inference, exports, history, remote/MCP clients and public distribution. Completed design and interview evidence belongs to [P1-M2](../archive/milestones/p1-m2/milestone-02-architecture-overview.md#architecture-clarifications-and-review); the [handoff](implementation-handoff.md#delivery-order-and-scope) assigns feature implementation and acceptance to later milestones. Concrete package/build-tool approval remains a prerequisite for scaffold setup.

## Completion checklist

- [ ] Obtain dependency/build-tool approval for the concrete versions/license/features inventory before editing manifests.
- [ ] Create the specified Cargo workspace with the client, server and inward crates included in the existing gate paths; verify dependency direction and package coverage.
- [ ] Run separate shell server/client commands with explicit config/discovery, private token lifecycle and workspace lock exclusion.
- [ ] Validate the startup configuration schema and port range against the maintained contracts, including absolute roots, CLI profile fields/default executable resolution and alias/type-only Apple profiles; optional profiles remain unavailable without inference or CLI execution.
- [ ] Verify first-available binding from 8781 through 8789 by default, fallback count 0–8, overflow rejection and failure after range exhaustion.
- [ ] Verify E1/E10 scaffold acceptance in the [evidence register](implementation-handoff.md#capability-evidence-register): discovery/auth/origin/Host/lease failures, stale discovery, second-client rejection and independently packaged server/client identities.
- [ ] Provide only the implemented typed session/status/fake-job bridge commands and channels with matching manifests/permissions; deny generic URL/path/shell inputs and commands from secondary windows.
- [ ] Verify bounded fake-job admission, cancellation, request deduplication, ordered events and reconnect/resync, including a lost event notification; no durable operation or real-agent capability is inferred from these fakes.
- [ ] Render the text-labeled Standard Layout and dedicated CodeMirror editor with typed fake state; frontend state/logs contain no private config or token. Feature controls stay disabled or explicitly synthetic.
- [ ] Set SemVer metadata from the approved version source, show `Version v1.0.0`, and reject release-version drift or an incompatible client/server protocol.
- [ ] Pass `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
- [ ] Pass `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [ ] Pass `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] Pass `npx prettier --check .`.
- [ ] Pass `npx eslint . --max-warnings 0`.
- [ ] Pass `npx vitest run`.
- [ ] Record a packaged macOS synthetic smoke test for separate launch, typed IPC, denied window/command and readable/denied folders; mocks alone do not establish native behavior.
- [ ] Update contribution/run instructions, index and phase status with scaffold evidence; keep unimplemented feature capabilities disabled.

## Acceptance boundary

E1/E10 and E11's configuration-schema preparation apply here. Fake jobs establish the transport/test skeleton; E2–E9 and the E11–E13 feature/runtime evidence remain assigned to their consuming milestones. Architecture documentation checks are P1-M2 evidence and do not check off this milestone. The absent scaffold/manifests and unavailable JS tooling leave the implementation gate pending.
