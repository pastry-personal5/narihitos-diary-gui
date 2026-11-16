# P1-M37: Job registry refactoring work plan

Status: Planned

Approach, module boundaries and sequencing for the [P1-M37 overview](milestone-37-job-registry-overview.md). Pure refactoring: every step compiles and passes its crate tests before the next starts, and no HTTP, SSE, bridge or UI output changes.

## Rules

- Keep the layer rules in [architecture](../architecture.md): `JobKind`/`JobOrigin` live in `diary-application`; `diary-api` uses them for dispatch only. They are not serialized in M37 ([M39](milestone-39-job-additions-overview.md) adds them to the contract).
- Feature payload stores stay where they are (`generations`, `conversations`, `illustrated_jobs`). Only lookup and dispatch move. Moved code keeps its validation and error handling.
- Characterize before changing: the per-kind cancel and progress behavior is pinned by tests that pass on the old code first (stage A03).
- Move first, then clean (as in [M28](milestone-28-simple-refactoring-architecture.md#rules)): a move commit changes no logic. Tests move with the code they cover; leaf test names do not change.
- No new dependency; `Cargo.toml` and `package.json` unchanged.

## Approaches

| Option                                                   | Trade-off                                                                                                                                                 |
| -------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A. Kind on `Job`, one `match` per operation (chosen)** | Smallest change; the compiler forces every operation to handle a new kind, which is what M38 and later kinds need. Feature code stays in feature modules. |
| B. A `JobFeature` trait object registered per kind       | Open for extension, but adds an abstraction, dynamic dispatch and registration wiring for eight fixed, in-tree kinds.                                     |
| C. Add `kind` only to the DTO, keep the lookup chains    | Least server work, but M38 lanes need kind inside the coordinator, and the chains keep growing with every new kind. Also an addition, so not M37.         |

A is the agent's call under the refactoring scope; B is rejected as speculative, C as not preparing M38.

## Job registry design

**Coordinator (`diary-application/src/lib.rs`).**

```rust
pub enum JobKind { Chat, ParagraphReview, Generation, Illustrated, IcloudExport, NotionExport, Import, Harness }
pub enum JobOrigin { None, Date(DiaryDate), Chat(String), Operation(String) }
impl JobKind { pub fn uses_agent_slot(self) -> bool { /* Chat | ParagraphReview | Generation | Illustrated */ } }
pub struct Job { pub id, pub sequence, pub state, pub kind, pub origin, pub error_code: Option<&'static str> }
pub struct Admission<'a> { pub request_id, pub digest, pub job_id, pub reserved_bytes, pub kind: JobKind, pub origin: JobOrigin }
pub fn admit(&mut self, session: &str, now: u64, admission: Admission<'_>) -> Result<Job, Error>
```

`Admission` (today private, built inside `admit`/`admit_agent`) becomes the public argument of the single `admit`, so M38 (lane) and later additions add a field instead of touching every call again. `admit_inner(.., agent: bool)` derives `agent` from `kind.uses_agent_slot()`; `admit_agent` is deleted. Request-ID idempotency compares the digest only, as today. A failure code is set with the terminal transition (an argument on `transition` or a `transition_failed` helper; pick whichever needs fewer call-site edits).

**Kinds and origins by admission site.**

| Admission site                                                     | Kind                                                                                            | Origin                    |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------- | ------------------------- |
| `chat/workflow.rs::PreparedTurn::admit` (via `diary-api/chats.rs`) | `Chat` or `ParagraphReview` from the conversation ID's `Purpose` (`Conversations::for_id` rule) | `Chat(conversation_id)`   |
| `generation.rs` (via `generation/workflow.rs`)                     | `Generation`                                                                                    | `Date(origin date)`       |
| `diary-api/illustrated.rs::create`                                 | `Illustrated`                                                                                   | `Date(date)`              |
| `diary-api/exports.rs::sync`                                       | `IcloudExport` or `NotionExport` from `ExportResult::target`                                    | `Operation(operation_id)` |
| `diary-api/imports.rs`                                             | `Import`                                                                                        | `Date(import date)`       |
| `diary-api/smoke.rs::admit` (feature `scaffold-smoke` only)        | `Harness`: synthetic native-harness job admitted on session open; never in production builds    | `None`                    |

About 57 coordinator `admit`/`admit_agent` calls exist; roughly 40 are in tests (`diary-application/src/tests.rs`, `run/tests.rs`, `chat/workflow.rs`, `generation/tests.rs`, `diary-api` tests). Each test module gets one small builder (`admission(request, job)` with a default kind) so the edit is mechanical; assertions stay as they are.

**API (`diary-api/src/jobs.rs`).**

- `job_value`: `match job.kind` selects `generations::progress`, `chats::progress` (chat and paragraph review) or `illustrated::progress`; export, import and harness kinds return no progress object, exactly as today.
- `cancel_job`: validate lease/ownership once, then `match job.kind`. Each arm is the existing branch body moved into its feature module unchanged, including its `touch`/`publish`, response shape and the `JobFinished` log record. The table below is what each arm must reproduce, including today's fall-through to the generic path:

| Kind                           | Non-terminal job                                                                               | Terminal job                                                   | Response    |
| ------------------------------ | ---------------------------------------------------------------------------------------------- | -------------------------------------------------------------- | ----------- |
| `IcloudExport`, `NotionExport` | `exports::cancel` (operation-level cancel)                                                     | `exports::cancel`                                              | as today    |
| `Illustrated`                  | request cancel on the run token; `touch` + publish if changed                                  | no signal; return current                                      | `job_value` |
| `Generation`                   | `cancel()`, phase `Stopping`, `touch` + publish                                                | return current                                                 | `job_value` |
| `Chat`, `ParagraphReview`      | if a conversation's active turn owns the job: `cancel_job` on conversations, `touch` + publish | falls through to generic `Coordinator::cancel` path (as today) | `JobDto`    |
| `Import`, `Harness`            | generic `Coordinator::cancel`, conversations notified, publish, `JobFinished` log              | generic path                                                   | `JobDto`    |

A chat job whose turn is no longer held by any conversation also takes the generic path today; the chat arm keeps that.

- `is_export_job` is removed; `export_jobs` keeps only the `(session, checkpoint, state)` dedup mark used by `exports::sync`. `import_errors` is removed; `get_job` adds `error_code` from `job.error_code`, producing the same JSON (`serde_json` maps are key-sorted, so insertion order cannot change output).

**Ordering hazard.** Today `cancel_job` checks export membership before taking the coordinator lock, and the chat branch tests membership by scanning conversations. With kind on the job, the coordinator lookup comes first for every kind; `exports::cancel` must therefore be called after releasing the coordinator lock, as it is today. Keep each other arm's lock order (coordinator, then feature mutex) as the existing branch has it.

## File splits

Target: no resulting file over 600 lines; public paths unchanged via re-exports; one move-only commit per file.

| File                                                | Lines | Direction                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| --------------------------------------------------- | ----: | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/main.rs`                             |   858 | ~70 thin `#[tauri::command]` wrappers (lines ≈19–716) move into `commands/` modules by area: `window` (close gate), `session` (connect/status/session/settings/user settings), `ai` (agents, chat, generation, review), `sources` (source, trash, imports, time resolution), `illustrated` (images, illustrated), `authentic` (authentic, operations, jobs, events, metadata, symbols, client log). `main.rs` keeps `main()` (≈717–858) and the `generate_handler!` list. |
| `src-tauri/src/bridge.rs`                           |   803 | Per-route methods already live in `bridge/*` submodules. Move the wire types (`Status` … `Job`, `JobList`, `Metadata`; ≈47–230) to `bridge/types.rs` and the 170-line SSE `subscribe`/`acknowledge`/`detach`/`stop` group (≈463–648) to `bridge/stream.rs`. `bridge.rs` keeps `Bridge`, `request`, connection and session methods (≈450 lines).                                                                                                                           |
| `src/application/navigation/resources.ts`           |   861 | Source mutations are already in `SourceMutations`. Move source open/refresh (`openSource`, `refreshSource`, `reconcileImport`; ≈555–756) into `source-loading.ts` and `openIllustrated` (≈471–540) into `illustrated-loading.ts`, following the `SourceMutations` pattern (a helper the coordinator owns); public methods stay on `ResourceCoordinator` as delegates. Stays inside `src/application/navigation`, so `scripts/frontend-boundaries.mjs` rules hold.         |
| `diary-application/src/generation/tests.rs` (tests) |   941 | `generation/tests/` with `run.rs` (≈129–356: order, partial failure, cancel, busy, lease, caps, budget), `capture.rs` (≈356–650: source fakes, selection, duplicates, packet, shared cap), `workflow.rs` (≈650–941: profile fakes, workflow) and shared fixtures in `mod.rs` (≈1–128).                                                                                                                                                                                    |

## Sequencing

1. Stage A: baseline test lists; inventory of admission callers and side-table uses; characterization tests for cancel and progress per kind (pass on the old code).
2. Stage B: coordinator `Admission`/kind/origin (B01), API dispatch (B02), side tables (B03). `cargo test -p diary-application`, `-p diary-api`, then the app crate after each; `cargo clippy`/`cargo test -p diary-api --features scaffold-smoke` after B01.
3. Stage C: splits, one file per commit, move-only. `cargo test` / `npx vitest run` after each.
4. Stage D: parity, feature build, smokes, full gate, docs.

## Risks

- Moving cancel branches can change per-kind behavior or lock order. Mitigation: the cancel table and A03 characterization tests; review with `git diff --color-moved`.
- The `scaffold-smoke` feature is not built by `scripts/gate.sh`, so breaking `smoke.rs` would pass the gate. Mitigation: D05 builds and tests it explicitly.
- An export job observed before `exports::sync` admits it has no kind. Mitigation: kind is set at that same admission, so no job exists without one; a test asserts every admission path sets kind.
- Wrong chat kind for paragraph review. Mitigation: kind comes from the same `paragraph-` conversation-ID rule `Conversations::for_id` uses; a test admits one of each purpose.
- Large moves hide accidental logic edits. Mitigation: move-only commits, as in M28.
