# Notion Automatic Day Reorder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a Notion month page's dated day headings are out of ascending order, the export reorders them itself (copy, verify, delete) before writing the requested day.

**Architecture:** `look` stops treating disorder as a conflict and reports which days to move (longest ascending subsequence stays). `apply` runs `Run::reorder` first: per moved day, insert a copy at its correct position, read it back and compare digests, then delete the original. Refusals (differing duplicates, foreign blocks, formatted text, legacy toggles) write nothing.

**Tech Stack:** Rust (`diary-infrastructure` Notion adapter and in-memory fake), Python smoke script, Markdown docs.

**Spec:** [docs/superpowers/specs/2026-11-07-notion-reorder-days-design.md](../specs/2026-11-07-notion-reorder-days-design.md). **Prerequisite:** all tasks of [the heading-day plan](2026-11-07-notion-heading-days.md) through its Task 4 are done; this plan uses its `Located`, `Slot`, `wire::DayRange`/`partition`, `Run::heading`/`append`/`delete`, `Notion::read_range` and the test helpers `Env::days`/`Env::range_texts`.

## Global Constraints

- Owner decision (2026-11-07): reorder is **automatic, no prompt**; there is no new UI action or confirmation.
- An original day is deleted only after its copy's read-back digest equals the original's (O10 order). Delete body blocks in order, heading last.
- `observe` and `read_back` never write. Only `apply` reorders.
- Only the month page is reordered; year and month pages keep creation order.
- Refuse with `Unavailable::Structural` / `Applied::Failed` and **no write** for: differing duplicate dates; a day to be moved containing a block outside the projection's output set (O12, including a stray `toggle`); a day to be moved with any non-plain rich text (an annotation set, non-default color or link); a legacy dated toggle; a refusal names its day in the log (no text content).
- Identical duplicate (a byte-identical twin left by a crash) is the only duplicate repaired: delete the later copy.
- No new dependency, no database migration (`notion_steps` kinds `move_copy` and `move_delete` are plain strings), no change to iCloud or the HTTP client.
- Per `AGENTS.md`: checklist before code; done means `scripts/gate.sh` passes. Commit only the files each task names (the tree may hold unrelated edits; use `git add -p` for the P1-M29 overview).

## Review Focus

- An already ascending month page: zero extra requests (no copy, no delete, no extra listing beyond `look`).
- Disorder plus a target date not yet present: the new heading goes to the right slot computed after the reorder, not from the stale listing.
- The requested date is itself a moved day: the write targets its new heading ID.
- Two same-date ranges, identical (crash leftover) versus differing: the first is cleaned up, the second refuses.
- A hand-typed non-date `heading_2`, a preamble, and nested list items inside a moved day: the first two never move, the third survives the copy with an equal digest.

## File Structure

`src-tauri/crates/diary-infrastructure/src/export/notion/wire.rs` (`plan_moves`, `Remote.plain`), `adapter.rs` (`look`, `Run::reorder`, `go`), `tests.rs` (matrix), `scripts/smoke/notion-live.py`, docs listed in Task 5.

---

### Task 1: Records and checklist

**Files:**

- Modify: `docs/phase-1/milestone-29-owner-requested-revisions-overview.md`

- [ ] **Step 1: Add the owner decision** under "Owner decisions": `T17 (owner, 2026-11-07): the Notion export reorders out-of-order day headings automatically, with no prompt; originals are deleted only after the copy is verified.`
- [ ] **Step 2: Add unchecked tasks** `T17 [behavior]` (planner and `look`: `plan_moves`, disorder reporting, tests in `wire.rs`/`export/notion/tests.rs`), `T18 [behavior]` (`Run::reorder`, crash and refusal cases), `T19 [docs]` (architecture text, changelog, live leg) and `T19.1` (owner-run live reorder leg records its result). Each names its evidence, as T01-T12 do.
- [ ] **Step 3: Commit**

```bash
git add -p docs/phase-1/milestone-29-owner-requested-revisions-overview.md
git add docs/superpowers
git commit -m "docs: plan automatic Notion day reorder"
```

### Task 2: Move planner and plain-text flag

**Files:**

- Modify: `.../export/notion/wire.rs` (`Remote`, `parse_page`, new `plan_moves`)
- Test: unit tests in `wire.rs`

**Interfaces:**

- Produces: `pub fn plan_moves(dates: &[String]) -> Vec<usize>` (ascending indexes into `dates` of days to move; `dates` are unique ISO strings in page order); `Remote.plain: bool` (true when every rich-text item has no annotations set, `color == "default"` and no `href`; `true` when there is no rich text).

- [ ] **Step 1: Write failing tests**
  - `plan_moves_leaves_an_ascending_page_alone`: `[a,b,c]` gives `[]`.
  - `plan_moves_moves_one_misplaced_day`: dates `["2026-11-01","2026-11-05","2026-11-03","2026-11-07"]` gives `[1]` (the longest ascending run is 01,03,07; 05 moves). Also `["...-02","...-01","...-03"]` gives `[0]`.
  - `plan_moves_on_a_reversed_page_moves_all_but_one`: five reversed dates give four indexes.
  - `plan_moves_prefers_the_longer_run`: `["2026-11-03","2026-11-01","2026-11-02"]` gives `[0]` (the run 01, 02 beats 03 alone).
  - `plan_moves_keeps_the_earliest_day_on_equal_length_ties`: `["2026-11-02","2026-11-01"]` gives `[1]` (ledger ruling: the spec keeps the earliest-positioned day).
  - `parse_page_marks_formatted_text_not_plain`: one fixture item with `annotations.bold = true`, one with `href`, one with `color = "red"`, one plain; assert `plain` is `false,false,false,true`.

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p diary-infrastructure wire`
Expected: compile error / failures.

- [ ] **Step 3: Implement** `plan_moves` as the standard O(n log n) or O(n²) longest strictly increasing subsequence over the ISO strings (lexicographic order equals date order), preferring the earliest-ending run on ties, and returning the complement indexes. Set `plain` in `parse_page` from the raw `rich_text` array (`annotations` booleans, `color`, `href`; the existing `plain` function stays for text).

- [ ] **Step 4: Run to confirm pass**

Run: `cargo test -p diary-infrastructure wire`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/diary-infrastructure/src/export/notion/wire.rs
git commit -m "feat: plan Notion day moves"
```

### Task 3: `look` reports disorder instead of failing

**Files:**

- Modify: `.../export/notion/adapter.rs` (`Located`, `look`, `observation`)
- Test: `.../export/notion/tests.rs`

**Interfaces:**

- Consumes: `wire::partition`, `wire::plan_moves`, `Remote.plain`.
- Produces: `struct Day { iso: String, heading: String, body: Vec<String>, plain: bool, foreign: bool }` (in page order) and on `Located`: `days: Vec<Day>`, `moves: Vec<usize>` (indexes into `days` from `plan_moves`), `twin: Option<Day>` (the later of two byte-identical same-date ranges, to delete). `slot`/`tail` semantics from the heading-day plan are unchanged; they describe the **current** listing and are recomputed after a reorder.

- [ ] **Step 1: Write failing tests**
  - `observing_a_disordered_page_is_read_only_and_not_structural`: month with days 05, 01, 03 (use `external_block` for the middle one); `observe(03)` is present and available, `unavailable == None`, and `e.fake.count("append") + e.fake.count("delete") == 0`.
  - `differing_duplicate_dates_still_refuse`: two 03 headings with different bodies → `Unavailable::Structural`.
  - `identical_twins_are_observed_as_present_and_flagged`: put 03, copy it with `external_block` calls (same heading text and body text) → `observe(03)` available; assert the adapter-visible `moves` via a `#[cfg(test)]` accessor or by the later apply test in Task 4.
  - Keep (adapt) the heading-day tests `a_legacy_dated_toggle_is_a_structural_conflict` and `a_foreign_block_in_a_section...`: foreign blocks in the **target** day still refuse, and in a **to-be-moved** day refuse at apply (Task 4); `look` marks `foreign` / `plain=false` per day instead of failing for non-target days.

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p diary-infrastructure export::notion`
Expected: the disorder test fails with `Structural`.

- [ ] **Step 3: Implement.** In `look`: build `days` from `partition`; reject duplicate dates unless exactly two ranges share a date and their `Section::digest()` are equal (then `twin = later`, and the earlier one stays the canonical day); never fail on descending order; set `moves = plan_moves(dates)`; compute `foreign` (any child kind outside `projection::Kind::from_tag`, via the existing read) and `plain` only for days in `moves` (other days are not inspected); the target day is validated as before. Keep every legacy-toggle rejection.

- [ ] **Step 4: Run to confirm pass**

Run: `cargo test -p diary-infrastructure export::notion`
Expected: the Task 3 tests pass; earlier tests unchanged and passing.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/diary-infrastructure/src/export/notion/adapter.rs src-tauri/crates/diary-infrastructure/src/export/notion/tests.rs
git commit -m "feat: report Notion day disorder from look"
```

### Task 4: Reorder in `apply`

**Files:**

- Modify: `.../export/notion/adapter.rs` (`Run::reorder`, `Run::go`, `apply`)
- Test: `.../export/notion/tests.rs`

**Interfaces:**

- Consumes: `Located.days/moves/twin`, `Run::heading`, `Run::append`, `Run::delete`, `Notion::read_range`, `projection::chunk`.
- Produces: `Run::reorder(&mut self, month: &str, loc: &Located) -> Result<(), Stop>`; `go` calls it when `!loc.moves.is_empty() || loc.twin.is_some()`, then **re-runs `look`** for the requested date and continues with the fresh `Located`. `apply` rejects (`Applied::Failed`, nothing written) any moved day that is `foreign` or not `plain` before the first write.

- [ ] **Step 1: Write failing tests**
  - `one_misplaced_day_moves_alone`: days 01, 05, 03, 07 exported through the adapter with an external swap; one export of 09 ends with `days()` ascending and all body texts equal to the originals; `copy` = 1 heading append plus its chunk appends, `delete` = old body blocks + 1.
  - `an_ascending_page_costs_no_extra_requests`: compare `e.fake.calls()` of an export on an ascending page with the heading-day baseline (same call list).
  - `a_reversed_page_moves_the_minimum`: five reversed days → four moved, one left; assert unmoved day's heading ID unchanged and the others changed.
  - `crash_between_copy_and_delete_then_rerun_ends_clean`: `fault("delete", Fault::Drop)` on the first delete → `Applied::Unknown`; page has the moved day twice; a second `put` finishes with one copy, no lost text, and `count("append")` for the rerun adds no second copy (identical twin repaired by deleting the later copy).
  - `a_lost_reply_on_the_copy_heading_is_unknown_never_replayed`.
  - `a_digest_mismatch_on_read_back_stops_before_deleting`: use `edit_text` on the fake from a fault hook or truncate the copy, assert `count("delete") == 0` and `Applied::Unknown`.
  - Refusals each assert `Applied::Failed` and **zero** `append`/`delete` calls: `a_formatted_day_is_not_moved` (fake block with a bold annotation, needs `fake.rs` to allow `external_block` with annotations), `a_foreign_block_in_a_moved_day_refuses`, `differing_duplicates_refuse`, `a_legacy_toggle_refuses`.
  - `the_requested_date_is_itself_moved_and_gets_written` and `a_new_date_goes_to_its_slot_after_a_reorder`.
  - `a_preamble_and_an_undated_heading_never_move`.
  - `a_month_spanning_several_api_pages_reorders` (`with_page_size`).
  - `a_moved_day_with_nested_list_items_keeps_equal_digest`.

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p diary-infrastructure export::notion`
Expected: the new tests fail.

- [ ] **Step 3: Implement `Run::reorder`.** For each index in `moves` in ascending date order, re-`look` the month (a fresh `Located`), then: read the day's body as `Block`s via `read_range`; insert the copy with `Run::heading` using the slot for that date from the fresh listing and `Run::append` per chunk of `projection::chunk(...)` (checkpoint kinds `move_copy`); `read_range` the copy and require `digest()` equal to the original's, else `return Err(Stop::Unknown)`; delete the original body IDs then its heading (`move_delete`). Then handle `twin`: if present, delete its body IDs and heading. Note the copy's source `Section` already holds `Block`s with children (nested items), so `wire::block` re-creates them; a chunk larger than `CHUNK_BYTES` aborts before the first write.

- [ ] **Step 4: Run to confirm pass**

Run: `cargo test -p diary-infrastructure export::notion && cargo test -p diary-domain`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/diary-infrastructure/src/export/notion
git commit -m "feat: reorder out-of-order Notion days automatically"
```

### Task 5: Live leg, docs and proof

**Files:**

- Modify: `scripts/smoke/notion-live.py`, `docs/architecture/configuration-and-exports.md:81` (the clause "pre-existing disorder ... return a structural conflict requiring external repair"), `docs/phase-1/milestone-27-notion-export-overview.md` (O8 note), `docs/phase-1/changelog.md`, the P1-M29 overview.

- [ ] **Step 1: Add a reorder leg to `notion-live.py`.** After the existing legs: create days 03 and 05 through the app, move 05's heading and body above 03 directly with the API (append copies after the month's start, delete the originals), export day 07, assert the month's `heading_2` texts are ascending and every body unchanged; print `Q3: reorder moved <n> days`. `python3 -m py_compile scripts/smoke/notion-live.py` must pass.
- [ ] **Step 2: Docs.** Replace the repair clause with the automatic reorder behavior and its refusals; add the O8 note (ordering is now repaired, not only verified); add a changelog entry (behavior change, no prompt, IDs of moved days change).
- [ ] **Step 3: Run the full gate**

Run: `scripts/gate.sh`
Expected: G01-G07 all PASS.

- [ ] **Step 4: Tick T17-T19 with evidence** (test names and the gate result). Leave `T19.1` for the owner's live run of `scripts/smoke/notion-live.py` with `DIARY_NOTION_LIVE=1`, `DIARY_NOTION_LIVE_TOKEN`, `DIARY_NOTION_LIVE_ROOT`; record `Q3` there.
- [ ] **Step 5: Commit**

```bash
git add scripts/smoke/notion-live.py docs/architecture/configuration-and-exports.md docs/phase-1/milestone-27-notion-export-overview.md docs/phase-1/changelog.md
git add -p docs/phase-1/milestone-29-owner-requested-revisions-overview.md
git commit -m "docs: record automatic Notion day reorder"
```
