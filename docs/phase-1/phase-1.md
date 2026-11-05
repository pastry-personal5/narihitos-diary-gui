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

The three milestones below are the first ones planned. More milestones will be added to this phase as the work becomes clear.

### P1-M1: Research
Status: Done
Goal: Gather the knowledge needed to make sound architecture decisions.
Plan: [overview](milestone-01-research-overview.md), including research sequence and evidence requirements.
Notes: Documentation research and its checklist are complete under the docs-only exemption. The overview carries unresolved owner choices and unexecuted feasibility checks into P1-M2 as blockers on dependent decisions, not established capabilities.

### P1-M2: Architecture
Status: Active
Goal: Decide and document the application architecture, based on the research and owner interview.
Plan: [overview](milestone-02-architecture-overview.md), [architecture work plan](milestone-02-architecture-architecture.md), [owner decisions](milestone-02-owner-decisions.md).
Notes: Decisions and plans recorded first; `docs/architecture.md` remains pending. Exempt from the code gate; done only when its completion checklist has evidence.

### P1-M3: Scaffolding
Status: Planned
Goal: Create the project skeleton following the documented architecture.
Plan: overview and architecture docs to be created before implementation.
