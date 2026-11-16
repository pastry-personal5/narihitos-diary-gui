# P1-M39: Job contract additions overview

Status: Planned

After the [P1-M37](milestone-37-job-registry-overview.md) refactoring the server knows each job's kind and origin, but the client still guesses a job's kind from feature-specific progress fields, and export jobs carry no progress in the job list. This milestone adds those facts to the job contract and lets the client use them. It runs after M37 and before [P1-M38 job lanes](milestone-38-job-lanes-overview.md), which builds on the same contract.

## Entry conditions

P1-M37 is Done: `Job` has `kind` (`chat`, `paragraph_review`, `generation`, `illustrated`, `icloud_export`, `notion_export`, `import`, and `harness` for the `scaffold-smoke` native harness job) and `origin` inside the server. The closed kind lists in the bridge and client include `harness` so harness builds still validate.

## Owner decisions (2026-11-16)

- **O1 - Split:** M37 is refactoring only; every addition goes to M39, which runs before M38.
- **O2 - Export kinds:** `icloud_export` and `notion_export` are separate kinds.
- **O3 - Order:** M37, M39 and M38 run before P1-M32 closes.

## Scope

In scope:

- Job DTO (`list_jobs`, `get_job`, SSE `job_changed` payload and snapshots) gains `kind` and `origin`. Export jobs gain a `progress` object from fields `exports.rs` already builds (stage, photos uploaded, target, date).
- Tauri bridge types and validation, `src/application/events.ts` and `src/adapters/validation.ts` accept the new fields with closed kind lists and bounds; unknown kinds and malformed origins are rejected.
- Client uses kind instead of guessing: Illustrated and generation guards read `job.kind`; the progress reporter finds post-admission paragraph review by kind instead of its special case.
- `api-and-processes.md` and `ai-job-experience.md` document the fields.

Out of scope: queues (M38); folding `ExportController`'s client-side step timer (its steps start before the job exists; M38 changes that); behavior changes to guards or messages.

## Open items

- **Q1 - Show the information now?** The fields make an informative job list possible. Recommended default: no new UI in M39; M38 already extends the AI Activities dialog with lanes, so the list arrives there once, with kind labels. Alternative: M39 adds a read-only kind/date/state column to the existing dialog now and M38 adds lanes to it.

## Completion checklist

### Plan review

- [ ] R01 - Owner confirms Q1 or accepts the default.

### Stage A - contract

- [ ] A01 - `JobDto` carries `kind` and `origin` (`{ "date" }`, `{ "chat_id" }`, `{ "operation_id" }` or absent); export jobs carry `progress`. API tests cover each kind; an event-size test bounds the largest DTO under `MAX_EVENT_BYTES`.
- [ ] A02 - Tauri bridge types and validation accept the fields; bridge tests cover each kind and reject unknown ones.

### Stage B - client

- [ ] B01 - `SnapshotJob` and `validation.ts` accept `kind`/`origin` with bounds tests.
- [ ] B02 - Illustrated and generation guards read `job.kind`; no `progress?.kind` checks remain (grep evidence); guard behavior unchanged.
- [ ] B03 - The progress reporter finds post-admission paragraph review by kind; message text and cadence unchanged (existing tests pass unedited).
- [ ] B04 - Q1 decision implemented (or recorded as no UI change).

### Stage C - proof and docs

- [ ] C01 - Tests that assert exact job DTO shapes are updated and listed with the reason; no other test changes.
- [ ] C02 - `python3 scripts/smoke/agent.py`, `export.py`, `review.py`, `illustrated.py` pass.
- [ ] C03 - `scripts/gate.sh` passes all seven commands.
- [ ] C04 - Contract docs, phase plan, roadmap and changelog updated.
