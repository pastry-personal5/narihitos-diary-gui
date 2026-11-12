# P1-M34: Illustrated diary format and creation overview

Status: Done

Add a second durable diary kind: an AI-rewritten version of a day's Authentic diary with generated photos, stored as one Markdown file plus one photo directory. This milestone delivers the format, the store and the server-side creation job; the GUI is [P1-M35](../../../phase-1/milestone-35-illustrated-diary-gui-overview.md). Requires [P1-M33](../p1-m33/milestone-33-image-engines-overview.md).

The owner confirmed "Illustrated diary" (kind `illustrated`) on 2026-11-13.

## Entry conditions

P1-M33 is Done. Owner decisions O1-O6 in the [P1-M33 overview](../p1-m33/milestone-33-image-engines-overview.md#owner-decisions-2026-11-12) apply.

This milestone changes a decided invariant ("Only Authentic is managed durable content", [domain and persistence](../../../architecture/domain-and-persistence.md#values-and-ownership)) on the owner's request of 2026-11-12. It is recorded in [decisions](../../../decisions.md).

## Scope

- Stage A: domain values and file schema for the `illustrated` kind; store port and filesystem adapter with atomic publish of the Markdown file and photo directory.
- Stage B: agent mode that turns one saved Authentic entry into rewritten text plus photo prompts, with strict output validation and a prompt asset.
- Stage C: creation job: precondition (saved Authentic exists), agent run, renders through the chosen engine, staged publish, replace of an existing version, cancel, restart cleanup.
- Stage D: API routes and bridge commands, smoke script, docs, full gate.

Out of scope: any GUI (P1-M35); editing the version in the editor; more than one version per day; export of the version to iCloud or Notion; Trash for the version; re-rendering a single photo; user-typed photo prompts; keeping the version in sync when the Authentic entry later changes (it only records which revision it came from).

## Interview decisions (2026-11-13)

- **Q1 - Name resolved:** Illustrated diary / `illustrated`, including the stored kind and path segment.
- **Q2 - Photo count resolved:** the AI Agent analyzes and counts theme groups in the saved Authentic text. Generate one photo per group, combining related groups when needed to meet `illustrated.max_photos` (configurable cap, default 4).
- **Q3 - Failure resolved:** require all photos to succeed. Any render failure or cancellation before publication publishes nothing and preserves the previous version.

- **Q4 - Replacement/deletion resolved:** creation silently replaces an existing Illustrated diary after successful publication, permanently removing the old Markdown/photos. Deletion requires explicit confirmation. No Trash is introduced.
- **Q5 - Saved source resolved:** finish pending saves before capturing the saved Authentic revision; failed saves block creation. M35 settles client saves before submission; the server refuses capture while a save is pending.
- **Q6 - Live proof resolved:** owner authorizes an isolated test-workspace end-to-end run with a real AI Agent and Krea 2. Use **xpp8** for AI Agent testing; resolve its installed profile/configuration before execution. This is authorization, not evidence of a completed run.

## Open items

Q1-Q6 are resolved. All interview choices and internal contracts are resolved. Acceptance evidence is collected in the [execution record](milestone-34-illustrated-diary-evidence.md).

## Completion checklist

Check only when evidence exists.

### Implementation review follow-up (2026-11-13)

- [x] V01 - Validate the complete assembled Markdown and every final prompt before rendering; regressions cover malformed Markdown and aggregate body limits without engine calls.
- [x] V02 - Lease loss retains active date reservations until workers finish; cancellation advances the job sequence and publishes progress. Direct API regressions cover both contracts and session-close cancellation. Evidence: [review follow-up](milestone-34-illustrated-diary-evidence.md#implementation-review-follow-up-2026-11-13).
- [x] V03 - Read-only store operations never create missing photo directories; regression tests verify missing-photo failures preserve the filesystem.
- [x] V04 - Run the full gate and isolated Illustrated server smoke; record results in the evidence record. Evidence: all seven lanes PASS in `run.LYEbpX` and synthetic smoke PASS in the [review follow-up](milestone-34-illustrated-diary-evidence.md#implementation-review-follow-up-2026-11-13).

### Plan review

- [x] R01 - Owner answered Q1-Q6. Evidence: interview decisions above, owner-decision log and changelog (2026-11-13).
- [x] R02 - Work plan re-read against current code (Authentic store and pending staging, generation service, `run::execute`, `Coordinator::admit_agent`, prompt assets, catalog); corrections recorded in the [work plan](milestone-34-illustrated-diary-architecture.md).
- [x] R03 - Research Atomic Editor for Obsidian-style inline Markdown and photos: inspected pinned upstream source/license, compared GUI contracts, documented integration gaps and a recommendation without adopting a dependency. Evidence: [research report](../../../research/atomic-editor.md); local link targets and formatting checked. This completes research only; no GUI runtime acceptance is claimed.

- [x] R04 - Resolve publication/read consistency, whole-job ownership, source/mutation races and delete authorization identified in the [plan review](milestone-34-illustrated-diary-architecture.md#review-gaps-to-resolve-before-implementation); record testable contracts before implementation.
- [x] C16 - Selected engine config supplies trigger and prompt directives before prompt derivation; final render prompts start with that trigger and use compatible directive components. Exact render-request tests cover two engines, optional defaults, bounds and reload stability; config parsing/reload tests cover the new fields. Semantic adherence is reviewed through prompt-asset fixtures.

### Stage A - format and store

- [x] C01 - Schema 1 header parses and serializes round-trip; photo references are restricted to `<date>/NN.png` relative links; any other image link, path traversal or missing photo is a typed validation error. Evidence: `diary-domain` tests.
- [x] C02 - Store publishes staged Markdown plus photo directory so a reader sees either the old complete version or the new complete version; files are 0600, directories 0700, symlinks refused. Evidence: filesystem tests replaying interrupted journals at pair rename boundaries; external viewers can observe intermediate files.
- [x] C03 - Startup cleanup removes abandoned staging for unfinished creations and never touches Authentic files. Evidence: restart test.
- [x] C04 - The Authentic catalog, autosave and exports ignore the new kind. Evidence: existing Authentic/export tests pass unchanged plus one test with both kinds present.

### Stage B - agent mode

- [x] C05 - A new packet mode sends exactly one Authentic body and returns rewritten Markdown with counted theme groups, related groups combined to the configured cap, ordered photo slots and one prompt per resulting group; output outside the contract (too many photos, empty prompt, over-length text, stray image links) is rejected. Evidence: `illustrated_creation_freezes_source_and_engine_settings_and_preserves_on_failure`, strict output tests and the synthetic server smoke.
- [x] C06 - Prompt text lives in a prompt asset following the prompt-asset conventions; writing locale is respected. Evidence: asset tests.

### Stage C - creation job

- [x] C07 - Creation is refused with a typed reason when the date has no saved Authentic, the engine is unavailable, no agent profile is usable, or the source has a server-owned pending/unfinished save reservation. M35 must finish client saves and block submission after a failed client save. Existing versions are replaced silently only on successful publication. Evidence: application tests.
- [x] C08 - A successful job writes the Markdown and N photos, records source Authentic ID, revision and hash, engine ID, agent profile, seeds and prompts, and leaves the Authentic file byte-identical. Evidence: integration test with fake agent and fake engine.
- [x] C09 - Cancel during the agent run or any render stops the child process, publishes nothing and keeps the previous version. Evidence: integration tests.
- [x] C10 - Progress is reported per step (text, photo k of N) through the existing job/event channel with no diary text in events. Evidence: event tests.

### Stage D - API, proof and docs

- [x] C11 - Routes: list, read (Markdown and metadata), photo bytes, create, delete; session-authorized like the Authentic routes; photo route serves only `NN.png` of an existing version. Evidence: API tests.
- [x] C12 - `scripts/smoke/` leg creates, reads and replaces a version against the synthetic server. Evidence: final synthetic smoke PASS; see the execution record.
- [x] C13 - Live end-to-end creation. Evidence: isolated xpp8 + Krea 2 live run PASS (129.08 seconds), retained artifact and hash in the execution record.
- [x] C14 - Architecture docs (domain and persistence, editing and agents, API), decisions, changelog, docs index updated.
- [x] C15 - `scripts/gate.sh` passes. Evidence: G01–G07 PASS in `run.serctK`.

## Completion record

Implemented and verified 2026-11-13. All checklist items have evidence in the [execution record](milestone-34-illustrated-diary-evidence.md): final seven-lane gate `run.serctK`, isolated synthetic smoke including agent/render cancellation, and the owner-authorized xpp8 + Krea 2 live creation. No dependency or build-tool change. GUI/save-settling controls remain M35; ordinary artifact qualification remains M32.
