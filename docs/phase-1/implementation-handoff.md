# Phase 1 implementation handoff

Status: Active

This records P1-M2 requirement coverage, evidence limits and delivery order for the remaining Diary v1.0.0 work. Architecture contracts stay independent of delivery numbering in [the maintained design](../architecture.md).

## Owner decision coverage

All internal choices below are engineering decisions under D17, not new interview answers. The [interview](milestone-02-owner-decisions.md) remains authoritative for D01–D17.

| Requirement | Current design evidence |
|---|---|
| D01 independent local server/client and close behavior | [Launch](../architecture/api-and-processes.md#launch-and-discovery), [client ownership](../architecture/api-and-processes.md#authentication-and-client-ownership) |
| D02 HTTP/SSE/token/origins/one client | [API and processes](../architecture/api-and-processes.md) covers discovery, admission, commands, errors, replay gaps and resync |
| D03 personal Apple Silicon shell delivery | [Platform target](../architecture/configuration-and-exports.md#version-and-platform-strategy); no unsupported minimum-version/public-distribution claim |
| D04 workspace/Markdown/SQLite authority | [Schema](../architecture/domain-and-persistence.md#workspace-and-file-schema), [table responsibilities](../architecture/domain-and-persistence.md#sqlite-responsibilities) |
| D05 history deferred | [Content model](../architecture/domain-and-persistence.md#workspace-and-file-schema) and recovery payload cleanup; no Git/history feature |
| D06 one Authentic, autosave/close failure | [Autosave](../architecture/domain-and-persistence.md#autosave-protocol), [UX transitions](../ux-gui.md#save-review-and-transition-feedback) |
| D07 fixed paragraph accept/reject review | [Alignment](../architecture/editing-and-agents.md#paragraph-alignment-and-review), including structural units, inserted/deleted paragraphs and stale invalidation |
| D08 path views, rename/refresh/dirty guards | [Path identity](../architecture/sources-and-dates.md#path-identity-and-observations) |
| D09 metadata/save compatibility/time conflicts | [Parsing](../architecture/sources-and-dates.md#parsing-and-timestamps), [explicit source saves](../architecture/sources-and-dates.md#explicit-source-saves) |
| D10 Trash and partial retry | [Related audio and Trash](../architecture/sources-and-dates.md#related-audio-and-trash) |
| D11 optional runtime profiles, sequential count, transient chat | [Agent port](../architecture/editing-and-agents.md#agent-port-and-profiles), [bounds](../architecture/editing-and-agents.md#context-output-and-limits) |
| Owner update: Apple Foundation Models and external prompts | [Native adapter](../architecture/editing-and-agents.md#apple-foundation-models-adapter), [prompt assets](../../assets/prompts/README.md); native acceptance remains E11 |
| D12 trusted normal CLI configuration | [Trust/lifecycle](../architecture/editing-and-agents.md#trust-and-process-lifecycle); no tool/retention-confinement promise |
| D13 explicit private YAML and literal Notion token | [Configuration](../architecture/configuration-and-exports.md#configuration-schema-and-validation) |
| D14 optional saved exports, ascending sections and external edits | [Shared reconciliation](../architecture/configuration-and-exports.md#export-input-and-shared-reconciliation), [Notion](../architecture/configuration-and-exports.md#notion-hierarchy-projection-and-recovery) |
| D15 original GUI | [Containment](../ux-information-architecture.md), [terms](../ux-terms.md), [interactions](../ux-gui.md) |
| D16 SF Symbols | [Native rendering route](../architecture/configuration-and-exports.md#version-and-platform-strategy); no exported asset bundle or license-clearance claim |
| D17 delegated design | All themed contracts; [dependency candidates](../architecture/configuration-and-exports.md#dependency-candidates) are recommendations only |
| Earlier import zone/fixed Entry dates | [Calendar boundaries](../architecture/sources-and-dates.md#import-zone-and-calendar-boundaries) |
| Clean Architecture and readable split docs | [Dependency rule and diagrams](../architecture.md#dependency-rule), nine independently indexed contract documents |
| Review owner answers: writing language, empty creation and background work | [Review interview](milestone-02-architecture-review.md#owner-interview), persistent [writing preference](../architecture/user-settings.md), durable [Create Empty](../architecture/domain-and-persistence.md#workspace-and-file-schema) and [background AI experience](../architecture/ai-job-experience.md) |
| Version, future transports, gate coverage | [Version strategy](../architecture/configuration-and-exports.md#version-and-platform-strategy), [extension seams](../architecture.md#failure-containment-and-extension-seams), [module allocation](../architecture.md#component-allocation) |

This disposes of all former internal-contract gaps in AGENTS.md: path normalization/collisions, review alignment, schemas, recovery, timestamp choice, skipped/folded dates, metadata/audio ownership, export reconciliation, auth/discovery and versions. It does not certify runtime feasibility. Source producer-specific mutations are deliberately capability-gated; inventing a producer catalog protocol would contradict the evidence boundary.

The P1-M2 [architecture review](milestone-02-architecture-review.md#findings-and-design-fixes) corrects operation/continuation recovery, review acceptance, resource reservations, delayed remote commits, settings, log/transport bounds and implementation evidence coverage. Its acceptance column extends E1–E12; E13 adds AI latency feedback. The seven interview answers are resolved, including initial language, background/date behavior, persisted empty creation, in-app attention and regeneration after changed baselines. The [P1-M2 completion record](../archive/milestones/p1-m2/milestone-02-architecture-overview.md#architecture-clarifications-and-review) owns the design/clarification evidence; P1-M3 is scaffolding only. No runtime check is marked passed by a contract correction.

## Capability evidence register

No check in this table has run as part of P1-M2. These are explicit blockers on enabling the capability, with bounded acceptance work in its consuming milestone. The owner has selected deployment/trust direction; feasibility claims remain conditional rather than silently settled.

| ID and blocked capability | Safe behavior without proof | Required acceptance evidence and destination |
|---|---|---|
| E1 authenticated bridge/discovery and native access | No production connection/path access claim | P1-M3: two independent shell commands; temporary workspace; token/origin/Host failures, occupied-port fallback, stale discovery, second-client rejection, denied Tauri window/command, packaged server/client identities and synthetic denied/readable folders |
| E2 durable autosave/recovery | No durable-save claim from a debounce or mock | P1-M4: temporary-workspace interruption at every sync/commit/publication boundary, original-ID retry, stale/external writes, lock exclusion, corrupt/missing DB and receipts, macOS directory durability; Create Empty with no snippets/agent, revision 1, restart listing, repeated ID, concurrent-create/no-clobber and failed/uncertain publication |
| E3 source read/date coverage | Report per-file unsupported/conflict/unavailable | P1-M5: synthetic adapter/schema/limits/path and complete zone inclusion matrix from the source/date contract; no private samples in fixtures |
| E4 producer-compatible source writes/deletes | Read supported text; disable write_body/trash_pair independently | P1-M6: synthetic mobile/Ayumi producer round trips, unknown metadata and required updated fields, audio/catalog recovery, shared/ambiguous ownership, cloud unavailable, native Trash per-member failure/lost reply/retry. Absent Ayumi producer evidence keeps its mutations blocked |
| E5 process supervision and real adapter output | Missing/unverified profile disabled; editor usable | P1-M7: fake child/grandchild and flooding/hung pipes, cancellation/lease/close/shutdown, compatibility fixtures for each exact CLI, then owner-authorized synthetic live schema turn per profile. No confinement tests required to reverse D12; account/retention terms remain unverified |
| E6 paragraph review correctness | No whole-file fallback adoption | P1-M7: unit/span/LCS fixtures, independent accept/reject, exact byte reconstruction, ack loss/restart/stale baseline and structural/limit errors |
| E7 iCloud availability/conflict/recovery | Local saving usable; export unavailable or uncertain | P1-M8: real synthetic provider files in packaged context, evicted/download-needed/denied cases, external changes, lost reply/receipt and confirmation freshness. Local write never proves remote upload |
| E8 Notion reconciliation and limits | No remote writes/automatic ambiguous replay | P1-M8: fake HTTP matrix and explicitly authorized synthetic workspace round trip for API permissions, pagination, order/chunks/partial update/lost replies/concurrent edits |
| E9 SF Symbols/native UX | Text-labeled scaffold; no bundled downloaded symbols | P1-M5 and P1-M9: allowlisted native symbol lookup, unavailable-symbol label, light/dark/Retina rendering; keyboard/VoiceOver/IME/focus/pane tests. Requested route is not proof of licensing clearance |
| E10 dependencies/toolchain/support | No new manifests or tools installed by P1-M2 | P1-M3: obtain package/tool approval after exact-version/license/features review, verify selected Rust/Node/Tauri/macOS set; P1-M9 qualifies only personal Apple Silicon/shell launch |
| E11 Apple Foundation Models and prompt assets | Native profile/action unavailable until its integration and assets are validated; editing and verified CLI profiles remain usable | P1-M3: typed config accepts alias/type-only Apple profiles and rejects CLI fields; no inference at startup. P1-M7: approved native SDK/bridge/build integration, all availability reasons and mid-run loss, actual packaged synthetic schema/chat/generation, context/language/refusal handling, cancellation/deadlines and late-result suppression. Validate prompt metadata, single-pass substitution, packaged assets, missing/malformed errors and action/schema compatibility for every enabled adapter; existing templates are not runtime proof |
| E12 logging, shortcuts and user settings | Diagnostics/bindings fall back without affecting saving; bad settings show a warning, newer schemas stay read-only, and live AI requires a resolved valid writing locale | P1-M5: [logging](../architecture/logging.md) async/cap/72-hour pruning/redaction and failure isolation; [user settings](../architecture/user-settings.md) section/version fallback, stale updates, backed-up recovery, pre/post-publication failures and writing-locale persistence; [shortcut](../architecture/keyboard-shortcuts.md) registry/default-evolution conflicts and Settings controls. P1-M5/P1-M9: actual WKWebView key/IME/restart qualification; mocks are not native-key proof. P1-M3 does not implement these complete features |
| E13 responsive background AI experience | Keep observed activity pending/stale and editing usable; no invented progress, implicit retry, notification permission or measured-responsiveness claim | P1-M5: activity/detail/result controls and fake-clock slow/no-output/flood/cancel scenarios in the canonical layout, progress snapshots and bounded UI updates. P1-M7/P1-M9: [complete scenario matrix](../architecture/ai-job-experience.md#accessibility-and-evidence), real-adapter milestones, off-date results and changed-baseline regeneration, partial outcomes, locale snapshots, packaged editing/autosave under a slow run, measured 100 ms feedback target, VoiceOver/IME/reduced motion, activation/sleep and disconnect; no native notifications or automatic extra inference. P1-M3 fake jobs cover transport admission/cancellation/resync only |

## Delivery order and scope

P1-M1 and P1-M2 are documentation-only under the [process exception](../development-process.md#definition-of-done). P1-M3 and later require the full code gate. These milestones remain sequential; dependent capabilities may not ship by silently dropping a failed acceptance requirement. Each later milestone needs its own overview/checklist and architecture work plan before code begins.

Milestone ownership: P1-M2 owns architecture design, owner clarifications/interviews, review and acceptance planning. P1-M3 consumes those decisions to create the buildable skeleton; feature persistence, detailed UI and real adapters remain in P1-M4–P1-M9. Moving design evidence to P1-M2 does not mark its future implementation tests passed.

| Milestone | Deliverables and acceptance focus |
|---|---|
| P1-M3 Scaffolding | [Overview](milestone-03-scaffolding-overview.md), [work plan](milestone-03-scaffolding-architecture.md); inward crate skeleton, two binaries, configuration/discovery/auth/session wiring, fake-status/job HTTP/SSE, typed Tauri bridge, minimal text-labeled layout/CodeMirror, version consistency, test infrastructure and full gate; E1/E10 and E11 profile schema only |
| P1-M4 Authentic persistence | Workspace/schema/SQLite migrations, locked ordered autosave, durable explicit Create Empty and error UI, crash/retry/rebuild tests, E2; no history |
| P1-M5 Source views and GUI | Read adapters, path identity, captured-zone import/refresh and conflict choice; complete canonical layout, shared editor buffers, Create Empty/activity controls, bounded logging, persistent settings and shortcut editing, keyboard/IME and native symbols, E3/E9/E12 and fake-job UX portion of E13 |
| P1-M6 Source mutation | Independently gated body saves/validated audio Trash, preview/confirmation/partial retry and producer compatibility, E4 |
| P1-M7 Agents and review | Three optional one-shot CLI adapters plus optional native Apple Foundation Models, asset-loaded locale snapshots, transient chat/candidates, sequential background generation, paragraph decisions and nonblocking progress/completion; native lifecycle/supervision and complete review/latency matrix, E5/E6/E11/E13 |
| P1-M8 Optional exports | Saved-revision iCloud/Notion, durable receipts/chunks, external-edit confirmations and ambiguous-result reconciliation, E7/E8 |
| P1-M9 Personal release | Owner's Mac end-to-end synthetic workflow, offline/missing-CLI/save-failure scenarios, final full gate, shell-install/run instructions, qualified version/platform report and residual limitations |

Release cannot be described as complete for a blocked required workflow. If producer/native/service evidence remains unobtainable, keep its feature/release checklist unchecked and obtain an explicit owner scope change rather than marking it implemented. No real user-data mutation, inference or service write is part of this planning change.
