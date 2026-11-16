# P1-M38: Job lanes and Illustrated photo candidates work plan

Status: Planned

Approach, interfaces and sequencing for the [P1-M38 overview](milestone-38-job-lanes-overview.md). Values marked "(Qn)" follow the overview's open-item defaults until the owner decides.

## Approaches

| Option                                        | Trade-off                                                                                                                                                                                     |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **1. Lanes in the coordinator (owner chose)** | Order, position and cancel-while-queued are pure coordinator state, testable with a fake clock; the UI's job store only gains `queued` and a lane. Needs the M37 kind registry.               |
| 2. A queue inside each feature                | Smaller first diff, but brings back per-feature side tables; every future kind repeats queue logic and the job list needs special cases again.                                                |
| 3. A semaphore per lane in `diary-api`        | Little code, but queue order and position live outside the coordinator, so "3rd in line" cannot be shown reliably, cancelling a waiter is awkward and ordering is not deterministic in tests. |

## Coordinator lanes (`diary-application/src/lib.rs`)

```rust
pub enum Lane { Agent, Image, Export }
impl JobKind { pub fn lane(self) -> Option<Lane> } // Import => None
// Session gains: lanes: BTreeMap<Lane, LaneState { running: Option<String>, queue: VecDeque<String> }>
```

- `admit` replaces the M37 `uses_agent_slot` busy check: if the job's lane has no running job it is admitted `Running`; otherwise it is appended `Queued`. Lane depth over its cap (Q3) is `Error::Limit`. Request-ID lookup still runs first, so a retried request returns the same queued job.
- Any transition to a terminal state (including `cancel` of a queued job) removes the job from its lane and, if it was running, promotes the queue head to `Running`. The method returns the promoted job so the caller can start its worker. A cancelled queued job never starts.
- `queue_position` is computed on read (1-based index in the lane queue); it is not stored, so it cannot go stale.
- Session close and lease expiry cancel every queued job before cancelling running ones; nothing is promoted into a closing session.
- Queued jobs reserve their metadata bytes (4 KiB) against the 64 MiB session budget as today; the 24 non-terminal cap applies to running and non-laned jobs only (Q3).

**Starting promoted jobs (`diary-api`).** Each admission that may queue registers a start closure: `ServerState::starters: Mutex<BTreeMap<job_id, Box<dyn FnOnce() + Send>>>`. On promotion the API removes and runs the closure (it spawns the same blocking worker the route spawns today). On cancel/expiry of a queued job the closure is dropped. Workers capture their inputs at admission (source/context/baseline for generation, the chat turn, the export request, the render slot), so promotion never re-reads UI state. One helper `jobs::promote(&ServerState, Option<Job>)` is called after every terminal transition; it is the only place starters run.

## AI lane

- Chat turns, paragraph review and generation call `admit`; a `Queued` result returns `202` with the job, as admission does today. Their existing workers move into starter closures unchanged.
- The `Busy` error and "An AI run is active for <date or chat>; view it or cancel it." are removed from server and client (`AIChat.tsx`, `app-actions.ts`, `generation/controller.ts`); the client shows "Queued · 2nd in AI lane".
- **Stale-input check at promotion.** Inputs stay captured at admission (the GUI's flush-before-admission and no-wait Authentic capture depend on it). When a queued generation, Illustrated text job or paragraph review is promoted, compare its captured Authentic revision (or paragraph snapshot) with the current one; if it changed, fail with `input_changed` without calling the agent, and show "Entry changed while queued — start again". Chat turns have no baseline and skip the check.
- A chat conversation still allows one unsettled turn; sending another turn in the same chat while one is queued keeps the existing per-chat rule.
- `ai-job-experience.md` state table gains a `Queued` row (position, Cancel, no elapsed time until running; elapsed starts at promotion).

## Export lane

- Today an export enters the coordinator only after its durable operation exists (`exports::sync`). M38 admits the export job first, then runs `Exports::start` in its starter. A queued export has no durable row, so dropping it on restart (O5) needs no recovery.
- Infrastructure keeps its own one-export rule as a backstop; with the lane it is never hit in normal use.
- Paused for confirmation (Q6): the paused job releases the lane and is shown as paused, not running. Its confirmation step is admitted at the **front** of the export lane, because the confirmation token expires after 60 seconds and must not wait behind a long Notion photo upload. On promotion the step re-checks token expiry and the remote fingerprint as today; an expired token returns the existing "needs a new preview" outcome. Uncertain/abandon flows are unchanged.
- With exports admitted before `start`, `ExportController`'s client-side step timer (kept in M37) can fold into the shared progress reporter; do so only where message text stays equivalent.

## Illustrated candidates

**Kinds.** `illustrated` (text job, agent lane) and new `illustrated_render` (image lane). Origin for both is the date; render progress carries `set_id`, `slot`, `candidate`.

**Text job.** Captures the Authentic revision, engine id and prompt settings, runs the existing agent mode (text blocks plus `{alt, prompt}` slots, theme grouping and `max_photos` cap unchanged), composes each slot's final prompt (trigger + directives, as today), writes the candidate set manifest, then admits K (Q1) render jobs per slot in slot order. The text job ends before any render starts, freeing the agent lane.

**Render job.** Renders one image through the P1-M33 registry with a new random seed, writes `NN-<candidate-id>.png` exclusively, syncs it, then appends the candidate to the manifest. If the engine id is no longer configured, the job fails with `engine_unavailable` and other candidates are unaffected. Renders use the stored final prompt, so prompt-setting reloads do not change queued work.

**Store** (`diary-infrastructure`, beside the Illustrated adapter):

```text
.diary/illustrated-candidates/<date>/<set-id>/manifest.json
.diary/illustrated-candidates/<date>/<set-id>/01-<candidate-id>.png
```

Manifest (schema 1): date, set id, created at, Authentic revision, engine id, rewritten text with slot placeholders, slots `[{alt, prompt, candidates: [{id, seed}], pick?}]`. Reuse the Authentic/Illustrated adapter's workspace lock, private permissions, symlink refusal and parent sync. Manifest updates are write-temp-then-rename under the Illustrated store mutex. Startup removes PNGs not in a manifest, temp files and set directories without a manifest. One open set per date (Q4). Per-slot candidate cap (Q3). Nothing goes into SQLite, matching M34. A manifest file instead of SQLite rows is the agent's call: the set lives and dies with its PNG directory.

**Routes** (session-authenticated; bridge commands mirror them):

| Route                                                          | Action                                                                                                                                                                            |
| -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `POST /api/v1/illustrated/{date}` (existing)                   | Admits the text job; refused while a set is open (Q4)                                                                                                                             |
| `GET /api/v1/illustrated/{date}/candidates`                    | Set manifest view: slots, candidate ids, picks, stale flag (Q5), queued/running render job ids                                                                                    |
| `GET /api/v1/illustrated/{date}/candidates/{slot}/{candidate}` | PNG bytes, same 32 MiB bound and object-URL handling as published photos                                                                                                          |
| `POST /api/v1/illustrated/{date}/candidates/{slot}/reroll`     | Admits K more render jobs for that slot (request ID for idempotency)                                                                                                              |
| `PUT /api/v1/illustrated/{date}/candidates/{slot}/pick`        | Records the pick in the manifest                                                                                                                                                  |
| `POST /api/v1/illustrated/{date}/candidates/publish`           | Requires a pick per slot (Q2); stages text + picked PNGs under `illustrated-pending/` and calls the existing `publish`; then cancels the set's queued renders and deletes the set |
| `DELETE /api/v1/illustrated/{date}/candidates`                 | Discard: cancels queued/running renders of the set, then deletes it                                                                                                               |

Publish keeps M34's all-or-nothing pair publication, silent replacement and journal recovery. A crash after publish but before set deletion leaves a set whose picks equal the published version; startup deletes a set whose manifest is marked `published`.

**Same-date interactions.** Deleting the published Illustrated entry does not touch an open set. Publishing while renders are queued is allowed once every slot has a pick. A running render finishing after publish/discard finds its set gone and discards its PNG.

## Client

- `events.ts`/`validation.ts`: `lane`, `queue_position`, kind `illustrated_render`.
- `ai-activities`: group by lane; queued rows show position and Cancel, not elapsed time; progress reporter skips queued jobs (no two-second messages until running).
- Illustrated: a candidate controller (session-owned, no React) loads the manifest view, refetches on `job_changed` for the set's render jobs, and owns pick/re-roll/publish/discard. The Illustrated tab renders a per-slot row of thumbnails via the existing authenticated photo bridge and temporary object URLs, bounded to visible slots.
- Guards: Illustrated creation no longer waits for unrelated jobs; generation/chat busy reasons removed.

## Sequencing

1. Stage A: coordinator lanes, then DTO/bridge fields. `cargo test -p diary-application`.
2. Stage B: AI lane (starters, busy removal), then export lane.
3. Stage C: store, text job, render job, routes, restart behavior; extend `smoke/illustrated.py` with the fake engine.
4. Stage D: client activity lanes, picker, guards.
5. Stage E: smokes, live run (Q8), gate, docs.

## Risks

- **Starter leaks or double starts.** Mitigation: one `promote` helper, unit tests that each terminal path promotes exactly once, and a debug assertion that a starter exists for every promoted job.
- **Finished render jobs pile up** in `session.jobs`, replay and the client `AIActivitiesStore` budget (slots × K × re-rolls across dates). Mitigation: prune a set's terminal render jobs from the coordinator when the set is published or discarded, and show render jobs in AI Activities only while queued/running; the set view is their lasting record.
- **Disk growth** from K × slots × re-rolls (2–4 MB per PNG). Mitigation: per-slot cap, one open set per date, deletion on publish/discard, startup orphan sweep. Report set size in the manifest view.
- **Lock order** between coordinator, Illustrated store and candidate manifest. Mitigation: coordinator lock is never held while touching the filesystem; manifest writes take only the store mutex.
- **Export semantics.** Moving admission before `start` touches M25 ordering guarantees. Mitigation: B02 reruns all export recovery/concurrency tests unchanged except admission-order tests, which are listed.
- **UI copy changes** in decided behavior (busy refusal). Mitigation: O1 records the owner decision; docs updated in E04.
