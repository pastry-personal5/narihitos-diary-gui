# P1-M2: Architecture overview

Status: Active

Design the overall software architecture in `docs/architecture.md`, using the research handoff and recorded owner interview. This overview defines the docs-only milestone's scope, sequencing, and verifiable completion checklist.

## Inputs and scope

Read the [owner decisions](milestone-02-owner-decisions.md), [research handoff](milestone-01-research-overview.md#p1-m2-handoff), and relevant linked findings before designing dependent behavior. Follow the [architecture work plan](milestone-02-architecture-architecture.md).

In scope: process/layer boundaries, domain and storage models, APIs/events, frontend ownership, source operations, agents, config, exports, recovery, trust boundaries, and implementation acceptance requirements. Write a reusable overall design with future MCP/web transport seams, while keeping v1 local and personal.

Out of scope: scaffolding, source code, package installation, build-tool changes, user-data mutation, live inference, service writes, remote access, history implementation, and future MCP/web clients. This first change records decisions and plans only; it does not create the overall architecture document or complete P1-M2.

## Sequence

1. Record the interview and reconcile active decisions with the research handoff.
2. Draft `docs/architecture.md` following the architecture work plan. Separate owner contracts, engineering choices, and unverified capabilities.
3. Resolve remaining internal design work under delegated discretion. Ask only for new product choices or changes to approved constraints. Record evidence gaps and the exact dependent implementation they block.
4. Update affected product/UX docs and plan the P1-M3 handoff and later feature/release work. Do not start their implementation in P1-M2.
5. Check documentation consistency, links/anchors, and whitespace; complete the milestone only when all checklist evidence exists.

## Completion checklist

- [x] Record owner decisions with provenance, including superseded research recommendations ([interview record](milestone-02-owner-decisions.md), 2026-11-05).
- [x] Create and index the milestone overview and architecture work plan before architecture drafting (this document and the [work plan](milestone-02-architecture-architecture.md)).
- [x] Mark P1-M2 Active in the roadmap/phase plan and reconcile active contributor/product decisions (linked from [documentation index](../README.md)).
- [ ] Write and index `docs/architecture.md` with deployment, component, and data-flow diagrams and clear dependency direction.
- [ ] Specify API/event/job/revision/error contracts and the Tauri command/permission boundary.
- [ ] Specify workspace/file schemas, authoritative versus rebuildable state, locking, autosave, source mutation, and recovery protocols.
- [ ] Specify paragraph review, agent adapters/process lifecycle, trust policy, configuration/secrets, and export reconciliation.
- [ ] Document timezone/disagreement rules, version strategy, SF Symbols integration route, and dependency candidates without silently approving package/build changes.
- [ ] Reconcile canonical product/UX contracts with the final design; explicitly dispose of every relevant unresolved item in AGENTS.md.
- [ ] Map remaining feasibility gaps to blocked capabilities and acceptance evidence; do not certify unexecuted checks or silently postpone architecture-blocking questions.
- [ ] Define P1-M3 deliverables and subsequent feature/release milestone sequencing before implementation.
- [ ] Check final documentation links, anchors, whitespace, and consistency, with evidence recorded here.
- [ ] Update phase/roadmap status to Done only after all preceding items are complete.

## Validation and current evidence

The [docs-only exception](../development-process.md#definition-of-done) applies: no code gate is required or claimed. Local-link/anchor and whitespace review still apply. Interview decisions are not evidence of runtime behavior, provider terms, source interoperability, packaged filesystem access, or service idempotence.

This preparation change completes only the first three checklist items. The overall architecture and its final validation remain pending.

Preparation validation (2026-11-05): 66 local Markdown links/anchors and whitespace checked across the nine preparation documents; `git diff --check` passed. Reviewed the interview record against the answers and kept engineering proposals separate. The code gate was not run under the docs-only exception. This evidence validates preparation only, not the unfinished architecture checklist.
