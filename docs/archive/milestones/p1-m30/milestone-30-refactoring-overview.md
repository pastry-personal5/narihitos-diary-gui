# P1-M30: Refactoring overview

Status: Done (2026-11-09)

Refactor the application around canonical UX terms and explicit ownership of state, use cases and adapters. Prepare prompt resolution and execution for multiple-prompt management and evaluation in Phase 2 / Diary v2.0.0, while retaining the current v1 feature set.

## Authority and entry

The owner authorized unrestricted refactoring on 2026-11-08, including incompatible changes to module/directory names, functions, variables and APIs. This supersedes M28's compatibility and mechanical-cleanup limits for M30. It does not authorize deleting user data, adding dependencies/build tools or inventing new product behavior. M29 and its follow-ups are complete; M30 implementation and acceptance are complete.

Plan: [architecture and sequence](milestone-30-refactoring-architecture.md). Vocabulary: [UX terms](../../../ux-terms.md). Product semantics remain in the maintained [architecture](../../../architecture.md) and [product behavior](../../../product-behavior.md).

## In scope

- Audit and rename UI components, hooks, application models, commands, DTOs, tests, styles and active documentation against canonical UX terms.
- Reorganize frontend features around Standard Layout, Explorer, Tabbed Editor, Global Messages, AI Chat, AI Activities, Settings and Trash Recovery, with explicit lifecycle ownership.
- Separate server use cases, transport conversion, persistence and provider protocols where current coupling obscures their responsibilities. Prefer descriptive types and functions to generic `Service`, `Inner`, `Frozen` or `Ctx` when context does not make their meaning clear.
- Extract prompt selection, parsing, rendering and immutable run provenance from provider execution. Keep the bundled default prompts and their verified model/effort pins.
- Remove obsolete internal aliases, duplicate validation/policy and abandoned code after callers and tests move together. Preserve independent validation at trust boundaries.
- Update architecture, prompt documentation, tests, scripts, command permissions and the implementation handoff with changed contracts.

## Out of scope

- Multiple-prompt management, prompt editing/selection UI, user prompt persistence, version history, import/export, evaluation datasets, scoring, comparison UI, batch experiments or automatic prompt optimization. These belong to Phase 2 / v2.0.0.
- New agents, provider/model upgrades, prompt wording changes, inference retries, product redesign or new dependencies/build tools.
- Destructive resets, deleting real data or weakening save, review, export, authentication, cancellation or recovery guarantees.
- Completing or claiming M32's packaged/native release evidence.

## Completion checklist

All implementation acceptance items below are checked against the final requirement audit and delegated validation evidence. Planning validation alone does not complete M30.

- [x] R01 — Record the before/after module and naming inventory, all affected contract/storage changes and the intentional technical-name exceptions in the work plan before moving code.
- [x] R02 — Align represented UI regions, components, hooks, labels and accessibility names with canonical terms; searches and affected UI tests demonstrate removal of obsolete aliases without collapsing distinct domain concepts.
- [x] R03 — Establish the frontend ownership boundaries in the work plan; regressions demonstrate one controller lifecycle, teardown and reconnect behavior without duplicate subscriptions or completion messages.
- [x] R04 — Refactor Rust use cases/adapters and descriptive interfaces; architecture checks demonstrate inward dependencies, and behavior tests cover moved responsibilities.
- [x] R05 — Update HTTP/SSE, native bridge, TypeScript contracts/validators, command registrations/permissions and smoke clients atomically; invalid-input and denied-command tests pass.
- [x] R06 — Introduce the prompt resolver/preparation boundary, typed identities, immutable revision provenance and separate execution settings; current assets render byte-equivalently with unchanged pins and capability failures.
- [x] R07 — Demonstrate with synthetic injected prompt definitions that distinct revisions resolve independently, running work retains its captured definition, and Apple session/task assets stay paired. No management/evaluation feature is exposed.
- [x] R08 — Fake-runner tests demonstrate that the preparation/execution boundary can be reused without a GUI/session bootstrap or real inference; validation failure, cancellation and output bounds remain enforced. Evaluation policy and storage remain deferred.
- [x] R09 — Preserve durable state and external producer formats. For any changed persisted format, document and test non-destructive conversion/refusal, restart and interrupted-operation recovery before changing its implementation; no real workspace reset is an acceptance shortcut.
- [x] R10 — Regression evidence covers Authentic autosave/conflicts, source save/Trash, Candidate/Paragraph review, date switching, AI progress/results/reconnect, Settings, export confirmation/recovery and ordinary GUI shutdown at the affected layers.
- [x] R11 — Update maintained docs, prompt inventory, scripts and links; record departures and residual risks. Existing M32 evidence obligations remain visible.
- [x] R12 — The delegated test runner reports targeted checks and the complete `scripts/gate.sh` passing on the final implementation; run affected synthetic smokes and record their results separately from native/live proof.

## Evidence record

Initial planning (2026-11-08) reviewed the repository and prompt execution flow. Implementation evidence follows; planning validation alone does not satisfy implementation acceptance.

### Stage A–C implementation progress (2026-11-08)

- Before moving code, the work plan recorded path/symbol mappings, controller
  owners, proposed prompt contracts, format classifications and intentional
  technical-name exceptions (R01).
- Moved frontend features and their callers/tests together. Added StandardLayout,
  MainToolbarStrip, MainContent and TabbedEditor; geometry belongs to layout and
  the retained editor host belongs to the Tabbed Editor composition. Moved
  GlobalMessagesStore out of navigation's module directory. Application
  `createAppControllers` now owns controller creation and final teardown, with
  graph-specific leases for StrictMode rehearsal and bridge replacement.
- Renamed Rust GenerationContext, AgentProtocolContext and ExportCoordinator /
  ExportCoordinatorState with callers. No dependency, command, transport JSON,
  producer or persisted format change has been made in this stage.
- Delegated baseline: six gate lanes passed; G03 initially failed on local-socket
  PermissionDenied during parallel execution. The runner's isolated handshake
  check and sequential Cargo tests passed. This is a baseline environment limit,
  not a passing full-gate claim.
- First frontend check passed all 391 existing tests. Subsequent checks caught
  stale imports, a moved candidateKey import, formatting and an export type-name
  collision; fixes passed delegated checks: 51 frontend files / 393 tests, TypeScript, ESLint, workspace Rust formatting, Clippy and sequential Cargo tests. These results precede the next ownership-stage changes.

- Subsequent ownership stage: the application graph constructs GlobalMessagesStore
  and injects it into NavigationModel; SourceTrashController coordinates Trash
  independently from navigation. Explorer and keyboard actions receive its
  interface, and connection lifecycle connects recovery separately. Removed the
  unused useAuthentic lifecycle hook and obsolete ExportController.revive; moved
  shared profile wording to application. Delegated frontend tests passed (51 files /
  392 tests), and the follow-up `npx tsc --noEmit`, `npx eslint . --max-warnings 0` and full frontend rerun passed.

### Prompt preparation and final verification (2026-11-09)

The application now owns PromptId, PromptRevision, PromptAction, PromptSelection,
PromptResolver, ResolvedPrompt and PreparedAgentRun. Infrastructure separates
ParsedPromptAsset metadata/source, content-only PromptTemplate, the immutable
renderer and the embedded catalog. Revision hashes length-prefix exact task and
paired session sources, including metadata. Resolution captures once per
admitted chat turn / generation job; sequential candidate styles retain that
same definition. Providers receive prepared parts and AgentExecutionSettings,
never the catalog. Body-bearing request Debug is redacted; template and prepared
content have no Debug/serialization implementation. Retained definition bytes,
settings and provenance are included in generation accounting.

The pre-M30 render fixture was generated independently from commit
`e502fa4dec2731abf98a0a9ccaacda4820e5f38e`: nine embedded parts, en-US/ko-KR,
fixed `fixture-data`, old newline normalization and single-pass replacement.
Current conformance tests match every SHA-256 and byte length; verified model /
effort pins and protocol flags remain unchanged. No live inference, prompt
wording, provider/model upgrade or management/evaluation surface was introduced.

#### Requirement-to-evidence audit

| Item | Implementation and verification evidence                                                                                                                                                                                                                                                                                                                                                                          |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R01  | Pre-move Stage A inventory plus the recorded final terminology-pass inventory; every change classified internal or synthetic-harness-only. No storage/transport schema changed.                                                                                                                                                                                                                                   |
| R02  | Feature modules, AIChat/AIActivity names, content/editor/layout components and canonical selectors/accessibility groups moved with callers. Searches found no obsolete code aliases; client, accessibility, navigation-layout, symbol and generation UI suites cover the resulting regions and controls.                                                                                                          |
| R03  | Application controller graph owns one shared SessionBudget and each controller; NavigationModel receives its message sink/budget, SourceTrashController owns recovery coordination. Controller-lifetime tests cover rehearsal, repeated release, exact-once teardown and bridge replacement. Connection/activity suites cover no overlap, stale replies, unmount, silent restore and one completion announcement. |
| R04  | Application prompt values/preparation/runner ports point inward; infrastructure catalog/parser/renderer and provider adapters implement them. GenerationContext, AgentProtocolContext and ExportCoordinator are descriptive interfaces. Cargo dependency architecture checks, Rust behavior tests and fake-child adapter suites cover moved responsibilities.                                                     |
| R05  | HTTP/SSE DTOs, native command names, TypeScript bridge/validators, command registration, permission manifest and protocol/schema versions are unchanged. Preparation failures use existing closed error codes before admission. Existing architecture inventory, invalid-input, stale-discovery and secondary-window denial tests pass; no compatibility shim or dual protocol was added.                                                                                                |
| R06  | Prompt conformance tests pin all embedded bytes/settings, bound packet/template/rendered sizes, isolate missing/invalid assets, reject incompatible provider/action, and preserve single-pass substitution. Apple session and task resolve together.                                                                                                                                                              |
| R07  | Synthetic catalogs prove content and metadata revisions differ while the captured definition remains stable after replacement. API tests prove one resolution for all candidates and fresh resolution for the next chat turn, with transient provenance retained internally.                                                                                                                                      |
| R08  | Standalone preparation and run_prepared need no GUI/session. Fake-runner/application tests plus direct prepared Codex fake-child tests cover success, cancellation, invalid/fenced output and output flooding; the original common supervisor/validator path remains in use.                                                                                                                                      |
| R09  | No migration, durable record, producer document, Authentic revision, export checkpoint or Trash receipt format changed. Authentic/import/export restart/fault smokes and the 22-scenario native/HTTP Trash report prove recovery on synthetic data; no real workspace was reset.                                                                                                                                  |
| R10  | Full frontend/Rust suites cover autosave/conflicts, source save/Trash, Candidate/Paragraph review, date switching, progress/results/reconnect, Settings and export confirmation/recovery. The ordinary GUI-close smoke separately passes and leaves the independent server available for session recovery.                                                                                                        |
| R11  | Maintained editing/agent architecture, UX implementation map, prompt inventory, handoff, phase/index/roadmap and smoke documentation updated. Completed M29 follow-up docs archived with links fixed. M32 packaged/native qualification and Qwen live exemption remain visible.                                                                                                                                   |
| R12  | All seven gate lanes pass on the final implementation, including the final parsed-template split. The affected Rust suite and agent smoke were rerun successfully after that split. All seven named synthetic/native smokes passed; final documentation formatting/link checks are recorded separately below.                                                                                                                                                                                                 |

#### Commands and evidence limits

- Delegated frontend checks: `npx tsc --noEmit`, `npx eslint . --max-warnings 0`,
  `npx vitest run --reporter=dot` — pass, 51 files / 392 tests.
- Delegated Rust checks: workspace formatting, Clippy with all targets and
  warnings denied, and sequential Cargo tests — pass. The largest suite has
  320 passing tests and one ignored test; other workspace suites also pass.
- `scripts/gate.sh` — G01–G07 pass with authorized execution outside the network
  sandbox. An explicit ephemeral-loopback probe fails inside the sandbox and
  passes outside it, explaining the earlier G03 failures without changing tests.
- `python3 scripts/smoke/authentic.py`, `imports.py`, `agent.py`, `review.py` and
  `export.py` — pass on temporary workspaces with fake providers/destinations.
- `python3 scripts/smoke/trash.py --enabled-server` — pass: 22 native/server
  scenarios recorded in `src-tauri/target/trash-harness-evidence.json`, including
  HTTP preview/confirmation/restart. The harness selects the library test
  artifact explicitly rather than assuming Cargo emits only one executable.
- `python3 scripts/smoke/gui-close.py` — pass: ordinary debug GUI close plus
  independent-server/session recovery. This is not M32 packaged qualification.
- Initial smoke attempts failed at default-port readiness / sandbox bind, and
  the first Trash attempt hit its stale compiler-artifact assumption. Fixtures
  now select isolated loopback ports and only their own authenticated discovery;
  no unrelated process was stopped. These failures are superseded by the passes.

Real prompt quality, all-version provider compatibility, phone/live-iCloud
behavior, packaged responsiveness/native interaction and release qualification
remain outside this evidence. Qwen live inference remains exempt: GPU throttling.
M30 adds no prompt-history store, user prompt controls, evaluator or scoring.

Closure (2026-11-09): R01–R12 complete. The final internal representation change
passed Clippy, all Rust tests, the synthetic agent smoke and the full G01–G07
gate. The other six successful smokes cover unchanged recovery/GUI boundaries;
the final split changes only prompt representation and retained-byte accounting.
Completed plans are archived with current links; M31 and M32 remain Planned.
Final documentation verification passed: `npx prettier --check .`, `git diff --check`, and a local Markdown target/anchor audit of 16 affected files and 559 references. External URLs and fenced examples were excluded; all checked local targets and anchors resolve.
