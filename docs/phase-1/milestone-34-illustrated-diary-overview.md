# P1-M34: Illustrated diary format and creation overview

Status: Planned

Add a second durable diary kind: an AI-rewritten version of a day's Authentic diary with generated photos, stored as one Markdown file plus one photo directory. This milestone delivers the format, the store and the server-side creation job; the GUI is [P1-M35](milestone-35-illustrated-diary-gui-overview.md). Requires [P1-M33](../archive/milestones/p1-m33/milestone-33-image-engines-overview.md).

"Illustrated diary" (kind `illustrated`) is a working name; see Q1.

## Entry conditions

P1-M33 is Done. Owner decisions O1-O6 in the [P1-M33 overview](../archive/milestones/p1-m33/milestone-33-image-engines-overview.md#owner-decisions-2026-11-12) apply.

This milestone changes a decided invariant ("Only Authentic is managed durable content", [domain and persistence](../architecture/domain-and-persistence.md#values-and-ownership)) on the owner's request of 2026-11-12. Record it in [decisions](../decisions.md) before implementing.

## Scope

- Stage A: domain values and file schema for the `illustrated` kind; store port and filesystem adapter with atomic publish of the Markdown file and photo directory.
- Stage B: agent mode that turns one saved Authentic entry into rewritten text plus photo prompts, with strict output validation and a prompt asset.
- Stage C: creation job: precondition (saved Authentic exists), agent run, renders through the chosen engine, staged publish, replace of an existing version, cancel, restart cleanup.
- Stage D: API routes and bridge commands, smoke script, docs, full gate.

Out of scope: any GUI (P1-M35); editing the version in the editor; more than one version per day; export of the version to iCloud or Notion; Trash for the version; re-rendering a single photo; user-typed photo prompts; keeping the version in sync when the Authentic entry later changes (it only records which revision it came from).

## Open items

- **Q1 - Name:** "Illustrated diary" / `illustrated` for the kind, path segment and UI term. Owner to confirm or rename before Stage A (renaming later means a file migration).
- **Q2 - Photo count:** proposal: the agent chooses 1 to 4 photos per entry, capped by config (`illustrated.max_photos`, default 3).
- **Q3 - Partial failure:** proposal: if any render fails or is cancelled, nothing is published and the previous version (if any) stays. Alternative: publish with the photos that succeeded.
- **Q4 - Replace:** proposal: creating again replaces the day's version only after an explicit confirmation flag from the client; the old files are deleted on publish (not moved to Trash). Deleting user data needs the owner's yes.
- **Q5 - Unsaved edits:** proposal: creation reads the last saved Authentic revision only and refuses while that date has a pending save.
- **Q6 - Live proof:** one end-to-end creation with a real agent profile and the Krea 2 engine, env-gated, agent-run; Qwen stays exempt.

## Completion checklist

Check only when evidence exists.

### Plan review

- [ ] R01 - Owner answered Q1-Q6; recorded here, in decisions and the changelog.
- [ ] R02 - Work plan re-read against current code (Authentic store and pending staging, generation service, `run::execute`, `Coordinator::admit_agent`, prompt assets, catalog); corrections recorded in the [work plan](milestone-34-illustrated-diary-architecture.md).
- [x] R03 - Research Atomic Editor for Obsidian-style inline Markdown and photos: inspected pinned upstream source/license, compared GUI contracts, documented integration gaps and a recommendation without adopting a dependency. Evidence: [research report](../research/atomic-editor.md); local link targets and formatting checked. This completes research only; no GUI runtime acceptance is claimed.

### Stage A - format and store

- [ ] C01 - Schema 1 header parses and serializes round-trip; photo references are restricted to `<date>/NN.png` relative links; any other image link, path traversal or missing photo is a typed validation error. Evidence: `diary-domain` tests.
- [ ] C02 - Store publishes staged Markdown plus photo directory so a reader sees either the old complete version or the new complete version; files are 0600, directories 0700, symlinks refused. Evidence: filesystem tests including injected crash between steps.
- [ ] C03 - Startup cleanup removes abandoned staging for unfinished creations and never touches Authentic files. Evidence: restart test.
- [ ] C04 - The Authentic catalog, autosave and exports ignore the new kind. Evidence: existing Authentic/export tests pass unchanged plus one test with both kinds present.

### Stage B - agent mode

- [ ] C05 - A new packet mode sends exactly one Authentic body and returns rewritten Markdown with ordered photo slots and one prompt per slot; output outside the contract (too many photos, empty prompt, over-length text, stray image links) is rejected. Evidence: application tests with the fake runner.
- [ ] C06 - Prompt text lives in a prompt asset following the prompt-asset conventions; writing locale is respected. Evidence: asset tests.

### Stage C - creation job

- [ ] C07 - Creation is refused with a typed reason when the date has no saved Authentic, the engine is unavailable, no agent profile is usable, or a version exists and replace was not confirmed. Evidence: application tests.
- [ ] C08 - A successful job writes the Markdown and N photos, records source Authentic ID, revision and hash, engine ID, agent profile, seeds and prompts, and leaves the Authentic file byte-identical. Evidence: integration test with fake agent and fake engine.
- [ ] C09 - Cancel during the agent run or any render stops the child process, publishes nothing and keeps the previous version. Evidence: integration tests.
- [ ] C10 - Progress is reported per step (text, photo k of N) through the existing job/event channel with no diary text in events. Evidence: event tests.

### Stage D - API, proof and docs

- [ ] C11 - Routes: list, read (Markdown and metadata), photo bytes, create, delete; session-authorized like the Authentic routes; photo route serves only `NN.png` of an existing version. Evidence: API tests.
- [ ] C12 - `scripts/smoke/` leg creates, reads and replaces a version against the synthetic server. Evidence: smoke output.
- [ ] C13 - Live end-to-end creation. Evidence: env-gated test output, or "exempt" with reason.
- [ ] C14 - Architecture docs (domain and persistence, editing and agents, API), decisions, changelog, docs index updated.
- [ ] C15 - `scripts/gate.sh` passes.
