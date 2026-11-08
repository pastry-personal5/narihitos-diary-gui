# P1-M31: Changeability refactoring work plan

Status: Archived (completed 2026-11-09)

This is the recorded implementation plan for the owner-approved changeability task set; implementation and acceptance are complete. The [overview/checklist](milestone-31-changeability-overview.md) owns completion status. Build on M30's completed boundaries and preserve the [maintained architecture](../../../architecture.md).

## Design principles

Make changes local by giving each invariant and mutable state one owner. Make extensions possible through interfaces already justified by real collaborators. Keep product policy straightforward to revise; avoid speculative abstractions, blanket line-count targets, duplicate state and compatibility layers for internal moves.

Preserve UX, HTTP/SSE and IPC contracts, command permissions, persisted formats, producer compatibility, effective prompt content/settings and current failure semantics. Refactoring does not authorize new dependencies, build tools, data deletion or product features. Any newly necessary departure must be recorded and resolved under the repository's decision rules before dependent implementation.

## Starting points and intended boundaries

These are inspection findings, not claims of failing behavior. Concrete signatures and extraction locations are finalized during A01–A02 after tracing callers.

| Starting point                                                          | Current coupling                                                                                                                                             | Intended boundary                                                                                                                                        |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/application/navigation/resources.ts`                               | Resource identity, source saves, reservations, navigation transitions and concrete `ResourceBuffer`/`EditorPayloadPool`; DOM attachment is application-owned | Resource identity/transition owner collaborating with focused source policy and an application-owned buffer port; UI/editor adapter owns host attachment |
| `src/editor/resource-buffer.ts`                                         | CodeMirror implementation also supplies classes constructed by application policy                                                                            | CodeMirror implements the buffer contract; composition injects creation; one retained buffer per resource                                                |
| `src/application/controllers.ts`, `src/components/useAppControllers.ts` | Existing graph construction and lease-managed lifecycle                                                                                                      | Retain one graph per bridge lifetime and exactly-once disposal; inject new collaborators without creating a second graph                                 |
| `src-tauri/crates/diary-api/src/generations.rs`, `chats.rs`, `lib.rs`   | HTTP handlers combine transport conversion with admission, profile/configuration checks and coordination                                                     | Application-owned typed use cases and outcomes; transport maps requests/responses; runtime adapter supplies execution mechanics                          |
| `src/components/standard-layout/useAppActions.ts`                       | Availability and handlers inspect nested resource/session/queue state                                                                                        | Application commands/selectors expose action decisions; React binds surfaces, subscriptions and dialogs                                                  |
| `tests/architecture.test.js`, `eslint.config.js`                        | Rust crate directions and permissions are checked; frontend layer rules are not explicit                                                                     | Enforced frontend layer directions alongside existing Rust checks, using current tooling                                                                 |

## Task sequence

### A — Trace ownership and establish evidence (A01–A03)

Trace four concrete change scenarios: modifying editor behavior, adding an action, substituting a source implementation and substituting an agent/export implementation. List current files touched, state owners, required wiring and proposed dependencies. Include the prompt boundary as an existing extension point rather than replacing it.

Have `test_runner` discover documented validation commands and establish the baseline. Identify relevant existing tests and synthetic smokes in [the validation guide](../../../smoke-tests.md). Classify failures before implementation. Record the inventory here; do not check tasks based on inspection alone when runtime evidence is required.

### B — Separate editor policy from CodeMirror (B01–B03)

Target `ResourceCoordinator` in `src/application/navigation/resources.ts`, which combines tab management, source saves, Trash reservations, transitions and concrete editor buffers (903 lines at planning baseline). Introduce an application-owned buffer interface, inject its CodeMirror implementation and extract source save/recovery responsibilities while retaining one owner for resource identity and transition ordering.

Derive the buffer port from operations actually used for text, edit/composition state, review, access control and payload accounting. Prefer extending the existing Authentic buffer concept where it fits. Keep CodeMirror state, view attachment and DOM types behind the editor/presentation adapter. Inject the implementation from the outer composition layer so application imports point inward.

Keep resource identity, selected/active resource, transition ordering and connection/resource epochs under one owner. Extract source save/retry state and reservation/reconciliation policy behind focused collaborators, with explicit callbacks or typed outcomes. Document ownership before moving fields; collaborators must not keep competing copies of dirty state, reservations or saved baselines.

Preserve retained undo/selection across view changes, explicit history reset on replacement, IME-aware saves, document/retained-payload limits, revision guards, retry operation IDs, dirty-source choices, Trash detachment and final close acknowledgement. Fake-buffer tests verify policy; actual CodeMirror tests verify editor semantics. Deliver the interface change and source extraction as separate reviewable steps.

Acceptance: application resource logic can run with a fake buffer; undo history, IME handling, memory limits and save acknowledgements remain correct.

### C — Move server workflow policy behind application interfaces (C01–C03)

Target `src-tauri/crates/diary-api/src/generations.rs`, which mixes HTTP handling with admission, profile checks, configuration checks and job coordination. Start with generation, then apply the demonstrated pattern to chat. Keep authentication, DTO validation, response mapping and runtime scheduling in their appropriate adapters; move transport-independent workflow decisions into application use cases.

Start with generation as a vertical slice. Define typed application requests/outcomes and explicit dependencies for existing stores, profile/prompt resolution, coordination, clock and execution hooks. Keep the application crate's existing dependency direction and standard-library-only dependency policy. Do not move Axum/Tokio types into application contracts.

Separate transport admission from business admission: authenticate and validate DTOs in HTTP; evaluate request replay, workflow eligibility, immutable capture and coordination through application use cases. Runtime adapters schedule blocking work and asynchronous execution. Preserve the ordering that recovers an existing admission before fresh profile/source checks, and recheck relevant epochs/configuration after asynchronous capture.

Apply the tested pattern to chat while retaining its fixed profile/locale and turn/transcript semantics. Preserve one inference slot, session close/cancel arbitration, transient results, memory limits, prepared prompt capture and redacted diagnostics. Define lock ownership/order explicitly so extraction does not extend lock lifetimes across callbacks or runtime waits. HTTP status/error codes, job progress and SSE behavior remain unchanged.

Acceptance: admission, retries, cancellation and configuration races can be verified without HTTP; existing routes retain their behavior.

### D — Centralize action decisions at application boundaries (D01–D03)

Derive availability selectors from explicit application snapshots or focused controller queries, rather than exposing mutable queues/maps to presentation. Commands delegate to the authoritative workflow owner and retain execution-time guards; a previously enabled button is not authorization to bypass changed state.

Use the existing action registry/dispatcher. Avoid a second command registry or parallel action state. Keep modal opening, focus, shortcut capture and React subscription mechanics in presentation. Verify common reasons and behavior across toolbar, palette and shortcuts, including pending/disconnected/reserved states and IME protection.

### E — Prove existing extension seams (E01–E03)

Review source, agent, prompt and export contracts individually. Separate shared invariants from format/provider-specific implementation only where an existing caller or demonstrated substitution requires it. Retain validation at both sides of trust boundaries; similar validation is not automatically redundant.

Use existing fake stores/runners/catalogs/destinations, extending them only when necessary to demonstrate substitution. Do not require a live new provider, new format or new destination. Keep the common execution/supervision pipeline and M30's immutable prepared prompts. Prompt management, evaluation, persistent selection and Phase 2 scheduling remain outside scope.

### F — Enforce and document the result (F01–F04)

Use existing ESLint capabilities or focused architecture tests to reject application imports of React, DOM/editor implementations, presentation and transport adapters. Check resolved local paths so alternate relative paths cannot evade rules. Keep outer composition explicit. Features may consume application contracts, but must not import another feature's presentation hook as policy. Retain existing Cargo direction and command-permission tests.

Update maintained architecture and contributor guidance with sole state owners, composition locations and procedures for editor/action/source/agent/export changes. Compare the final change scenarios with A02: identify which unrelated edits disappeared and justify any remaining cross-feature edits. Avoid universal numerical file-count targets that encourage hidden indirection.

## Verification and delivery

Implement B, C and D as independently reviewable changes after A; complete E before closing F. After each responsibility move, delegate relevant existing checks and first-pass failure analysis to `test_runner`. The main agent owns fixes and acceptance judgments. Add behavioral coverage only for meaningful risks or missing contracts, not file structure assertions that merely mirror a move.

Select affected synthetic smokes from the documented inventory: generation/chat/agent and review for workflow boundaries; source/import/producer/Trash for source mutation changes; export for shared save/action changes; ordinary GUI close when lifecycle or close paths change. Use temporary workspaces and fake inference/destinations; never real user data. Record which checks were selected and why. Unchanged external protocols do not imply new live-provider proof, and synthetic proof does not establish packaged M32 acceptance.

Run the full `scripts/gate.sh` on the final tree through `test_runner` and record fmt, clippy, Rust tests, Prettier, ESLint, Vitest and TypeScript results. Rerun only after relevant fixes or unresolved failures. Keep every implementation checkbox open until evidence supports it; completion requires the checklist, gate and affected docs to agree.

## Risks and implementation record

The main risks are moving lifecycle ownership accidentally, changing admission ordering, extending lock scope, duplicating state and adding interfaces without reducing coupling. Use small responsibility changes, focused regressions and explicit state/lock ownership to address them.

Implementation inventory and evidence are recorded below. No new product capability, transport schema or persisted format is introduced.

## Implementation ownership inventory

- `ResourceCoordinator` owns resource identity/map, selected/active resource and serialized transitions; resource epochs guard loads. `AuthenticSession` owns its save queue and acknowledgement, not the editor view.
- Source mutation collaborators operate on the resource's single source/save state; their recovery reservation map is session-owned. `TrashCoordinator` owns durable operation discovery and delegates buffer preparation/reconciliation through its existing hooks.
- An injected buffer collection owns retained editor implementations; its outer CodeMirror adapter owns host attachment. The existing graph lease owns all controller disposal and survives effect rehearsal.
- Rust `Coordinator` owns admission identities, session leases, job states and inference exclusivity. Generation `Store` owns bounded results; `Chat` owns transient turns/transcript; application use cases own preflight/capture/commit policy. HTTP owns authenticated transport and runtime scheduling; shared locks retain coordinator-before-result ordering and are never held across blocking capture.
- Action policy belongs in application commands/selectors. React owns live subscriptions, focus/dialog effects and shortcut installation; no second action registry is introduced.

### Change scenarios and existing evidence

| Scenario                               | Before                                                                                          | Intended after / evidence                                                                                                                                                  |
| -------------------------------------- | ----------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Editor implementation change           | ResourceCoordinator constructs CodeMirror buffers and owns DOM host                             | Inject buffer collection in outer composition; policy verified with a fake, existing resources/source-save/Trash tests retain real editor history/IME/memory/save coverage |
| Add action or change eligibility       | useAppActions reaches into resources/save queues                                                | Application action builder owns decisions/commands; React only binds effects; shortcuts/palette/UI tests plus focused command guard tests                                  |
| Substitute source implementation       | Existing SourceReader/Writer/Trash and ImportStore ports; frontend shares immutable source DTOs | Preserve contracts and producer validation; fake buffer/source mutation tests and existing producer/source/Trash conformance                                               |
| Substitute agent/export implementation | Existing runner/profile/prompt/export ports, but HTTP owns some workflow policy                 | Application preflight/capture/admission callable without HTTP; fake profiles/runners/stores plus existing conformance, byte-equivalence and export recovery coverage       |

No external transport or persisted contract changes are planned. Source/agent/export ports already separate implementations; only demonstrable orchestration coupling will be extracted.

## Implemented boundaries and extension review

- Buffer policy now lives in `application/editor/buffers.ts`; `CodeMirrorBuffers` is constructed with the shared budget by `useAppControllers` and injected into the existing graph. Only the adapter owns the DOM host; the coordinator selects a logical buffer. The existing Authentic `BufferPort` remains the text-only subset. Review callbacks use the same application-owned type.
- `SourceMutations` owns explicit save/retry, delete preparation and bounded reservation/reconciliation policy. It operates on the same resource object; there is no second dirty/save state. `ResourceCoordinator` retains resource-map identity, epoch invalidation, detached-copy identity changes and transition serialization. This is a responsibility boundary, not an attempt to move every source-related line out of the resource owner.
- Generation `StartRequest` owns replay-first preflight and port-based profile/prompt/source capture with configuration checks; Store.start/finish own admission/start/settlement decisions. Chat workflow owns profile eligibility, typed preparation, admission, turn start/completion and replacement/close. The API retains authentication, strict DTO conversion, error/status/progress/diagnostic mapping and Tokio scheduling. Existing cancellation guards and the common executor remain authoritative.
- `createAppActions` owns action decisions and commands, rechecking policy and current controller state at invocation. Presentation supplies modal/focus callbacks, settings/connection snapshots and subscription wiring. Structural shortcut events remove DOM event types from application policy. Profile status props use an explicit `ProfileStatusModel`, and optional review subscriptions use shared presentation plumbing instead of another feature's hook.
- Frontend boundary tests use the existing TypeScript parser/resolver to check static imports/re-exports, import types and literal dynamic imports/requires, including alternate relative paths. They reject application imports of outer implementations, DOM types/globals and cross-feature hook dependencies; explicit outer composition and application contracts remain allowed. Existing Cargo/permission architecture checks remain in place. No dependencies or gate commands changed.

### Existing extension contracts retained

| Contract                                 | Review outcome and evidence                                                                                                                                                                                                                                                                                                        |
| ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ImportStore / SourceWriter / SourceTrash | Already application-owned; format parsing and verified producer mutation stay in infrastructure. No new producer abstraction is necessary. Node fake-buffer/source tests verify client policy independently; existing source/producer/Trash conformance verifies implementations.                                                  |
| ProfileDirectory / AgentRunner           | Already separate protocol/host implementations from shared execution. The actual gap was HTTP workflow orchestration, now extracted. Direct workflow tests substitute a synthetic Claude profile/runner and verify captured provider identity and successful execution. Existing registry conformance covers all registered kinds. |
| PromptResolver / ResolvedPrompt          | M30 already provides immutable identity, preparation and byte-equivalent rendering. Reuse it unchanged; existing embedded rendering/revision/Apple pairing/failure-isolation conformance remains acceptance coverage. No prompt asset, model pin or provider protocol changed.                                                     |
| ExportAdapter / Exports                  | Already destination-neutral, with fake destination and fault/recovery coverage. No contract expansion is justified. Export actions now share application eligibility/commands, while existing real/fake adapters and durable recovery remain unchanged.                                                                            |

### Change scenarios after implementation

An editor implementation changes its adapter and explicit outer wiring; resource policy tests run in Node without CodeMirror. Source retry/reservation changes stay in SourceMutations rather than tab/view logic. A new action changes the existing registry and application action builder plus the chosen UI entry point; toolbar, palette and shortcuts reuse its decisions. Provider and destination substitutions implement existing ports and register at composition; generation/chat workflow policy no longer depends on HTTP types. Tests/maintained docs accompany those changes; transport and producer fixtures still change when their actual contracts change.

## Final validation evidence

`test_runner` established a passing seven-lane baseline before implementation. Focused frontend integration coverage passed 118 tests across 12 files. New fake-buffer/action/architecture coverage passed 8 tests across 3 files, TypeScript and ESLint passed, application tests passed 75 tests and API tests passed 88 tests. First-pass failures were corrected: React render-time ref access, incomplete fixtures/invalid test digest, cross-feature type imports exposed by the new boundary checker, and Clippy's large chat start outcome (boxed without suppressing the lint). Final `scripts/gate.sh` passes all seven lanes (791 Rust tests, 400 frontend tests in 54 files). All eight selected synthetic/native smokes pass: agent, review, export, imports, producer, Trash with `--enabled-server`, Authentic recovery and ordinary GUI close. The GUI check passed on retry after an initial pre-dispatch abort; no unrelated process was stopped. See the [acceptance record](milestone-31-changeability-overview.md#acceptance-record) for exact commands and evidence limits. M32 remains Planned.
