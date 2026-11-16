# P1-M37: Job registry refactoring overview

Status: Done (2026-11-16)

Every background job (AI Chat, paragraph review, Draft generation, Illustrated creation, iCloud and Notion export, import) is tracked by the one session job coordinator, but the coordinator does not know what a job is. Each feature keeps a side table and the API finds a job's progress or cancel path by trying each feature in turn. This milestone gives every job a kind and an origin inside the server, replaces the per-feature lookup chains with one dispatch on kind, and splits four files near the size limit. It is a pure refactoring: the HTTP/bridge contract, UI and behavior do not change. Additions (the job kind in the UI contract and anything visible) are [P1-M39](milestone-39-job-additions-overview.md); queues are [P1-M38](milestone-38-job-lanes-overview.md). Order: M37, M39, M38.

## Entry conditions

P1-M33–P1-M36 are Done. No feature work touches `diary-api/src/jobs.rs`, the coordinator or the four files split below.

## Owner decisions (2026-11-16)

- **O1 - Purpose:** illustration, AI generation, AI review, AI Chat, iCloud and Notion export jobs will later be shown to the user as information. M37 refactors job management so that is a small step.
- **O2 - Split:** M37 is refactoring only. Every addition goes to [P1-M39](milestone-39-job-additions-overview.md), which runs before [P1-M38](milestone-38-job-lanes-overview.md) queues.
- **O3 - Approach for M38:** lanes live in the shared coordinator (approach 1 in the [M38 work plan](milestone-38-job-lanes-architecture.md#approaches)). M37 builds the registry that approach needs; its own design choice is in the [work plan](milestone-37-job-registry-architecture.md#approaches).
- **O4 - Order:** M37, M39 and M38 run before P1-M32 closes, as M33–M36 did; release artifacts are refreshed afterwards.
- **O5 - File splits:** split all four files near the 900-line M28 threshold: `src/application/navigation/resources.ts` (861), `src-tauri/src/main.rs` (858), `src-tauri/src/bridge.rs` (803) and `diary-application/src/generation/tests.rs` (941).
- **O6 - Export kinds:** iCloud and Notion exports are two kinds, `icloud_export` and `notion_export`.

## Current state

| Concern                        | Today                                                                                                                                                               |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Job record                     | `Job { id, sequence, state }` in `diary-application/src/lib.rs`; no kind or origin                                                                                  |
| Agent slot                     | Two entry points, `admit` and `admit_agent`; callers choose the one-AI-run rule by picking the function                                                             |
| Progress lookup                | `jobs.rs::job_value` tries generation, then chat, then illustrated (`or_else` chain)                                                                                |
| Cancel                         | `jobs.rs::cancel_job` is a ~100-line if-chain: export, illustrated, generation, chat, then generic                                                                  |
| Side tables in `ServerState`   | `export_jobs` (publish-dedup mark, also used as "is this an export job"), `import_errors` (failure code by job), `illustrated_jobs`, `generations`, `conversations` |
| Paragraph review               | A chat job (`Purpose::ParagraphReview`); the client presents it through its own controller                                                                          |
| Draft candidate review (hunks) | `reviews.rs`, synchronous `spawn_blocking`; not a background job and out of scope                                                                                   |
| Native harness job             | `diary-api/src/smoke.rs` admits a synthetic job on session open in `scaffold-smoke` builds only; `scripts/gate.sh` does not build that feature                      |

## Scope

In scope:

- `JobKind` (`chat`, `paragraph_review`, `generation`, `illustrated`, `icloud_export`, `notion_export`, `import`, plus `harness` for the `scaffold-smoke` job) and `JobOrigin` (date, chat, export operation, or none) on the coordinator's `Job`, set at admission. Server-internal only.
- One `admit` entry point taking a public `Admission` value; whether a job takes the single AI slot comes from its kind. `admit_agent` is removed.
- `job_value` and `cancel_job` dispatch with one `match` on kind. `is_export_job` and the `import_errors` table go away (failure code moves onto the job; `get_job` output is unchanged).
- The four file splits (O5); no file in a split over 600 lines.

Out of scope: any change to HTTP responses, SSE events, bridge types or client code other than the `resources.ts` move; queues (M38); additions (M39); Draft candidate hunk review; dependency changes.

## Completion checklist

### Plan review

- [x] R01 - Owner decisions O1–O6 recorded (2026-11-16).

### Stage A - baseline

- [x] A01 - Record Rust (`cargo test -- --list`) and Vitest test counts and names before any change. Baseline before any change: 876 Rust tests (`cargo test --workspace -- --list`), 484 Vitest tests (`npx vitest list`).
- [x] A02 - List every `admit`/`admit_agent` caller (about 57, ~40 in tests) and the call sites of `is_export_job` and `import_errors`. 57 coordinator admissions in 17 files (production: chat workflow, generation, Illustrated, imports, exports, smoke); `is_export_job` in `jobs.rs`, `import_errors` in `imports.rs`/`jobs.rs`/`lib.rs`. Implementation added `Admission::new` so tests changed mechanically.
- [x] A03 - Characterization tests, passing on the unchanged code: cancel of a running and a terminal job of each kind (state, response shape, published event, `JobFinished` log) and `job_value` progress per kind, following the [cancel table](milestone-37-job-registry-architecture.md#job-registry-design). Coverage, stated exactly: terminal-job cancel is pinned for chat, generation and Illustrated by new tests and for export by the existing export test; running-job cancel for every kind is pinned by existing tests. Terminal cancel for import and harness has no dedicated test (they run the unchanged generic path). New tests: `chat_cancel_returns_bare_job_and_terminal_cancel_is_unchanged`, `terminal_generation_cancel_returns_current_job_with_progress`, `terminal_cancel_returns_current_without_signal`; they passed on the unchanged code (commit 52cdf5d). Running-job cancel for every kind was already covered by existing chat, paragraph, generation, Illustrated, export, import and fake-job tests.

### Stage B - job registry (server)

- [x] B01 - `Job` has `kind` and `origin`; every admission site sets them per the work plan's table (paragraph review from the `paragraph-` conversation rule); `admit_agent` is gone and the AI-slot rule follows `JobKind::uses_agent_slot`. Coordinator unit tests cover each kind's busy rule unchanged. `agent_kinds_share_one_run_and_other_kinds_record_kind_and_origin`, `chat_and_paragraph_turns_have_their_own_kinds`, export kind/origin assertion in the export cancel test.
- [x] B02 - `job_value` and `cancel_job` use one `match` on kind; no `or_else` chain or `is_export_job` remains (grep evidence); A03 tests pass unedited; `exports::cancel` still runs without the coordinator lock held. Grep shows no `or_else` chain; `is_export_job` remains only as the documented fallback for exports the coordinator never admitted ([implementation record](milestone-37-job-registry-architecture.md#implementation-record-2026-11-16)). A03 tests pass unedited; the coordinator guard is released before `exports::cancel` (the handler would not compile otherwise, since the guard is not `Send`).
- [x] B03 - Import failure code is stored on the job; `ServerState::import_errors` is removed; `get_job` responses are byte-identical in existing tests. Export's dedup mark is private to `exports.rs`. `Coordinator::fail` stores the code; `import_errors` removed; import tests pass unedited. `fail_keeps_its_code_on_the_job` covers idempotence.

### Stage C - file splits

- [x] C01 - `src-tauri/src/main.rs`: Tauri commands move into modules by area; `main.rs` keeps setup and the command list. `main.rs` 858 → 196; six `commands/*` modules (69–154 lines); window commands stay in `main.rs`. Every moved line matches the old file in order apart from added `pub`.
- [x] C02 - `src-tauri/src/bridge.rs`: wire types move to `bridge/types.rs` and the SSE stream group to `bridge/stream.rs`; `bridge.rs` keeps `Bridge`, `request`, connection and session methods. `bridge.rs` 803 → 464; `types.rs` 158; `stream.rs` 191.
- [x] C03 - `src/application/navigation/resources.ts`: source open/refresh/import reconcile and Illustrated open move into `source-loading.ts` and `illustrated-loading.ts` following the existing `SourceMutations` pattern; `ResourceCoordinator` keeps its public methods as delegates; `frontend-boundaries` passes. Implemented as one `resource-loading.ts` (388) holding Authentic, Illustrated and source loads; `resources.ts` 861 → 555; `frontend-boundaries` and eslint pass.
- [x] C04 - `diary-application/src/generation/tests.rs`: split by feature (run/cancel, capture, workflow) with shared fixtures; leaf test names unchanged. `generation/tests/{mod,run,capture,workflow}.rs` (315/228/133/292); 18 tests changed module path only.

### Stage D - proof and docs

- [x] D01 - Test counts and leaf names match A01 plus the A03 additions (module paths may change for moved tests only). After: 882 Rust (876 + 6 new: three A03, three B01; no leaf name removed), Vitest list identical (484).
- [x] D02 - `python3 scripts/smoke/agent.py`, `export.py`, `review.py`, `illustrated.py` and `imports.py` pass. `agent.py`, `export.py`, `review.py`, `illustrated.py` and `imports.py` passed (synthetic mode).
- [x] D03 - `scripts/gate.sh` passes all seven commands. `scripts/gate.sh` passed G01–G07. An earlier run failed G06 on `tests/architecture.test.js` (it read command names from `main.rs`; updated to read `commands/`) and G03 once on `qwen_late_output_after_cancel_is_dropped`, a fake-adapter timing test in untouched code that passed three isolated reruns and the next full gate.
- [x] D04 - Docs citing moved paths are updated; phase plan, roadmap and changelog are updated. `editing-and-agents.md` and `api-and-processes.md` updated; phase plan, roadmap, changelog and docs index updated.
- [x] D05 - `cargo clippy` and `cargo test` for `diary-api` and the app crate with `--features scaffold-smoke` pass (the gate does not build this feature). `cargo clippy -D warnings` passes for `diary-api` and `diary-client` with `--features scaffold-smoke` (after collapsing one pre-existing `if`); tests pass: diary-api 103, diary-client 97.
