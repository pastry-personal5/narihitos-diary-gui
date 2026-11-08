# P1-M30: Refactoring work plan

Status: Done (2026-11-09)

Implement the [M30 scope and checklist](milestone-30-refactoring-overview.md) as ordered, reviewable changes. Internal compatibility may break; saved user content and the decided product behavior remain protected.

## Findings from the current tree

- M28 already split Rust agent/export/API modules and extracted React hooks. Repeating file-size splits alone would miss ownership and terminology issues.
- `src/components` mixes layout, feature views and lifecycle hooks. `App.tsx` and `useAppControllers.ts` compose navigation, chat, generation, review, exports and activities, while `NavigationModel` also constructs messages and Trash coordination.
- Names including `ChatPane`, `AuthenticPane`, `GenerationPane`, `ReviewBar`, `Messages` and `activity` need contextual mapping to the canonical vocabulary. Draft and Entry are navigation views of the same Authentic, not separate persisted entities.
- `agent/assets.rs` in `diary-infrastructure` combines an embedded asset table, front-matter parsing, substitution and action capability lookup. Its `Prompt` combines template text with model/effort settings.
- `agent/engine.rs` owns asset lookup, rendering and execution. Its `Protocol::stdin` receives the entire `PromptAssets` catalog; Apple uses that to fetch session instructions separately. This hides which complete prompt definition a run uses.
- `generation::Frozen` captures date, profile, locale, baseline and snippets, but no prompt identity/revision. Prompt identity is currently implicit in `(AgentKind, asset name)`. An active run needs a resolved immutable prompt definition before future catalog changes can be safe.
- Provider protocols already share supervision, output validation and lifecycle handling. Keep this common execution path rather than introducing a parallel path for future evaluation.

These observations establish targets, not a claim that every module needs rewriting. Stage A records the concrete symbol inventory and remaining coupling before implementation.

## Naming and module boundaries

Use [UX terms](../../../ux-terms.md) for represented components and their lower-camel/snake-case mappings. Do not rename a distinct technical/domain concept merely because its spelling resembles an old UI label. Examples: protocol transcript role `assistant`, an HTTP session, source storage identity, job state and provider type identifiers retain their precise meanings.

| Current area or name                               | Target responsibility/name                                                                                                                                                      |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `App.tsx`, shell JSX and pane geometry             | `standard-layout/StandardLayout` and layout-only hooks; `App` remains the composition entry                                                                                     |
| Toolbar JSX and shared action buttons              | `standard-layout/MainToolbarStrip`; canonical action labels and existing bitmap rules                                                                                           |
| `Explorer`, navigation views                       | `explorer/Explorer`, `DateTab`, `SnippetTab`, `DraftTab`, `EntryTab`; shared date/import selection stays in the application layer                                               |
| `AuthenticPane`, `GenerationPane`, `candidateTabs` | `tabbed-editor/TabbedEditor`, `AuthenticEditor`, `CandidateEditor` and Editor tab state; generation orchestration remains a use case                                            |
| `ReviewBar`, editor review extension               | `paragraph-review/ReviewControls` and dedicated CodeMirror adapter; deterministic review policy remains outside React                                                           |
| `ChatPane`, `useChat`, chat controller             | `ai-chat/AIChat`, `useAIChat`, `AIChatController`                                                                                                                               |
| `Messages`, `GlobalMessages`                       | `global-messages/GlobalMessages` and `GlobalMessagesStore`; typed message severity, no recovery workflow ownership                                                              |
| Activity views/hooks/runtime                       | `ai-activities` presentation and `AIActivitiesRuntime`; internal transport jobs remain jobs; use the canonical `aiActivity` identifier when representing an individual activity |
| Settings and profile controls                      | `settings` views/cards and `ai-agents` profile status/selection; retain Chat profile versus Generation profile distinction                                                      |
| Trash recovery/dialog/coordinator                  | `trash-recovery` presentation, application-owned Trash operation coordination                                                                                                   |
| Generic Rust values/helpers                        | Context-bearing names such as `GenerationContext`, `AgentProtocolContext`, `ExportCoordinator`; finalize names from their actual responsibility in Stage A                      |

React feature directories live under `src/components`; matching client state/use cases live under `src/application`; IPC validation lives under `src/adapters`; CodeMirror integration lives under `src/editor`. Cross-feature calls use application interfaces rather than importing another feature's view or hook. Avoid barrel exports that obscure dependencies.

The composition root constructs one session-lifetime controller graph and disposes it once. React hooks subscribe and translate user intent; they do not own a second copy of business state. Explorer date selection, open Editor tabs, Authentic persistence and Global Messages have separate owners. AI Activities observes chat/generation jobs and retained results independently of mounted panes. Navigation cannot become a general container for unrelated services.

Rust keeps the existing crates and inward dependency rule. Domain owns values/invariants; application owns orchestration/ports; infrastructure owns SQLite/filesystem, assets and provider protocols; API/native bridge own transport conversion. Extract duplicated policy only into a layer that its callers already depend on. Retain validation on both sides of HTTP/IPC trust boundaries, even when the checks look similar.

## Prompt preparation for Phase 2

### Implement now

Introduce an application-owned prompt preparation port, implemented by the embedded infrastructure catalog. The following names describe the target contracts; Stage A fixes concrete signatures before callers move:

- `PromptId` identifies an app-authored logical prompt. `PromptRevision` identifies exact immutable definition content, including template metadata. Do not use a mutable path or model name as revision identity. Reuse existing hashing facilities through a port; no new dependency is needed.
- `PromptAction` distinguishes chat from candidate generation. Session instructions are a part of a prompt definition, not an independently selectable user action. Profile alias, provider kind, generation style and prompt identity remain distinct.
- `PromptSelection` identifies the bundled default for an action/provider today. No user preference or persisted selector is added. Resolution returns one `ResolvedPrompt` with identity/revision, provider compatibility, template parts and execution defaults.
- Separate `PromptTemplate` content/substitution from `AgentExecutionSettings` (model and reasoning effort). Keep the current front matter as the source for the same effective settings; precedence is asset default only in v1. Do not invent overrides or change verified pins.
- `PreparedAgentRun` captures the resolved definition's provenance, action, profile, locale, input/output contract version, bounded rendered task/session parts and effective execution settings. Bodies and rendered content have no unrestricted `Debug`/serialization/logging path. Existing bounded packet encoding and single-pass substitution remain mandatory.
- Prepare once at admission/capture, before execution can observe a later catalog revision. Generation captures one definition for the whole sequential candidate job; each style/candidate renders from it. Chat captures one definition per admitted turn; an existing turn cannot change because a later turn selects another definition.
- The provider protocol receives prepared parts/settings, never the whole prompt catalog. Apple's locale-rendered session instructions and task prompt are resolved and captured together. A missing required part fails the affected action before inference, with no provider/default fallback.
- Keep a single runner/supervisor/output-validator pipeline. Preparation does not acquire a second inference slot, bypass cancellation, or change the configured profile/version probes.
- Keep provenance transient with the chat turn/generation job/candidate metadata. No prompt-history database or durable content capture is introduced. Redacted diagnostics may carry bounded validated IDs/revisions only through explicit allowlisted fields; never template text, rendered prompts, diary data or arbitrary user-controlled identifiers.

Asset prose stays in `assets/prompts/<agent-type>/`. Keep current paths and format initially; split catalog/parser/renderer into focused infrastructure prompt modules and update the asset README when implemented. Test the immutable catalog with injected synthetic definitions/revisions; this is internal conformance coverage, not a management API.

### Reuse later

Phase 2 can supply a different catalog implementation and explicit selection to the preparation port, then add persistence, editing, revisions and selection policy without modifying provider protocols. Multiple prompt definitions do not mean multiple agent profiles or multiple generated candidates.

Evaluation can later call the same preparation and execution boundaries with explicit bounded inputs and collect validated results plus provenance. Dataset lifecycle, repeatability policy, budgets, scoring/rubrics, judging models, comparison UX and durable reports require a Phase 2 design. Real model output is not promised to be deterministic; schema conformance and successful inference are not quality evaluation. M30 adds no evaluator, scheduler, scoring port, database tables or public evaluation endpoint.

## Compatibility and data safety

Change internal APIs atomically across application, API DTOs/routes, Tauri commands, TypeScript bridge/contracts/validators, fixtures and smoke scripts. Do not retain aliases or compatibility shims merely to preserve old names. Update command registration, manifest allowlists and capability permissions together; verify the existing narrow bridge and rejected secondary-window calls still hold.

Prefer leaving persisted formats and external producer formats unchanged: an internal rename alone does not justify a disk migration. If an actual invariant requires a persisted change, first record the exact old/new format and a non-destructive conversion or explicit refusal strategy, with fixture/restart/interruption tests. Unsupported old data must fail with actionable guidance before mutation. Do not silently drop operation receipts, reservations, export checkpoints or Authentic revision history used for recovery. User data deletion requires separate authorization.

Restart server and client together for incompatible protocol changes; stale discovery/protocol peers must fail explicitly and cannot mutate state under a misunderstood schema. Update version/handshake checks if the changed contract needs them. No dual-protocol support is required.

## Implementation sequence and evidence

1. **Stage A — Baseline and inventory (R01).** Record symbol/path renames, controller owners/lifetimes, dependency violations, duplicated policy and concrete prompt contracts here. Classify every breaking change as internal, transport or persisted. Have the test runner establish baseline failures with repository-documented commands; distinguish them from new regressions.
2. **Stage B — Canonical names (R02, R05).** Move/rename one feature at a time with all callers, tests, labels, CSS and docs. Review moved-code diffs and search for old symbols/paths. Record justified schema/domain exceptions rather than doing blind global replacements.
3. **Stage C — Ownership and boundaries (R03–R04).** Separate layout from composition and view subscription from application lifecycle. Refactor backend use-case/adapter coupling. Preserve save queues, connection epochs, review guards, result budgets and cancel/shutdown arbitration. Run focused behavioral regressions after each responsibility moves.
4. **Stage D — Prompt preparation (R06–R08).** Extract catalog/parser/renderer; introduce application contracts and immutable capture; adapt the common engine and Apple parts; attach transient provenance. Test byte-equivalent current rendering, unchanged execution settings, invalid/missing isolation, no fallback, captured revision stability and fake-runner reuse.
5. **Stage E — Contract/recovery audit (R05, R09–R10).** Verify transport inventories and stale-peer refusal, then affected synthetic server/bridge smokes. Test persisted conversions only if a format changed. Preserve existing autosave, review accept receipts, source observations, export reservations/uncertainty and ordinary close cleanup.
6. **Stage F — Close (R11–R12).** Update maintained architecture, asset README, UX mapping, handoff and this evidence record. Delegate the full gate once on the final tree, fix regressions and rerun affected checks. Check off only evidenced items; update milestone status only after acceptance is complete.

The test runner discovers documented commands and performs first-pass failure analysis. The main agent owns fixes and the acceptance judgment. Synthetic tests use temporary workspaces and fake providers/destinations. Run affected existing smokes; do not infer native/live proof from unit tests. Qwen live tests remain exempt for GPU throttling, and M32 retains packaged/native qualification. A provider protocol or effective prompt change would need fresh adapter evidence; this plan aims to preserve both.

## Risks and limits

- Broad renames can conceal lifecycle regressions: keep renaming and responsibility changes separate and verify reconnect/unmount/cancel/close behavior.
- Over-generalizing for evaluation could add unused machinery: implement only identity, immutable preparation and shared execution seams exercised by v1 and synthetic tests.
- Moving defaults out of the engine can alter effective prompts: compare rendered bytes/settings and capability behavior, including Apple's paired instructions.
- Breaking transport names can miss native permissions or scripts: maintain an explicit inventory and verify rejection paths as well as successful calls.
- Passing automated checks does not complete M32's release evidence, prove output quality or enable multiple-prompt management/evaluation.

## Stage A inventory (2026-11-08)

Recorded before moving code. All changes below are internal; HTTP/SSE JSON,
Tauri command names, permission lists, protocol negotiation, SQLite migrations,
Authentic files, producer documents and recovery receipts retain their formats.
No compatibility aliases are planned.

| Before                                                                  | After / owner                                                                                       |
| ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `components/ChatPane`, `useChat`, `application/chat` / `ChatController` | `components/ai-chat/AIChat`, `useAIChat`, `application/ai-chat/controller` / `AIChatController`     |
| `components/AuthenticPane`                                              | `components/tabbed-editor/AuthenticEditor` (shared editor host, including source/Candidate buffers) |
| `components/GenerationPane`, `candidateTabs`                            | `components/tabbed-editor/CandidateEditor`, `useCandidateTabs`                                      |
| `components/ReviewBar`, `useReview`                                     | `components/paragraph-review/ReviewControls`, `useReview`                                           |
| `components/Explorer`, `SnippetTable`                                   | `components/explorer`                                                                               |
| Activity views and `application/activity` / `ActivityRuntime`           | `components/ai-activities`, `application/ai-activities` / `AIActivitiesRuntime`                     |
| `navigation/messages` / `Messages`                                      | `application/global-messages/store` / `GlobalMessagesStore`                                         |
| Settings/profile views/hooks                                            | `components/settings`, `components/ai-agents`                                                       |
| Trash views                                                             | `components/trash-recovery`                                                                         |
| Layout, toolbar, separator, palette and close/connection hooks          | `components/standard-layout`; App composes features                                                 |
| `generation::Frozen`                                                    | `GenerationContext`                                                                                 |
| `engine::Ctx`                                                           | `AgentProtocolContext`                                                                              |
| `export::Service` / `Inner`                                             | `ExportCoordinator` / `ExportCoordinatorState`                                                      |
| `agent/assets.rs`                                                       | focused prompt catalog/parser/renderer modules                                                      |

Controller ownership: the application composition graph owns one NavigationModel,
GlobalMessagesStore, TrashCoordinator, AIChatController, GenerationController,
ReviewController, ExportController and AIActivitiesRuntime per bridge lifetime.
Navigation owns date/import/catalog selection; ResourceCoordinator owns editor
buffers and Authentic save sessions; GlobalMessagesStore owns notices only;
TrashCoordinator owns operation recovery. Hooks subscribe to controllers and
translate input. StrictMode effect rehearsal must retain the graph; bridge
replacement and final unmount must dispose its exact graph once. Reconnect uses
existing connection epochs and does not recreate controllers.

Prompt contracts: application owns PromptId, PromptRevision, PromptAction,
PromptSelection, ResolvedPrompt, AgentExecutionSettings, PreparedAgentRun and the
preparation port. Infrastructure owns embedded definitions, parsing, rendering
and SHA-256 revision computation. Resolve at admission, capture an immutable
handle per chat turn / generation job, render each bounded input from that
handle, and execute through the existing runner. Provider protocols consume
prepared task/session parts and settings only. Revision hashes cover exact task
and paired session asset bytes (including metadata), with unambiguous framing.
No prompt content is Debug/Serialize. Synthetic catalogs exercise revisions and
missing Apple pairing independently from inference.

Intentional technical names: transcript `assistant`, HTTP `session`, transport
`job`, provider `AgentKind`, source IDs, generation styles and Chat versus
Generation profile names retain their distinct meanings. `draft` in producer
formats is not an Explorer tab. Existing trust-boundary validators remain
independent. Export/source recovery implementation modules are adapters, not
new domain policy; shared supervision remains infrastructure-owned.

### Stage B/C boundary notes

The application graph now owns GlobalMessagesStore and SourceTrashController /
TrashCoordinator. NavigationModel receives the message sink and publishes a
sourceMoved reconciliation method; it no longer constructs notices or recovery
controllers. The connection hook connects navigation and Trash recovery as
separate owners. Explorer receives source-Trash actions explicitly. Layout
components own window structure/geometry, and TabbedEditor composes retained
buffers, review controls and Global Messages. Removed the unused useAuthentic
hook (a second session/editor lifecycle) and ExportController.revive; the graph
lease is now the sole StrictMode lifetime mechanism. Shared profile display
wording moved into application/ai-agents/presentation instead of importing a
React polling hook from Settings policy.

Prompt resolution/preparation and admission capture are now implemented. The
application port owns immutable identities, provenance and prepared runs;
infrastructure separates parsed asset metadata/source, content-only templates
and renderer/catalog adapters. Provider protocols accept only prepared parts /
settings; standalone runs and admitted work share the same executor. Chat
preparation failures retain content-free typed diagnostics. No management or
evaluation policy is exposed. Final evidence is mapped in the overview.

### Final terminology pass inventory (2026-11-09)

Before this pass: ActivityStore/ActivityState/ActivityRecord become
AIActivitiesStore/AIActivitiesState/AIActivityRecord; ActivityStatus, ActivityDetails
and ActivityFeedback gain the AI prefix in files, symbols and callers.
CandidateBanner becomes CandidateEditor, matching its feature module.
CSS region selectors toolbar/workspace/editor-column become
main-toolbar-strip/main-content/tabbed-editor; editor-pane becomes editor-surface
because the retained host also displays source/Candidate buffers. AI Chat classes
use the ai-chat prefix. The review-bar React selector becomes review-controls,
while cm-review-bar remains the technical decorative gutter bar, not controls.
Accessible names become Editor tabs, Explorer tabs and Review controls. No layout,
role, control behavior or transport/storage value changes in this pass.

### Validation harness isolation (2026-11-09)

The first synthetic smoke attempts encountered occupied default app ports and
socket permission failures; no unrelated processes were stopped. A shared
standard-library smoke helper now chooses an unused loopback port for temporary
server configurations, with port_fallback_count zero and authenticated,
process-specific discovery unchanged. This affects validation fixtures only;
production port defaults/configuration and build tools are unchanged. Run the
native Trash harness with --enabled-server to include its server/recovery leg.

### Departures and retained technical structure

No planned transport or persisted rename proved necessary: existing commands,
DTO resource names and durable producer formats represent technical resources,
not obsolete UI regions. Their inventories and trust-boundary tests remain
unchanged. Explorer panels share one navigation controller and remain local
views rather than independent lifecycle components. AuthenticEditor is the
shared retained surface under TabbedEditor; source/Candidate body states stay in
ResourceCoordinator and their Authentic save sessions remain separate objects.
The temporary parsed-asset aggregate was split during audit so PromptTemplate
contains text only, while ParsedPromptAsset owns source/settings for catalog
resolution. These are internal design choices within the authorized refactor,
not new product behavior or compatibility aliases.

The final audit also moved SessionBudget creation into the application graph,
placed the Tabbed Editor accessible region around its actual tabs/surface,
and made validation fixtures independent of occupied production ports. Native
Trash compilation now explicitly selects its library worker artifact. All
validation uses synthetic temporary workspaces, fake inference/destinations and
ordinary debug GUI close; packaged/native M32 evidence remains separate.
