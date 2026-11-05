# Phase 1: Diary v1.0.0

Status: Active
Goal: Research, design, and deliver the Diary v1.0.0 app on macOS.

Theme: Deliver Diary v1.0.0 through the [maintained architecture](../architecture.md). Research, architecture and scaffolding remain the first three milestones; the remaining milestones separate durable local workflows, source interoperability, optional agents, AI interaction, exports and personal release.

## Exit criteria

- Research findings are written down and linked from the milestone plans.
- The architecture is documented and any related "Undecided" items in [AGENTS.md](../../AGENTS.md) are resolved.
- The Tauri 2 + React + CodeMirror 6 scaffold builds, runs, and passes the full gate.
- The owner-confirmed v1 Diary workflows from [the research baseline](milestone-01-research-overview.md#working-baseline-and-decision-boundaries) are implemented and verified, with affected product/UX docs completed.
- The macOS v1.0.0 release meets the installation/distribution criteria agreed after R14 research. Feature and release milestone checklists must be added before that work begins.

## Milestones

This phase plan defines the milestone order. P1-M4–P1-M28 are 25 theme-level milestones derived from the architecture, within the owner's limit of 27 milestones after P1-M3. The [implementation handoff](implementation-handoff.md) maps this sequence to contracts and required evidence. Detailed milestone overviews, work plans and verifiable completion checklists will be written before each milestone's implementation, not as part of this theme-planning change. P1-M3 is Done; P1-M4–P1-M28 remain Planned.

### P1-M1: Research

Status: Done
Goal: Gather the knowledge needed to make sound architecture decisions.
Plan: [overview](milestone-01-research-overview.md), including research sequence and evidence requirements.
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

Status: Planned
Goal: Add private server/client diagnostics with bounded asynchronous writes, daily rotation, three-day retention and content redaction.
Plan: [overview](milestone-04-diagnostic-logging-overview.md), [work plan](milestone-04-diagnostic-logging-architecture.md).
Notes: Includes the owner-requested agent-debugging trace to redacted resource detail (trace IDs, keyed fingerprints, read-only `diary-server trace`).

### P1-M5: Workspace and durable operations

Status: Planned
Goal: Establish locked workspace initialization, versioned SQLite authority, durable operation/step identities, resource reservations and startup recovery classification.

### P1-M6: Authentic persistence and recovery

Status: Planned
Goal: Implement canonical Authentic Markdown, guarded durable saves and empty-file creation, saved-date catalogs, external-change detection and interrupted-save recovery.

### P1-M7: Authentic editing and autosave

Status: Planned
Goal: Connect one shared Draft/Entry buffer to ordered IME-aware autosave, Create Empty, acknowledged save feedback and guarded date/close transitions.

### P1-M8: Settings and configuration reload

Status: Planned
Goal: Add guarded client-owned preferences, persistent Writing Language and redacted Settings status with explicit validated server-config reload.

### P1-M9: Source identity and reading

Status: Planned
Goal: Implement bounded Ayumi/Narihito body views, metadata parsing, canonical path identities and fresh observations with independent per-file capability outcomes.

### P1-M10: Civil-date imports and timestamp resolution

Status: Planned
Goal: Import D-1/D/D+1 in a captured system timezone, preserve fixed Entry dates and support explicit timestamp-authority choices without rewriting sources.

### P1-M11: Explorer and editor navigation

Status: Planned
Goal: Complete the resizable Standard Layout, civil-date calendar, grouped source selection, saved-date navigation and resource-owned tabs with guarded refresh and dirty-source handling.

### P1-M12: Command palette and keyboard shortcuts

Status: Planned
Goal: Route actions through one registry and add the command palette plus editable, persistent, scope-aware shortcuts with conflict and IME protection.

### P1-M13: Native symbols and accessible interaction

Status: Planned
Goal: Add allowlisted AppKit SF Symbols and verify keyboard, VoiceOver, focus, pane resizing and Korean/Japanese IME behavior in the packaged macOS interface.

### P1-M14: Producer-compatible source saves

Status: Planned
Goal: Enable explicit body saves only for producer-verified formats, preserving metadata, audio integrity and guarded recovery under concurrent changes.

### P1-M15: Source and audio Trash

Status: Planned
Goal: Add confirmed snippet/audio Trash with exact ownership previews, durable per-member outcomes and safe partial-operation reconciliation and retry.

### P1-M16: Agent contracts and prompt assets

Status: Planned
Goal: Establish the shared agent port, profile capability model, immutable bounded request/output contracts and validated release-packaged prompt assets.

### P1-M17: CLI supervision and Codex adapter

Status: Planned
Goal: Implement bounded process-group execution, cancellation and versioned Codex compatibility through the common agent port.

### P1-M18: Claude Code adapter

Status: Planned
Goal: Add independently verified Claude Code profile checks, one-shot streaming/schema conversion and supervised lifecycle behavior.

### P1-M19: Qwen Code adapter

Status: Planned
Goal: Add independently verified Qwen Code profile checks, one-shot streaming/schema conversion and supervised lifecycle behavior.

### P1-M20: Apple Foundation Models adapter

Status: Planned
Goal: Integrate the optional on-device system model with explicit availability, fresh native sessions, guided-output validation and verified cancellation.

### P1-M21: Transient AI Chat

Status: Planned
Goal: Implement profile-fixed, memory-only chat using the active editor snapshot, saved writing locale and bounded explicit turn/context handling.

### P1-M22: Candidate generation

Status: Planned
Goal: Generate one to five sequential validated candidates from eligible selected snapshots, preserving immutable date/baseline context and independent candidate outcomes.

### P1-M23: Background AI activity and results

Status: Planned
Goal: Complete nonblocking cross-date AI progress, provisional previews, cancellation, reconnect/result discovery and accessible in-app completion feedback.

### P1-M24: Paragraph review and acceptance

Status: Planned
Goal: Add deterministic lossless paragraph/structure review with serialized durable accepts, transient rejects and regeneration after baseline changes.

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
