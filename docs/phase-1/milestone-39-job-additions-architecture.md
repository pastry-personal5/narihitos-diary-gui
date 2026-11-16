# P1-M39: Job contract additions work plan

Status: Planned

Approach and sequencing for the [P1-M39 overview](milestone-39-job-additions-overview.md). Additive contract change; existing behavior, guards and messages stay the same.

## Server (`diary-api/src/jobs.rs`, `exports.rs`)

- `JobDto` gains `kind` (snake_case of `JobKind`) and `origin`:

  ```json
  {
    "id": "…",
    "sequence": 3,
    "state": "running",
    "kind": "notion_export",
    "origin": { "operation_id": "…" },
    "progress": {
      "stage": "…",
      "photos_uploaded": 2,
      "target": "notion",
      "date": "2026-11-16"
    }
  }
  ```

  `origin` is `{ "date": "YYYY-MM-DD" }`, `{ "chat_id": … }`, `{ "operation_id": … }` or absent. The same DTO is used by `list_jobs`, `get_job`, cancel responses and the SSE `job_changed` payload, so every surface stays consistent.

- `job_value` gains the export arm: a new `exports::progress` returns the bounded fields `exports.rs` already serializes for operation results. Import jobs keep no progress object.

## Bridge (`src-tauri/src/bridge/`, `events.rs`)

Add `kind` and `origin` to the bridge `Job` type and event payload validation: closed kind list, existing ID/date shape checks, `origin` with at most one key. Unknown kinds fail validation (no silent pass-through), matching how unknown states are handled today.

## Client

- `src/application/events.ts`: `SnapshotJob` and the `job_changed` payload gain `kind: JobKind` and `origin?`. `src/adapters/validation.ts` validates both.
- `illustrated/controller.ts`, `generation/controller.ts`: replace `progress?.kind === …` with `job.kind`. Do not narrow the Illustrated "any queued or running job" guard (`controller.ts:294`); M38 changes it.
- `ai-activities/progress-messages.ts`: find post-admission paragraph review in the activity store by `kind === "paragraph_review"`; pre-admission preparation keeps its controller source. Export kinds are skipped (`ExportController` reports them).

## Sequencing

1. Server DTO and export progress (A01); `cargo test -p diary-api`.
2. Bridge (A02); app crate tests.
3. Client validation, guards, reporter (B01–B03); `npx vitest run`.
4. Q1 (B04), smokes, full gate, docs.

## Risks

- The bridge rejects a kind the server sends if they drift. Mitigation: one list per side with a test that the server's kind strings equal the bridge's and client's lists.
- Event size grows slightly. Mitigation: origins are bounded IDs/dates; the A01 size test covers the largest DTO.
