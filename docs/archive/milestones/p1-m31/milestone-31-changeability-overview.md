# P1-M31: Changeability refactoring overview

Status: Done (2026-11-09)

Make existing behavior easy to change and extensions easy to add through explicit ownership and interfaces. This is the first owner-requested task set for M31, following the completed M30 refactor; implementation and acceptance are complete. The [work plan](milestone-31-changeability-architecture.md) defines sequencing and acceptance evidence.

## Authorization and scope

The owner accepted the changeability plan on 2026-11-09 and requested that it be recorded as tasks. The owner subsequently requested M31 implementation. The refactor and its acceptance checks are complete; evidence is recorded below.

In scope: editor/resource boundaries, source save and Trash reconciliation ownership, generation/chat orchestration, shared action policy, existing extension contracts, architecture enforcement and contributor guidance. Preserve M30's controller lifetime and immutable prompt preparation work.

Preserve current UX, transport contracts, persisted formats, dependencies and build tools. New source formats, providers, export destinations, prompt management and evaluation are outside this milestone's refactoring task set. Multiple-prompt management/evaluation remain Phase 2; packaged release qualification remains M32. Dependency/build-tool changes, user-data deletion and departures from decided behavior still require separate authorization under [AGENTS.md](../../../../AGENTS.md).

## Completion checklist

Items are checked only when the implementation and evidence below support them. File size alone is not an acceptance criterion.

### A — Baseline and ownership

- [x] A01 — Record current owners and lifetimes for editor resources, saves, source reservations, navigation, AI admission, cancellation and recovery. Include an explicit map of collaborators and state owners in the work plan.
- [x] A02 — Trace representative changes: editor behavior, a new action, another source implementation and another agent/export implementation. Record current cross-layer edits and intended extension points; synthetic examples do not enable new product capabilities.
- [x] A03 — Delegate the documented baseline checks to `test_runner`; record failures and environment limits separately from regressions. Inventory existing behavioral coverage before adding tests.

### B — Separate editor policy from CodeMirror

`ResourceCoordinator` in `src/application/navigation/resources.ts` combines tab management, source saves, Trash reservations, transitions and concrete editor buffers (903 lines at planning baseline). Introduce an application-owned buffer interface and inject its CodeMirror implementation. Extract source save/recovery responsibilities while retaining one owner for resource identity and transition ordering.

Acceptance: application resource logic can run with a fake buffer; undo history, IME handling, memory limits and save acknowledgements remain correct.

- [x] B01 — Define an application-owned buffer interface and creation contract from actual resource needs; inject the CodeMirror implementation at composition. Application resource policy no longer imports concrete editor code or owns a DOM host.
- [x] B02 — Separate source save/retry and Trash reconciliation responsibilities from tab identity and navigation. Record the sole owner of each mutable field and reservation; preserve operation-ID reuse and guarded async results.
- [x] B03 — Verify resource workflows with fake buffers and real-editor regressions covering retained undo/selection, IME, memory refusal, save acknowledgements, reconnect, tab/date transitions and close.

### C — Move server workflow policy behind application interfaces

`src-tauri/crates/diary-api/src/generations.rs` mixes HTTP handling with admission, profile checks, configuration checks and job coordination. Start with generation, then apply the demonstrated pattern to chat. Keep authentication, DTO validation, response mapping and runtime scheduling in their appropriate adapters. Move transport-independent workflow decisions into application use cases.

Acceptance: admission, retries, cancellation and configuration races can be verified without HTTP; existing routes retain their behavior.

- [x] C01 — Extract transport-independent generation admission and lifecycle policy behind application interfaces. HTTP retains authentication, DTO validation and response mapping; runtime adapters retain scheduling and execution mechanics.
- [x] C02 — Apply the demonstrated boundary to chat, preserving differences between chat turns and sequential generation. Keep one agent slot, captured prompt provenance, bounded results and session ownership.
- [x] C03 — Exercise admission replay, busy/cancel/close races, configuration-generation changes and failure handling without HTTP; retain route/bridge coverage for unchanged transport behavior.

### D — Shared action policy

- [x] D01 — Introduce focused commands and availability selectors for Save, Create Empty, Generate, Export and Trash. Remove their UI dependence on nested controller/save-queue internals while preserving reasons and guards.
- [x] D02 — Keep toolbar, command palette and keyboard shortcuts on the same action definitions. React owns subscriptions, modal/focus interactions and keyboard wiring.
- [x] D03 — Verify identical action eligibility across surfaces, safe invocation after state changes, existing shortcut/IME behavior and a documented path for adding an action.

### E — Extension contracts

- [x] E01 — Review source, agent, prompt and export interfaces for implementation-specific assumptions; document each necessary change and its existing caller before extracting an abstraction.
- [x] E02 — Use focused synthetic implementations to prove representative substitutions preserve shared invariants without modifying unrelated workflows. Reuse existing conformance fixtures where sufficient; add no production provider or destination.
- [x] E03 — Preserve M30's prompt resolution/preparation boundary and effective prompt bytes/settings. Do not introduce prompt management, an evaluator, a service locator or a generic plugin framework.

### F — Enforcement, documentation and acceptance

- [x] F01 — Enforce frontend dependency directions and cross-feature view/hook restrictions with existing tooling; retain Rust dependency and explicit permission checks. Checks cover forbidden imports and permitted composition, including relative-path bypasses.
- [x] F02 — Update maintained architecture and contributor guidance with state ownership, extension procedures and before/after change scenarios. Record any justified departure from this work plan.
- [x] F03 — Delegate affected behavioral regressions and synthetic smokes to `test_runner`; preserve temporary-workspace isolation and record exact commands/results. Native evidence is required where an affected boundary needs it; automated checks do not discharge M32.
- [x] F04 — Delegate the final `scripts/gate.sh` to `test_runner`, resolve regressions, and record all seven results before marking implementation complete. Update milestone/roadmap status and archive completed plans according to the development process.

## Acceptance record

Completed 2026-11-09. `test_runner` executed the baseline, focused regressions, synthetic/native smokes and final gate; the main agent reviewed implementation and acceptance.

| Tasks   | Implementation and acceptance evidence                                                                                                                                                                                                                                                                                                                            |
| ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A01–A03 | Ownership and before/after change scenarios are recorded in the work plan. Baseline `scripts/gate.sh` passed all seven lanes with loopback permissions.                                                                                                                                                                                                           |
| B01–B03 | Resource policy uses injected buffer ports; CodeMirrorBuffers owns host attachment; SourceMutations uses the resource's single save state. Node fake-buffer tests pass, and real-editor integration/full-suite tests retain undo, IME, payload limits, save acknowledgement, navigation and close behavior.                                                       |
| C01–C03 | Generation/chat application workflow services own typed preflight/capture/admission/start/completion decisions. Application tests pass 75 cases, including 8 new direct workflow tests for replay, simultaneous admission, profile/configuration checks, cancellation/close and alternative-provider execution. API tests pass 88 cases against unchanged routes. |
| D01–D03 | Common action builder uses the existing registry/dispatcher, focused application commands/selectors and invocation-time guards. New Node action tests and existing toolbar/palette/shortcut/UI coverage pass in the 400-test frontend suite.                                                                                                                      |
| E01–E03 | Work-plan extension review retains existing source/agent/prompt/export ports. Fake-source/alternative-provider tests, prompt rendering/revision conformance and existing export fake/fault/recovery coverage pass; no prompt assets, protocols or pins changed.                                                                                                   |
| F01–F02 | Frontend dependency tests resolve imports and reject forbidden directions/relative-path bypasses; Cargo/permission checks remain intact. Maintained architecture and contributor change procedures are updated.                                                                                                                                                   |
| F03     | All eight synthetic/native checks below pass, including ordinary GUI close and native/server Trash recovery.                                                                                                                                                                                                                                                      |
| F04     | Final `scripts/gate.sh` passes all seven lanes: fmt, clippy, Rust tests (791 passed in aggregate), Prettier, ESLint, Vitest (54 files / 400 tests) and TypeScript. Plans are archived and phase/roadmap status is updated.                                                                                                                                        |

### Smoke commands and outcomes

- `python3 scripts/smoke/agent.py` — passed; fake CLI/profile/chat and embedded asset checks.
- `python3 scripts/smoke/review.py` — passed; synthetic paragraph review and recovery.
- `python3 scripts/smoke/export.py` — passed; fake-destination coordination and fault recovery.
- `python3 scripts/smoke/imports.py` — passed; civil-date import/resolution/restart.
- `python3 scripts/smoke/producer.py` — passed; 12 fixtures and 3 accepted mutants, with expected malformed rows reported.
- `python3 scripts/smoke/trash.py --enabled-server` — passed; native faults/recovery, normal-server preview/confirm/restart/duplicate and producer catalog/reopen coverage.
- `python3 scripts/smoke/authentic.py` — passed; create/save/restart/interrupted-save recovery.
- `python3 scripts/smoke/gui-close.py` — passed; ordinary close exits launcher/client/dev tree, independent server survives and admits a fresh session. The initial attempt was aborted before tool dispatch; the authorized retry executed and passed.

### Evidence limits

All smokes used isolated temporary workspaces and synthetic inference/destinations. No real user data, live provider usage or live Notion changes were needed. Qwen live inference remains exempt for GPU throttling; registered-kind synthetic conformance still runs. M32 retains packaged/native release qualification, live iCloud and other previously assigned release evidence; ordinary debug GUI close does not discharge it.

Representative editor, action, source and provider/destination changes now have explicit owners and wiring points, as detailed in the work plan. UX, transport/persisted contracts, dependencies and build tools remain unchanged.
