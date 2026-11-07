# Notion export: automatic reorder of out-of-order days

Status: Draft for owner review (2026-11-07)

Depends on [Notion heading-day export](2026-11-07-notion-heading-days-design.md) (the day-range reader and writer). Implement after its Task 4.

## Intent

When the dated `heading_2` days of a month page are out of ascending order, the Notion export reads the whole month page and rearranges the days into ascending date order itself, then writes the requested day. The owner chose **automatic, no prompt** (2026-11-07), replacing the rule in `docs/architecture/configuration-and-exports.md:81` that disorder is "a structural conflict requiring external repair". A recorded owner decision for the change goes in the P1-M29 overview.

Success: a month page with misplaced days ends up ascending after one export, no day's text is lost or altered, and every failure mode stops with a typed reason before anything is deleted.

Safety property kept without a prompt: an original day is deleted only after its copy has been read back and its digest equals the original's (the O10 order).

## Behavior

- **Scope:** the month page being exported to. Year and month pages cannot be positioned in Notion and keep their creation order.
- **Observe is read-only.** A month page whose only problem is repairable disorder no longer reports `Unavailable::Structural`; the target date reports its normal state so the export proceeds. Only `apply` writes.
- **Which days move.** Compute the longest strictly ascending subsequence of day dates (ties broken by earliest position). Those days stay. Every other day moves, processed in ascending date order, with the month page re-listed after each move.
- **How one day moves.**
  1. Insert a copy of the heading and its body blocks at the correct position (`position: after_block` the last block of the nearest earlier day, or `start`, or end of page), checkpointing in `notion_steps` before the request.
  2. Read the copy back (`read_range`) and require its digest to equal the original's digest.
  3. Delete the original's body blocks in order, heading last (same steps as a replace).
- **Target day moved.** If the requested date is itself a moved day, the later write reuses the new heading ID from a fresh `look`; the pre-move observation is not reused. The export service's expected-vs-seen check runs before the reorder, as today.
- **Crash or lost reply mid-move.** The day exists twice. `apply` returns `Unknown` (reported `uncertain`). The next `apply` or `observe` finds two same-date ranges; because one is a byte-identical copy of the other, the reorder resolves it by deleting the **later-in-page-order copy only if** it equals its twin's digest, otherwise it stays a structural conflict (no merging of differing text). This is the only duplicate the reorder repairs.

## Refusals (typed structural conflict, nothing written)

- Two headings for the same date whose ranges differ.
- Any block in a day to be moved that is outside the projection's output set (O12), including a stray `toggle`.
- A day to be moved whose text carries formatting the app cannot reproduce: any rich-text item with an annotation set (bold, italic, strikethrough, underline, code), a non-default color, or a link. `wire::Remote` gains `pub plain: bool` (true when every rich-text item is plain), set in `parse_page`; `read_blocks` fails with `Fail::Structural` for a moved day with `plain == false`. Days that are not moved are never inspected for this.
- A legacy dated `toggle` anywhere in the month page (heading-day spec).
- A moved day whose read-back digest differs from the original's: stop before deleting; the copy is left and reported `uncertain`.

A refusal stops the export even when the target date is unrelated; the error names the day. The owner repairs it by hand.

## Interfaces

All in `diary-infrastructure/src/export/notion/`.

- `wire.rs`: `Remote.plain: bool`; `pub fn plan_moves(dates: &[String]) -> Vec<usize>` returns the indexes (into the page's day list) of days to move, via longest strictly ascending subsequence. Duplicates are not accepted here (handled by the caller before the call).
- `adapter.rs`: `look` stops treating disorder as conflict and returns `Located.disorder: Vec<usize>` (the same indexes) alongside a new `Located.days: Vec<DayRange>` (heading ID, block IDs, ISO date) in page order; duplicates still fail except the identical-twin case above. `Run::reorder(&mut self, month: &str, loc: &Located) -> Result<(), Stop>` performs the moves; `Run::go` calls it before writing the requested day whenever `loc.disorder` is non-empty and re-runs `look` afterwards.
- Checkpoint kinds in `notion_steps`: `move_copy`, `move_delete`. No schema change (the table already stores kind, parent, digest and remote IDs).

## Out of scope

Merging days with the same date; reordering year or month pages; preserving block IDs of moved days (they change); preserving formatting by reading annotations; any UI change (no new action, no confirmation); any new dependency or migration.

## Testing and evidence

Fake-HTTP matrix (`export/notion/tests.rs`, `fake.rs` already supports positional insert):

- one misplaced day moves alone and the page ends ascending (assert call count: one copy, N+1 deletes);
- fully reversed order moves the minimum (`plan_moves` unit tests: ascending → none, one out of place, reversed, equal dates excluded);
- crash after the copy, after the digest check, and mid-delete; the rerun ends with no duplicate and no lost text;
- lost reply on the copy's heading append;
- formatted text (bold, link), a foreign block, differing duplicate dates, a legacy toggle: each refuses with no write calls;
- the requested date is a moved day; the requested date is new and disorder exists elsewhere;
- a month page spanning several API pages (`with_page_size`);
- `observe` on a disordered page makes no write calls.

`scripts/smoke/notion-live.py` gains a reorder leg (create three days, move one by hand through the API, export a fourth day, assert ascending). Owner-run, like the other live legs. Full gate: `scripts/gate.sh`.

## Docs and records

`docs/architecture/configuration-and-exports.md` (replace the repair clause), the P1-M27 overview note on O8 (ordering is now repaired, not just verified), `docs/phase-1/changelog.md`, and a P1-M29 task with a verifiable checklist and the owner decision line, added before any code (AGENTS.md).
