# Notion Heading-Day Export Implementation Plan

Status: Implemented (2026-11-07)

Task plan for the [heading-day design](../specs/2026-11-07-notion-heading-days-design.md): each diary day becomes a `heading_2` plus plain sibling blocks in the month page.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Export each diary day to Notion as a `heading_2` titled `YYYY-MM-DD (WED)` followed by plain sibling blocks in the month page, replacing the daily toggle.

**Architecture:** A day becomes a _range_: a dated `heading_2` plus the siblings up to the next dated `heading_2`. `look` partitions the month page's children once; writes insert the heading and chunks into the month page with `position`; replace keeps O10's order (insert new range after the old range, verify, delete old range, heading last).

**Tech Stack:** Rust (`diary-domain`, `diary-infrastructure`), the in-memory Notion fake, Python smoke scripts, Markdown docs.

**Spec:** [docs/superpowers/specs/2026-11-07-notion-heading-days-design.md](../specs/2026-11-07-notion-heading-days-design.md)

## Global Constraints

- Day title is exactly `YYYY-MM-DD (WED)`: English uppercase `MON`..`SUN` from `diary_domain::calendar::weekday` (Sunday is 0).
- A day heading is a `heading_2` whose text starts with a valid ISO date followed by end of text or whitespace; the suffix is not checked on read.
- Every projected Markdown heading (levels 1-3) becomes `heading_3`; levels 4-6 keep the existing warning.
- No migration: a `toggle` whose text is a valid ISO date anywhere in a month page is a structural conflict.
- No new dependency, no database migration, no change to iCloud, year/month page titles, the HTTP client or the confirmation/cancel/abandon flows.
- Old text survives until the new range is verified (O10). Blocks outside the projection's output set inside the target range are a structural conflict (O12). The app never archives or deletes Notion pages.
- Per `AGENTS.md`: checklist in the milestone overview before code; done means `scripts/gate.sh` passes. The working tree already holds unrelated uncommitted edits (P1-M29 T11/T12): commit only the files each task names, using `git add -p` for `docs/phase-1/milestone-29-owner-requested-revisions-overview.md`. `scripts/gate.sh` may already fail on `cargo fmt --check` for `diary-logging` `codec.rs`/`codec/tests.rs` (noted in P1-M29 T10); report that, do not fix it here.

## Review Focus

- A day with an empty body (heading only, zero following blocks): applies, observes and replaces without a phantom range.
- A hand-typed `heading_2` in Notion with no date, or a dated one with a wrong weekday: the first is absorbed into the previous day (shows as Changed), the second still identifies its date.
- A Markdown body containing `#` and `##` headings: they project to `heading_3` and never split the range.
- Blocks before the first day heading (user preamble): untouched by create, backdate and replace; never read as part of a day.
- A month page longer than one API page (with_page_size small): every day is found and ordered.

## File Structure

- `src-tauri/crates/diary-domain/src/notion.rs` (+ `notion/tests.rs`): heading collapse, `day_title`.
- `src-tauri/crates/diary-infrastructure/src/export/notion/wire.rs`: heading builder and reply/title parsing.
- `.../notion/adapter.rs`: partition, `look`, write path. `.../notion/tests.rs`: matrix. `.../notion/fake.rs`: only if a test needs it (it already supports `start`/`after_block`).
- `scripts/smoke/export.py`, `scripts/smoke/notion-live.py`: new shape.
- Docs: `docs/architecture/configuration-and-exports.md`, `docs/phase-1/milestone-27-notion-export-{overview,architecture}.md`, `docs/phase-1/milestone-29-owner-requested-revisions-overview.md`, `docs/phase-1/changelog.md`.

---

### Task 1: Branch and checklist

**Files:**

- Modify: `docs/phase-1/milestone-29-owner-requested-revisions-overview.md` (append after T12)

**Interfaces:**

- Produces: checklist items `T13`-`T16` and `T16.1` that later tasks tick with evidence.

- [ ] **Step 1: Create the branch**

Run: `git switch -c notion-heading-days` (main is the default branch; uncommitted edits travel with you).

- [ ] **Step 2: Add the checklist** under "Proof and docs"'s predecessor list, unchecked, each naming its evidence:
  - `T13 [behavior]` domain: `day_title` and heading collapse (tests in `diary-domain` `notion::tests`).
  - `T14 [behavior]` adapter read side: partition, ordering, structural conflicts (`export/notion/tests.rs`).
  - `T15 [behavior]` adapter write side: create, backdate, replace, crash and lost-reply cases (`export/notion/tests.rs`).
  - `T16 [docs]` architecture, P1-M27 wording, changelog; smoke scripts updated; `export.py` fake leg passes.
  - `T16.1` owner-run live round trip (`scripts/smoke/notion-live.py`) records Q1 and Q2 of the spec.

- [ ] **Step 3: Commit**

```bash
git add -p docs/phase-1/milestone-29-owner-requested-revisions-overview.md   # stage only the new lines
git add docs/superpowers
git commit -m "docs: plan Notion heading-day export"
```

### Task 2: Domain — title and heading collapse

**Files:**

- Modify: `src-tauri/crates/diary-domain/src/notion.rs` (the `Tag::Heading` arm near line 200; new `day_title`)
- Test: `src-tauri/crates/diary-domain/src/notion/tests.rs`

**Interfaces:**

- Produces: `pub fn day_title(date: DiaryDate) -> String` in `diary_domain::notion`.

- [ ] **Step 1: Write failing tests**
  - `day_title_has_iso_date_and_english_weekday`: `day_title(2026-11-04) == "2026-11-04 (WED)"`, `2026-11-08 == "... (SUN)"`, `2026-11-02 == "... (MON)"`.
  - `body_headings_never_project_as_heading_two`: `project("# A\n\n## B\n\n### C\n\n#### D\n")` has kinds `["heading_3","heading_3","heading_3","heading_3"]` and `warnings == 1`.
  - Update the existing `supported_blocks_project_in_order` (first kind becomes `heading_3`) and `deep_headings_and_nesting_are_restructured_with_a_warning` if they assert `heading_1`/`heading_2`.

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p diary-domain notion -- --nocapture`
Expected: the new tests fail (`day_title` missing, kinds differ).

- [ ] **Step 3: Implement** `day_title` using a `["SUN","MON",..,"SAT"]` table indexed by `calendar::weekday(date)`; in the heading arm emit `Kind::Heading(3)` for every level, keeping `if n > 3 { warnings += 1 }`.

- [ ] **Step 4: Run to confirm pass**

Run: `cargo test -p diary-domain`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/diary-domain/src/notion.rs src-tauri/crates/diary-domain/src/notion/tests.rs
git commit -m "feat: add Notion day title and collapse body headings"
```

### Task 3: Adapter read side — partition, look, observation

**Files:**

- Modify: `.../export/notion/wire.rs` (replace `toggle`, `new_toggle`; rename `Section::fingerprint` parameter to `heading_id`), `.../export/notion/adapter.rs` (`Located`, `look`, `observation`)
- Test: `.../export/notion/tests.rs`

**Interfaces:**

- Produces in `wire.rs`:
  - `pub fn heading(title: &str) -> Value` (a `heading_2`).
  - `pub fn new_heading(v: &Value, title: &str, old: Option<&str>) -> Option<String>` (the `heading_2` in an append reply whose trimmed text equals `title` and whose ID is not `old`).
  - `pub fn day_of(r: &Remote) -> Option<DiaryDate>` (the day-heading rule in Global Constraints).
  - `pub fn is_legacy_day(r: &Remote) -> bool` (`toggle` whose trimmed text parses as an ISO date).
  - `pub struct DayRange { pub iso: String, pub heading: usize, pub end: usize }` and `pub fn partition(items: &[Remote]) -> Vec<DayRange>`: pure, no validation; `heading` is the index of the heading, `end` the exclusive index of the range's end (next day heading or `items.len()`).
- Produces in `adapter.rs`: `Located { year, month, dated: Vec<(String, String)> /* iso, heading id */, section: Option<(String, Section)> /* heading id, body */, tail: Option<String> /* last block id of the target range, else its heading */, slot: Slot, ids: HashSet<String> /* every month child id */ }` and `enum Slot { Start, After(String), End }` — where a _new_ date's heading goes (Slot is meaningful only when `section` is `None`).
- Consumes: `Remote`, `Section::new`, `read_blocks` (kept for nested children).

- [ ] **Step 1: Write failing tests** (rewrite the read-side tests; add helper `Env::days(&self) -> Vec<String>` returning the text of each `heading_2` child of `e.month()`, and `Env::range_texts(&self, heading: &str) -> Vec<String>` returning the text of the siblings after it up to the next heading_2)
  - `partition_splits_on_dated_heading_two_only` (unit test in `wire.rs`): items `[p, h2 "2026-11-01 (SUN)", p, h2 "notes", p, h2 "2026-11-03"]` give ranges `("2026-11-01", 1..5)` and `("2026-11-03", 5..6)`; the leading `p` belongs to none.
  - `day_of_ignores_the_weekday_suffix`: `"2026-11-04 (FRI)"` and `"2026-11-04"` both yield 2026-11-04; `"2026-11-04x"`, `"2026-13-40"` and a `heading_3` yield `None`.
  - `observing_an_absent_date_reports_slot_and_no_write`: via `Notion::observe` on a month with days 03 and 10: absent for 05 and no fake write calls (`e.fake.count("append") == 0`).
  - Adapt `duplicate_and_disordered_hierarchy_is_a_structural_conflict`: use `external_block` to add dated `heading_2` blocks (duplicate and descending); expect `Unavailable::Structural`. Add `a_legacy_dated_toggle_is_a_structural_conflict` using the existing toggle JSON from line 161.
  - Adapt `a_foreign_block_in_a_section_is_a_structural_conflict_with_no_write`: `external_page(&month, ...)` placed inside a range via `external_block(..., Some(index))` or `external_page` appended after the date's heading; expect Structural on observe and `Applied::Failed` from `apply`.
  - `pagination_reads_every_page_of_a_long_month` (adapt, `with_page_size`).

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p diary-infrastructure export::notion`
Expected: compile errors, then failures.

- [ ] **Step 3: Implement** `wire::heading`, `new_heading`, `day_of`, `is_legacy_day`, `DayRange`, `partition`. Rewrite `look`: list the month once; reject any `is_legacy_day` item; take `partition(&items)`; require strictly ascending ISO dates (duplicate or descending → `Fail::Structural`); build `dated`; for the target date build `section` by validating every item in its range against `projection::Kind::from_tag` (else `Fail::Structural`) and reusing `read_blocks` recursion for children of items with `has_children`; compute `tail`, `slot` (`Start` if the first later heading is item 0, `After(id of the item before it)` otherwise, `End` if none later) and `ids`. Keep the lines/fingerprint format (`id|kind|text|{sub}`) over body blocks only; `fingerprint(heading_id)` still prefixes the heading ID. `Observation.ids` is the heading ID.

- [ ] **Step 4: Run to confirm pass**

Run: `cargo test -p diary-infrastructure export::notion::tests::{duplicate,legacy,foreign,pagination,observing} && cargo test -p diary-infrastructure wire`
Expected: pass (write-side tests still fail until Task 4; leave them failing).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/diary-infrastructure/src/export/notion/wire.rs src-tauri/crates/diary-infrastructure/src/export/notion/adapter.rs src-tauri/crates/diary-infrastructure/src/export/notion/tests.rs
git commit -m "feat: read Notion days as heading ranges"
```

### Task 4: Adapter write side — create, backdate, replace

**Files:**

- Modify: `.../export/notion/adapter.rs` (`Run::section` → `Run::heading`, `Run::append`, `Run::go`, `apply`, the verify read), `.../export/notion/wire.rs` (`pub fn last_new(v: &Value, known: &HashSet<String>) -> Option<String>`)
- Test: `.../export/notion/tests.rs`

**Interfaces:**

- Consumes: `Located` and `Slot` from Task 3, `day_title`, `wire::heading`, `wire::new_heading`.
- Produces: `Run { known: HashSet<String>, .. }` seeded from `Located.ids`; every ID a reply returns is added to `known`. `Run::heading(&mut self, month, date, slot: &Slot, old_tail: Option<&str>) -> Result<String, Stop>`; `Run::append(&mut self, month, chunk, after: &str) -> Result<String, Stop>` (returns the chunk's last new block ID via `last_new`); `Notion::read_range(&self, month: &str, heading: &str) -> Result<Section, Fail>` (lists the month, uses `partition`, tolerates duplicates, returns the range after `heading`).

- [ ] **Step 1: Write failing tests** (adapt the existing ones to `e.days()`/`e.range_texts()`)
  - `create_builds_the_hierarchy_and_exact_content`: month `days()` is `["2026-11-07 (SAT)"]`, body digest matches, steps are 4 and all `done` (two pages, heading, one chunk).
  - `days_stay_ascending_for_first_middle_and_last_insertion`: puts 10, 05, 07, 20, 01; `days()` is the five headings in ascending order with each `range_texts` equal to its entry, even for the day placed at the start (`position: start`).
  - `a_preamble_before_the_first_day_survives_every_write`: `external_block(&month, paragraph "preamble", None)` first; then put 05, put 01 (backdate), replace 05; the preamble stays child 0 and is never in any range.
  - `an_empty_body_is_a_heading_with_no_siblings`: put `""` creates only the heading; observe is present; replace with text and back to `""` work.
  - `body_headings_do_not_split_the_range`: put `"# A\n\n## B\n\ntext\n"`; `days()` has one entry; `range_texts` is three entries.
  - `replace_inserts_the_new_range_after_the_old_one_then_removes_the_old_range`: after replace, exactly one range remains, the heading ID changed, `delete` count equals old body blocks + 1, heading deleted last (assert via `e.fake.calls()` order).
  - `a_crash_between_the_new_range_and_the_old_delete_keeps_the_old_text`: `fault("delete", Fault::Drop)` → `Applied::Unknown`; old body text still present; two same-date headings; observe is `Structural`. Second half: `fault_after("append", 1, Fault::Throttle(90))` → `Unknown`, old range untouched, `count("delete") == 0`.
  - `a_lost_reply_on_the_heading_append_is_unknown_never_replayed`: `fault("append", Fault::LoseReply)` → `Unknown`, `count("append") == 1`, the heading exists.
  - `a_hand_typed_undated_heading_two_is_absorbed_into_the_previous_day`: external `heading_2 "Notes"` after day 05's body → observe sees a different digest than the exported one (Changed), and a replace of 05 deletes it with its range only through the confirmation flow's `Write::Replace` (assert `apply` with the stale expected observation returns `Failed`).
  - Update `long_unicode_entries_chunk_and_round_trip`, `an_external_edit_between_observe_and_apply_fails_without_writing`, `concurrent_exports_never_create_duplicate_year_or_month_pages` for the new helpers.

- [ ] **Step 2: Run to confirm failure**

Run: `cargo test -p diary-infrastructure export::notion`
Expected: write-side tests fail.

- [ ] **Step 3: Implement.**
  - `heading`: `PATCH /v1/blocks/{month}/children` with `wire::append(vec![wire::heading(&day_title(date))], after, start)` where for a replace `after = old_tail`, otherwise from `Slot` (`Start` → `start=true`; `After(id)` → `after`; `End` → neither); id from `new_heading`.
  - `append`: same endpoint, `after` = the previous block ID (the heading for the first chunk); `last_new` returns the last result ID not in `known`; checkpoint `kind "append"`, `parent` = month ID.
  - `go`: create the pages as today, `heading`, append chunks, and for a replace call `read_range(month, new_heading)` and compare `digest()` to `want` (else `Stop::Unknown`), then delete the old body IDs in order and the old heading last (reuse `Run::delete`, including its `NotFound` tolerance).
  - `apply`'s final check uses `look` (it must see exactly one range after a replace).

- [ ] **Step 4: Run to confirm pass**

Run: `cargo test -p diary-infrastructure export::notion && cargo test -p diary-domain`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/diary-infrastructure/src/export/notion
git commit -m "feat: write Notion days as heading ranges"
```

### Task 5: Smoke scripts

**Files:**

- Modify: `scripts/smoke/export.py` (`section_texts` near line 256 and its callers at 278-292), `scripts/smoke/notion-live.py` (`month_toggles` at 166 and its callers at 175-212)

**Interfaces:**

- Consumes: the new shape. Produces: scripts that assert it.

- [ ] **Step 1: Update `export.py`.** `section_texts()` finds the `heading_2` whose text starts with `DATE`, returns `(heading_id, texts of the following siblings up to the next heading_2)`; the replace leg asserts the heading ID changed and the texts equal `['Notion three.']`.

- [ ] **Step 2: Update `notion-live.py`.** `month_days()` returns the month ID and the month's `heading_2` blocks; ordering assertions use heading texts; add prints labelled `Q1:` (what `PATCH .../children` with `position` returned: number of results vs. month children) and `Q2:` (whether deleting a range's blocks in order, heading last, succeeded). Remove the Q4 toggle-nesting probe or retarget it at list items under a day's paragraphs.

- [ ] **Step 3: Run the fake leg**

Run: `python3 scripts/smoke/export.py` (the invocation its header documents)
Expected: exits 0 with the Notion legs passing. The live script is owner-run (Task 6, T16.1) and is only syntax-checked here: `python3 -m py_compile scripts/smoke/notion-live.py`.

- [ ] **Step 4: Commit**

```bash
git add scripts/smoke/export.py scripts/smoke/notion-live.py
git commit -m "test: update Notion smoke legs for heading days"
```

### Task 6: Docs, gate and owner-run proof

**Files:**

- Modify: `docs/architecture/configuration-and-exports.md:81`, `docs/phase-1/milestone-27-notion-export-overview.md` (owner decisions O3, O8, O10, O12, scope text), `docs/phase-1/milestone-27-notion-export-architecture.md` (toggle wording), `docs/phase-1/changelog.md`, the P1-M29 overview checklist.

- [ ] **Step 1: Rewrite the architecture paragraph** (line 81): "one daily toggle section titled ISO date" becomes the heading range with the `YYYY-MM-DD (WED)` title, the demoted body headings, the legacy-toggle conflict and the removal of the stable daily anchor (the heading ID changes on replace, as O10 already says).

- [ ] **Step 2: Add a dated note to the P1-M27 overview** that O3, O8, O10 and O12 stand with "toggle" read as "heading range" (owner, 2026-11-07); fix the remaining "toggle" mentions in both P1-M27 files and the work plan.

- [ ] **Step 3: Add a changelog entry** naming the behavior change and "no migration".

- [ ] **Step 4: Run the full gate**

Run: `scripts/gate.sh`
Expected: exit 0. If it fails only on the `diary-logging` fmt issue noted in Global Constraints, report that and run the remaining stages individually (`cargo clippy`, `cargo test`, `npx prettier --check`, `npx eslint`, `npx vitest run`, `npx tsc --noEmit`).

- [ ] **Step 5: Tick T13-T16 with evidence** (test names, command output summaries). Leave `T16.1` unchecked and ask the owner to run `scripts/smoke/notion-live.py` with `DIARY_NOTION_LIVE=1`, `DIARY_NOTION_LIVE_TOKEN`, `DIARY_NOTION_LIVE_ROOT` and report the `Q1:`/`Q2:` lines; record them in the spec's Open items and tick `T16.1` only then.

- [ ] **Step 6: Commit**

```bash
git add docs/architecture/configuration-and-exports.md docs/phase-1/milestone-27-notion-export-overview.md docs/phase-1/milestone-27-notion-export-architecture.md docs/phase-1/changelog.md
git add -p docs/phase-1/milestone-29-owner-requested-revisions-overview.md
git commit -m "docs: record Notion heading-day export"
```
