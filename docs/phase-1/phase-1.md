# Phase 1: Diary v1.0.0

Status: Active
Goal: Research, design, and deliver the Diary v1.0.0 app on macOS.

Theme: Deliver Diary v1.0.0 through the [maintained architecture](../architecture.md). Research, architecture and scaffolding remain the first three milestones; the remaining milestones separate durable local workflows, source interoperability, optional agents, AI interaction, exports and personal release.

## Exit criteria

- Research findings are written down and linked from the milestone plans.
- The architecture is documented and any related "Undecided" items in [AGENTS.md](../../AGENTS.md) are resolved.
- The Tauri 2 + React + CodeMirror 6 scaffold builds, runs, and passes the full gate.
- The owner-confirmed v1 Diary workflows from [the research baseline](../archive/milestones/p1-m1/milestone-01-research-overview.md#working-baseline-and-decision-boundaries) are implemented and verified, with affected product/UX docs completed.
- The macOS v1.0.0 release meets the installation/distribution criteria agreed after R14 research. Feature and release milestone checklists must be added before that work begins.

## Milestones

This phase plan defines the milestone order. P1-M4–P1-M28 are 25 theme-level milestones derived from the architecture, within the owner's limit of 27 milestones after P1-M3. The [implementation handoff](implementation-handoff.md) maps this sequence to contracts and required evidence. Detailed milestone overviews, work plans and verifiable completion checklists will be written before each milestone's implementation, not as part of this theme-planning change. P1-M4 is Done; P1-M5 is Done; P1-M6 is Done; P1-M7 is Done; P1-M8 is Done; P1-M9 is Done; P1-M10 is Done; P1-M11–P1-M15 are Done; P1-M16 is Done; P1-M17 is Done; P1-M18 is Done; P1-M19 is Done; P1-M20 is Done; P1-M21 is Done; P1-M22 is Done; P1-M23 is Done; P1-M24 is Done; P1-M25–P1-M28 remain Planned.

### P1-M1: Research

Status: Done
Goal: Gather the knowledge needed to make sound architecture decisions.
Plan: [overview](../archive/milestones/p1-m1/milestone-01-research-overview.md), including research sequence and evidence requirements.
Notes: Documentation research and its checklist are complete under the docs-only exemption. The overview carries unresolved owner choices and unexecuted feasibility checks into P1-M2 as blockers on dependent decisions, not established capabilities.

### P1-M2: Architecture

Status: Done
Goal: Decide and document the application architecture, based on the research and owner interview.
Plan: [overview](../archive/milestones/p1-m2/milestone-02-architecture-overview.md), [architecture work plan](../archive/milestones/p1-m2/milestone-02-architecture-architecture.md), [owner decisions](milestone-02-owner-decisions.md), [review/interview](milestone-02-architecture-review.md).
Notes: [Clean Architecture](../architecture.md) and nine themed contracts complete; configuration/agent/prompt clarifications, architecture review and background-AI design belong to the [P1-M2 completion record](../archive/milestones/p1-m2/milestone-02-architecture-overview.md#architecture-clarifications-and-review). The [handoff](implementation-handoff.md) maps owner decisions and conditional capabilities to later implementation evidence. Documentation-only validation applies; no runtime capability is certified.

### P1-M3: Scaffolding

Status: Done
Goal: Create the project skeleton following the documented architecture.
Plan: [overview](../archive/milestones/p1-m3/milestone-03-scaffolding-overview.md), [work plan](../archive/milestones/p1-m3/milestone-03-scaffolding-architecture.md).
Notes: Scaffolding only: S01–S10/G01–G06 map approved setup, the implemented command subset, fixture transport, build/type checks and packaged evidence to verifiable acceptance. Workspace initialization/recovery and all feature implementation remain in P1-M4–P1-M28; completed 2026-11-05 with S01–S10/G01–G06 evidence in the archived overview.

### P1-M4: Diagnostic logging

Status: Done
Goal: Add private server/client diagnostics with bounded asynchronous writes, daily rotation, three-day retention and content redaction.
Plan: [overview](../archive/milestones/p1-m4/milestone-04-diagnostic-logging-overview.md), [work plan](../archive/milestones/p1-m4/milestone-04-diagnostic-logging-architecture.md).
Notes: Includes the owner-requested agent-debugging trace to redacted resource detail (trace IDs, keyed fingerprints, read-only `diary-server trace`).

### P1-M5: Workspace and durable operations

Status: Done
Goal: Establish locked workspace initialization, versioned SQLite authority, durable operation/step identities, resource reservations and startup recovery classification.
Plan: [overview](../archive/milestones/p1-m5/milestone-05-workspace-durable-operations-overview.md), [work plan](../archive/milestones/p1-m5/milestone-05-workspace-durable-operations-architecture.md).
Notes: `rusqlite` approved 2026-11-05.

### P1-M6: Authentic persistence and recovery

Status: Done
Goal: Implement canonical Authentic Markdown, guarded durable saves and empty-file creation, saved-date catalogs, external-change detection and interrupted-save recovery.
Plan: [overview](../archive/milestones/p1-m6/milestone-06-authentic-persistence-overview.md), [work plan](../archive/milestones/p1-m6/milestone-06-authentic-persistence-architecture.md).
Notes: `notify`, `objc2` and `objc2-foundation` approved 2026-11-05; Authentic Tauri commands move to P1-M7.

### P1-M7: Authentic editing and autosave

Status: Done
Goal: Connect one shared Draft/Entry buffer to ordered IME-aware autosave, Create Empty, acknowledged save feedback and guarded date/close transitions.
Plan: [overview](../archive/milestones/p1-m7/milestone-07-authentic-editing-overview.md), [work plan](../archive/milestones/p1-m7/milestone-07-authentic-editing-architecture.md).

### P1-M8: Settings and configuration reload

Status: Done
Goal: Add guarded client-owned preferences, persistent Writing Language and redacted Settings status with explicit validated server-config reload.
Plan: [overview](../archive/milestones/p1-m8/milestone-08-settings-reload-overview.md), [work plan](../archive/milestones/p1-m8/milestone-08-settings-reload-architecture.md).

### P1-M9: Source identity and reading

Status: Done
Goal: Implement bounded Ayumi/Narihito body views, metadata parsing, canonical path identities and fresh observations with independent per-file capability outcomes.
Plan: [overview](../archive/milestones/p1-m9/milestone-09-source-reading-overview.md), [work plan](../archive/milestones/p1-m9/milestone-09-source-reading-architecture.md).

### P1-M10: Civil-date imports and timestamp resolution

Status: Done
Goal: Import D-1/D/D+1 in a captured system timezone, preserve fixed Entry dates and support explicit timestamp-authority choices without rewriting sources.
Plan: [overview](../archive/milestones/p1-m10/milestone-10-civil-date-imports-overview.md), [work plan](../archive/milestones/p1-m10/milestone-10-civil-date-imports-architecture.md).

### P1-M11: Explorer and editor navigation

Status: Done
Goal: Complete the resizable Standard Layout, civil-date calendar, grouped source selection, saved-date navigation and resource-owned tabs with guarded refresh and dirty-source handling.
Plan: [overview](../archive/milestones/p1-m11/milestone-11-explorer-navigation-overview.md), [work plan](../archive/milestones/p1-m11/milestone-11-explorer-navigation-architecture.md).
Notes: Closed with the owner-waived packaged navigation smoke (2026-11-06); no new dependency or producer mutation capability.

### P1-M12: Command palette and keyboard shortcuts

Status: Done
Goal: Route actions through one registry and add the command palette plus editable, persistent, scope-aware shortcuts with conflict and IME protection.
Plan: [overview](milestone-12-command-palette-shortcuts-overview.md), [work plan](milestone-12-command-palette-shortcuts-architecture.md).

### P1-M13: Native symbols and accessible interaction

Status: Done
Goal: Add allowlisted AppKit SF Symbols and close the keyboard, focus, pane-resizing and IME-safe-save gaps that automated tests can reach; packaged key, resize and Korean IME proof is carried to P1-M28; native VoiceOver proof are not required.
Plan: [overview](milestone-13-native-symbols-accessibility-overview.md), [work plan](milestone-13-native-symbols-accessibility-architecture.md).
Notes: Owner closes on automated tests only; native proof moves to P1-M28.

### P1-M14: Producer-compatible source saves

Status: Done
Goal: Enable explicit body saves only for producer-verified formats, preserving metadata, audio integrity and guarded recovery under concurrent changes.
Plan: [overview](milestone-14-producer-compatible-source-saves-overview.md), [work plan](milestone-14-producer-compatible-source-saves-architecture.md).
Notes: Narihito mobile schema 1 only; Ayumi stays disabled without a producer contract. The save was enabled after `scripts/smoke/producer.py` passed (format-level evidence only; phone-app and live-iCloud proof is carried to P1-M28). Owner decisions (2026-11-06): the title is left unchanged, the producer harness is sufficient evidence to enable the save, and unresolved iCloud conflict versions are a recorded known gap.

### P1-M15: Source and audio Trash

Status: Done
Goal: Add confirmed snippet/audio Trash with exact ownership previews, durable per-member outcomes and safe partial-operation reconciliation and retry.
Plan: [overview](../archive/milestones/p1-m15/milestone-15-source-audio-trash-overview.md), [work plan](../archive/milestones/p1-m15/milestone-15-source-audio-trash-architecture.md).
Notes: Narihito schema 1 Trash is enabled after native process-fault/receipt/restart and pinned producer catalog/audio/reopen evidence, ordinary server integration and the full gate. Ayumi stays disabled. Retained empty assets directories produce a producer recovery row; phone/live-iCloud remains unverified without a manual exit gate.

### P1-M16: Agent contracts and prompt assets

Status: Done
Goal: Establish the shared agent port, profile capability model, immutable bounded request/output contracts and validated release-packaged prompt assets.
Plan: [overview](milestone-16-agent-contracts-prompt-assets-overview.md), [work plan](milestone-16-agent-contracts-prompt-assets-architecture.md).
Notes: Library-only; no adapter, route, command or UI. Claude and Qwen assets wait for P1-M18/P1-M19; Codex model/effort pins pass through unverified until P1-M17.

### P1-M17: CLI supervision and Codex adapter

Status: Done
Goal: Implement bounded process-group execution, cancellation and versioned Codex compatibility through the common agent port.
Plan: [overview](milestone-17-cli-supervision-codex-adapter-overview.md), [work plan](milestone-17-cli-supervision-codex-adapter-architecture.md).
Notes: Allowlists `codex-cli 0.167.0` only. Adds read-only profile status (routes, bridge commands, Settings) but no run-starting surface. An automated live Codex test records evidence; no human test is an exit gate. Done 2026-11-06 ([evidence](milestone-17-cli-supervision-codex-adapter-overview.md#evidence-record)); the pins were accepted.

### P1-M18: Claude Code adapter

Status: Done 2026-11-06
Goal: Add independently verified Claude Code profile checks, one-shot streaming/schema conversion and supervised lifecycle behavior.
Plan: [overview](milestone-18-claude-code-adapter-overview.md), [work plan](milestone-18-claude-code-adapter-architecture.md).
Notes: Allowlists the `2.1.x` family (fixtures from the single captured patch); pins `claude-sonnet-4-6`/`low`; about 5 live turns on the default login, no human gate; shared host extracted from the Codex adapter; settings isolated; no spend cap. No run-starting surface.

### P1-M19: Qwen Code adapter

Status: Done 2026-11-06
Goal: Add independently verified Qwen Code profile checks, one-shot streaming/schema conversion and supervised lifecycle behavior.
Plan: [overview](milestone-19-qwen-code-adapter-overview.md), [work plan](milestone-19-qwen-code-adapter-architecture.md).
Notes: Allowlists the `0.26.x` family; `ready` after `--version` only (no auth probe exists); pins `qwen3.6:35b-a3b` with no effort pin; agent-run live test on the local endpoint, no human gate; isolation by `--safe-mode` (`--bare` was tested and drops the endpoint); the model pin is checked against the init record because the CLI silently falls back. No run-starting surface. Done 2026-11-06 ([evidence](milestone-19-qwen-code-adapter-overview.md#evidence-record)).

### P1-M20: Agent abstraction, diagnostics and Apple Foundation Models adapter

Status: Done 2026-11-06
Goal: Give all four agents one engine, registry and typed diagnostic vocabulary, then integrate the optional on-device system model with explicit detailed availability reasons, fresh native sessions, guided-output validation and verified cancellation.
Plan: [overview](milestone-20-apple-foundation-models-adapter-overview.md), [work plan](milestone-20-apple-foundation-models-adapter-architecture.md).
Notes: Staged A abstractions (generic engine and registry; Codex, Claude and Qwen ported with no behavior change), B typed content-free diagnostics for every failure, blocked and locked state, C Apple adapter. Swift helper process; `build.rs` compiles it with `swiftc` and skips softly (`helper_missing`); closed reason set shown in Settings; agent-run live test, no human gate; no run-starting surface. Delivered 2026-11-06 with a passing live Apple run on macOS 26.7.1; see the [evidence record](milestone-20-apple-foundation-models-adapter-overview.md#evidence-record).

### P1-M21: Transient AI Chat

Status: Done 2026-11-06
Goal: Implement profile-fixed, memory-only chat using the active editor snapshot, saved writing locale and bounded explicit turn/context handling.
Plan: [overview](milestone-21-transient-ai-chat-overview.md), [work plan](milestone-21-transient-ai-chat-architecture.md).
Notes: Also builds the real job layer (single-run busy gate, runner, bounded SSE progress) that P1-M22 reuses; streams provisional text; server keeps the transcript for the lease; one live turn plus cancel per adapter, all passing ([evidence](milestone-21-transient-ai-chat-overview.md#evidence-record)).

### P1-M22: Candidate generation

Status: Done (2026-11-06; Qwen live proof exempt: GPU throttling)
Goal: Generate one to five sequential validated candidates from eligible selected snapshots, preserving immutable date/baseline context and independent candidate outcomes.
Plan: [overview](../archive/milestones/p1-m22/milestone-22-candidate-generation-overview.md), [work plan](../archive/milestones/p1-m22/milestone-22-candidate-generation-architecture.md).

### P1-M23: Background AI activity and results

Status: Done
Goal: Complete nonblocking cross-date AI progress, provisional previews, cancellation, reconnect/result discovery and accessible in-app completion feedback.
Plan: [overview](../archive/milestones/p1-m23/milestone-23-background-ai-activity-results-overview.md), [work plan](../archive/milestones/p1-m23/milestone-23-background-ai-activity-results-architecture.md).
Notes: Completed 2026-11-07 on automated tests, synthetic built-server smoke and the full gate; packaged slow-run, responsiveness, native interaction and reconnect/wake proof moves explicitly to P1-M28. Native VoiceOver proof remains exempt.

### P1-M24: Paragraph review and acceptance

Status: Done
Goal: Add deterministic lossless paragraph/structure review with serialized durable accepts, transient rejects and regeneration after baseline changes.
Plan: [overview](../archive/milestones/p1-m24/milestone-24-paragraph-review-overview.md), [work plan](../archive/milestones/p1-m24/milestone-24-paragraph-review-architecture.md); owner decisions O1–O3 recorded (2026-11-07).

### P1-M25: Export coordination and confirmations

Status: Planned
Goal: Establish saved-revision export snapshots, destination reservations, authoritative receipts and checkpointed conflict confirmation and reconciliation.

### P1-M26: iCloud export

Status: Planned
Goal: Export exact saved Markdown through verified File Provider availability and coordinated publication, with external-change confirmation and honest local-write outcomes.

### P1-M27: Notion export

Status: Planned
Goal: Export deterministic year/month/daily projections with ascending dates, bounded chunking, service-limit handling and provenance-aware ambiguous-write recovery.

### P1-M28: Personal release qualification

Status: Planned
Goal: Qualify Diary v1.0.0 on the owner's Apple Silicon Mac with separate shell launch, complete workflow/recovery/native evidence and the full release gate.

Notes: P1-M28 also owns the packaged AI proof moved from M23 by the owner on 2026-11-06: editing/autosave under a slow synthetic run and maximum streaming, toolbar/details and off-date results, native keyboard/reduced motion, connection recovery and activation after sleep, measured submission feedback (100 ms target), cancel acknowledgement and render/memory bounds. See the [M23 evidence limits](../archive/milestones/p1-m23/milestone-23-background-ai-activity-results-architecture.md#gate-and-evidence-limits). Native VoiceOver proof remains exempt.
