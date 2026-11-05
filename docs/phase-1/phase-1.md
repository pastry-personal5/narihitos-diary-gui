# Phase 1: Diary v1.0.0

Status: Active
Goal: Research, design, and deliver the Diary v1.0.0 app on macOS.

Theme: Diary v1.0.0. Research, architecture, and scaffolding remain the first three milestones. Plan the feature and release milestones from the research handoff before implementation; expanding the phase does not authorize skipping those foundations.

## Exit criteria

- Research findings are written down and linked from the milestone plans.
- The architecture is documented and any related "Undecided" items in [AGENTS.md](../../AGENTS.md) are resolved.
- The Tauri 2 + React + CodeMirror 6 scaffold builds, runs, and passes the full gate.
- The owner-confirmed v1 Diary workflows from [the research baseline](milestone-01-research-overview.md#working-baseline-and-decision-boundaries) are implemented and verified, with affected product/UX docs completed.
- The macOS v1.0.0 release meets the installation/distribution criteria agreed after R14 research. Feature and release milestone checklists must be added before that work begins.

## Milestones

The [implementation handoff](implementation-handoff.md) defines dependency order and acceptance gates for the remaining work. Detailed plans/checklists are required before implementing each milestone.

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
Status: Planned
Goal: Create the project skeleton following the documented architecture.
Plan: [overview](milestone-03-scaffolding-overview.md), [work plan](milestone-03-scaffolding-architecture.md).
Notes: Scaffolding only: assemble the approved design into a buildable/testable skeleton. Feature implementation remains in P1-M4–P1-M9.

### P1-M4: Authentic persistence
Status: Planned
Goal: Implement locked durable Authentic autosave, authoritative operation recovery and rebuildable catalog behavior.
Plan: [scope and evidence E2](implementation-handoff.md); detailed overview/work plan required before code.

### P1-M5: Source views and GUI
Status: Planned
Goal: Implement source/date import, refresh/conflicts, canonical layout/editor ownership, accessibility and native symbols.
Plan: [scope and evidence E3/E9](implementation-handoff.md); detailed overview/work plan required before code.

### P1-M6: Source mutation
Status: Planned
Goal: Enable producer-compatible body saves and confirmed snippet/audio Trash with partial recovery only after verification.
Plan: [scope and evidence E4](implementation-handoff.md); detailed overview/work plan required before code.

### P1-M7: Agents and review
Status: Planned
Goal: Implement optional CLI and Apple Foundation Models profiles, asset-loaded prompts, transient chat/candidates and saved paragraph accept/reject review.
Plan: [scope and evidence E5/E6/E11](implementation-handoff.md); detailed overview/work plan required before code.

### P1-M8: Optional exports
Status: Planned
Goal: Implement saved-revision iCloud/Notion exports with confirmations, durable receipts and ambiguous-result reconciliation.
Plan: [scope and evidence E7/E8](implementation-handoff.md); detailed overview/work plan required before code.

### P1-M9: Personal release
Status: Planned
Goal: Qualify Diary v1.0.0 for the owner's Apple Silicon Mac and separate shell launch with full workflow/native evidence.
Plan: [release scope](implementation-handoff.md#delivery-order-and-scope); detailed overview/work plan required before code.
