# P1-M2: Architecture overview

Status: Done

Completed the Clean Architecture design in [the maintained overview](../../../architecture.md) and the themed contracts, using the research handoff and recorded owner interview. This archived plan records the docs-only milestone's scope, checklist and acceptance evidence.

## Inputs and scope

Read the [owner decisions](../../../phase-1/milestone-02-owner-decisions.md), [research handoff](../../../phase-1/milestone-01-research-overview.md#p1-m2-handoff), and relevant linked findings before designing dependent behavior. Follow the [architecture work plan](milestone-02-architecture-architecture.md).

In scope: process/layer boundaries, domain and storage models, APIs/events, frontend ownership, source operations, agents, config, exports, recovery, trust boundaries, and implementation acceptance requirements. Write a reusable overall design with future MCP/web transport seams, while keeping v1 local and personal.

Out of scope: scaffolding, source code, package installation, build-tool changes, user-data mutation, live inference, service writes, remote access, history implementation, and future MCP/web clients.

## Sequence

1. Record the interview and reconcile active decisions with the research handoff.
2. Draft `docs/architecture.md` following the architecture work plan. Separate owner contracts, engineering choices, and unverified capabilities.
3. Resolve remaining internal design work under delegated discretion. Ask only for new product choices or changes to approved constraints. Record evidence gaps and the exact dependent implementation they block.
4. Update affected product/UX docs and plan the P1-M3 handoff and later feature/release work. Do not start their implementation in P1-M2.
5. Check documentation consistency, links/anchors, and whitespace; complete the milestone only when all checklist evidence exists.

## Completion checklist

- [x] Record owner decisions with provenance, including superseded research recommendations ([interview record](../../../phase-1/milestone-02-owner-decisions.md), 2026-11-05).
- [x] Create and index the milestone overview and architecture work plan before architecture drafting (this document and the [work plan](milestone-02-architecture-architecture.md)).
- [x] Mark P1-M2 Active in the roadmap/phase plan and reconcile active contributor/product decisions (linked from [documentation index](../../../README.md)).
- [x] Write and index `docs/architecture.md` with deployment, component, and data-flow diagrams and clear dependency direction.
- [x] Specify API/event/job/revision/error contracts and the Tauri command/permission boundary.
- [x] Specify workspace/file schemas, authoritative versus rebuildable state, locking, autosave, source mutation, and recovery protocols.
- [x] Specify paragraph review, agent adapters/process lifecycle, trust policy, configuration/secrets, and export reconciliation.
- [x] Document timezone/disagreement rules, version strategy, SF Symbols integration route, and dependency candidates without silently approving package/build changes.
- [x] Reconcile canonical product/UX contracts with the final design; explicitly dispose of every relevant unresolved item in AGENTS.md.
- [x] Map remaining feasibility gaps to blocked capabilities and acceptance evidence; do not certify unexecuted checks or silently postpone architecture-blocking questions.
- [x] Define P1-M3 deliverables and subsequent feature/release milestone sequencing before implementation.
- [x] Check final documentation links, anchors, whitespace, and consistency, with evidence recorded here.
- [x] Update P1-M2 status to Done in the phase/roadmap only after all preceding items are complete; Phase 1 remains Active.

### Architecture audit added before drafting

- [x] Map D01–D17 and every work-plan theme to maintained architecture sections.
- [x] Define and illustrate inward dependencies, separately launched processes, and save/generation/export flows.
- [x] Give each unverified capability a fail-closed outcome, destination milestone, and concrete acceptance evidence.
- [x] Replace canonical UX placeholders from the approved layout and delegated interaction contracts.
- [x] Record a requirement-by-requirement documentation audit and archive completed milestone plans with links repaired.

### Owner-requested milestone organization correction

The owner assigned architecture clarifications, review and AI experience design to P1-M2, and limited P1-M3 to scaffolding. This correction updates the archived completion record's classification; it does not reopen implementation or certify runtime capabilities.

- [x] Move the configuration, agent/prompt, architecture-review and background-AI design records from the P1-M3 overview into this P1-M2 record.
- [x] Identify the review/interview record as P1-M2 and repair incoming links, index and phase references.
- [x] Keep P1-M3 limited to skeleton assembly and validation; map feature implementations to their consuming milestones in the handoff.
- [x] Check local links/anchors, whitespace and milestone-scope consistency; record documentation evidence separately from the code gate.

Reorganization evidence (2026-11-05): checked 48 first-party Markdown files including the affected archived plans, 356 local links and 96 heading anchors, whitespace/title conventions, all four relocated sections, review-record naming/index coverage and preservation of the six-command scaffold gate; zero errors. `git diff --check` passed. The code gate is not required for this P1-M2 documentation correction, and P1-M3 implementation remains unchecked.

## Architecture clarifications and review

The following completed documentation work belongs to P1-M2. The owner requested moving these records out of P1-M3; their original validation evidence and runtime limits are preserved.

### Owner configuration clarification

This documentation clarification prepares the configuration contract for scaffolding; implementation and its full gate remain pending.

- [x] Require absolute `paths.entries_root` and `paths.runtime_root`, use `agents.profiles[].profile_home`, and define optional `executable` overrides with type-specific `PATH` defaults.
- [x] Explain runtime discovery state separately from durable workspace state and reconcile the launch/agent contracts and owner decision records.
- [x] Verify affected documentation links, whitespace and agreement with the requested configuration behavior.

Documentation evidence (2026-11-05): checked seven affected documents and 50 local links/anchors; absolute-root schema examples, `profile_home`, omitted-executable example and contract consistency passed. `git diff --check` and affected-file whitespace checks passed. Prettier was unavailable via `npx --no-install --offline prettier --version` (not installed/cached); formatting and the scaffold gate remain unchecked.

### Owner agent and prompt clarification

This documentation update prepares scaffolding for the requested Apple Foundation Models agent and external prompt assets. Runtime implementation remains in the planned agent milestone.

- [x] Add `apple-foundation-models` alongside the three CLI types, with native availability, configuration, lifecycle and acceptance requirements.
- [x] Require prompts under `assets/prompts/<agent-type>/`, inspect the existing files, and document loading/validation without hard-coded prompt fallbacks.
- [x] Reconcile the affected architecture, product, owner records, index and delivery plans; verify local links and whitespace.

Prettier formatting remains unverified because the tool is unavailable. This is a validation limitation outside the P1-M2 docs-only completion gate, not evidence that the scaffold or native agent is implemented.

Documentation evidence (2026-11-05): inspected all nine supplied Apple/Codex templates and preserved their contents. Checks passed across 15 affected documents and 179 local links/anchors, trailing whitespace/newlines, prompt front-matter keys, provider/directory agreement, Apple default metadata and allowed placeholders. Prompt/schema compatibility remains pending. The three Rust gate commands could not run because `src-tauri/Cargo.toml` is absent; Prettier, ESLint and Vitest were unavailable in installation-free offline checks. No tool was installed, and the full gate and E11 runtime acceptance remain unchecked.

### Architecture review

The owner requested an architecture review, fixes and an interview for unresolved behavior. This is P1-M2 architecture work under the documentation-only exception; scaffold implementation and its code gate remain separate. Findings and acceptance cases are recorded in the [review record](../../../phase-1/milestone-02-architecture-review.md).

- [x] Audit maintained contracts against D01–D17 and the later owner clarifications, including logging, shortcuts, settings and prompt assets.
- [x] Specify recoverable operation/review outcomes, safe retry/confirmation ordering, bounded resources and consistent settings behavior.
- [x] Interview the owner on unresolved product behavior; record answers or explicit blockers without inventing defaults.
- [x] Reconcile affected contracts, handoff evidence, scaffold plan and documentation index.
- [x] Validate local links/anchors and whitespace; report formatter/code-gate availability separately from design evidence.

Review evidence (2026-11-05): the [review record](../../../phase-1/milestone-02-architecture-review.md) records twelve corrected contract findings and seven resolved owner answers. Checked 46 maintained Markdown files, 317 local links and 85 anchors, prompt metadata, title/status conventions, index coverage and whitespace. `git diff --check` passed. Rust gate commands could not run because `src-tauri/Cargo.toml` does not exist; installation-free offline Prettier/ESLint/Vitest checks returned unavailable/uncached. No dependency was installed. These checks complete P1-M2 architecture documentation, not P1-M3 scaffolding or its full gate.

### Background AI experience design

The owner requested an architecture for long-running AI feedback and continued work. This extends the review with interaction/transport contracts, without enabling inference or implementing UI.

- [x] Compare progress/completion surfaces within the Standard Layout and specify the selected interaction.
- [x] Define truthful phases, counts, liveness/silence, cancellation, disconnect/reconnect and partial outcomes without invented percentages or reasoning output.
- [x] Specify session/date ownership, continued editing, completion attention, bounded event/render behavior and accessibility.
- [x] Map the design to fake-job scaffold checks and real-agent/native acceptance, index the contract and validate links/whitespace.

Design evidence: [background AI experience](../../../architecture/ai-job-experience.md) includes the alternatives, state diagram, DTOs, timing/render targets and scenario matrix; E13 assigns implementation/native evidence. Owner answers I06/I07 select in-app attention and regeneration after a changed baseline. Primary Apple/W3C/React references were checked; no UI, responsiveness or live-agent acceptance has run.

## Validation and acceptance evidence

The [docs-only exception](../../../development-process.md#definition-of-done) applies: no code gate is required or claimed. Local-link/anchor and whitespace review still apply. Interview decisions are not evidence of runtime behavior, provider terms, source interoperability, packaged filesystem access, or service idempotence.

Preparation validation (2026-11-05): 66 local Markdown links/anchors and whitespace checked across the nine preparation documents; `git diff --check` passed. Reviewed the interview record against the answers and kept engineering proposals separate. The code gate was not run under the docs-only exception. That earlier evidence validated preparation only; completion evidence follows.

Completion audit (2026-11-05): [D01–D17 coverage](../../../phase-1/implementation-handoff.md#owner-decision-coverage) and [E1–E10 evidence gates](../../../phase-1/implementation-handoff.md#capability-evidence-register) identify every owner contract and conditional implementation. The following mapping also covers each work-plan section rather than relying on owner coverage alone:

| Work-plan theme / checklist requirement | Acceptance artifact inspected |
|---|---|
| Design approach, Clean Architecture, separate processes/modules and future transports | [Overview](../../../architecture.md): dependency/deployment/flow diagrams, crate/client allocation and extension seams |
| API, discovery/token/origin/admission, revisions, jobs/events/backpressure and Tauri permissions | [API contract](../../../architecture/api-and-processes.md): exact operation/command table, wire bounds, errors, cancellation and resync |
| Asset/file/DB model, authority, locks, save ordering, external edits and recovery | [Persistence](../../../architecture/domain-and-persistence.md): complete schema/table responsibilities and interrupted-write decision procedure |
| Path identity, aliases/renames/collisions, parsing, metadata and timestamp/date rules, guarded source save/Trash | [Sources/dates](../../../architecture/sources-and-dates.md): normalization, conflict choice, explicit capability gates and partial-outcome recovery |
| Frontend ownership, paragraph alignment, agents/trust and child lifecycle | [Editing/agents](../../../architecture/editing-and-agents.md): fixed-baseline recomposition, independent decisions, bounded one-shot protocol and cancellation |
| YAML/secrets, export reconciliation, versions, platform/icons and package recommendations | [Configuration/exports](../../../architecture/configuration-and-exports.md): schemas, immutable export inputs, remote fingerprint confirmation and native integration route |
| Product/UX and AGENTS reconciliation | [Product](../../../product-behavior.md), [UX GUI](../../../ux-gui.md), [terms](../../../ux-terms.md), [containment](../../../ux-information-architecture.md), [AGENTS.md](../../../../AGENTS.md); no canonical TBD remains |
| Handoff, acceptance destinations and later sequencing | [Implementation handoff](../../../phase-1/implementation-handoff.md), [P1-M3 checklist](../../../phase-1/milestone-03-scaffolding-overview.md), [P1-M3 work plan](../../../phase-1/milestone-03-scaffolding-architecture.md), updated phase/roadmap |

Final documentation validation (2026-11-05): 34 first-party Markdown files, 236 local links/heading anchors, whitespace and final newlines checked with zero errors after archiving/relinking; `git diff --check` passed. The documentation check also confirms all docs are indexed, no canonical UX TBD remains, and zero phase/milestone references occur in the six maintained architecture documents, as requested by the owner. No code, dependency/tool installation, user-data mutation, live inference or service write was performed; no runtime/test-gate success is claimed. P1-M3 remains Planned and requires approval of concrete dependencies/tools before setup.
