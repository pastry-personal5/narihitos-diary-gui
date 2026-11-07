# Notion export: Heading 2 days instead of daily toggles

Status: Draft for owner review (2026-11-07)

## Intent

The owner wants the Notion export to show each day as a **Heading 2** titled `YYYY-MM-DD (WED)` followed by **ordinary paragraph blocks**, with no daily toggle (owner, 2026-11-07, quoting the request: "No Daily toggle title. Heading 2 title. Then, normal text."). This changes the shipped P1-M27 adapter. Year (`YYYY`) and month (`YYYY-MM`) pages, the Markdown-to-blocks projection, chunking, the HTTP client and the confirmation, cancel and abandon flows stay as they are.

Success: a new export, a backdated export and a replace each produce the new shape in a fake-HTTP matrix and in the env-gated live round trip, with the P1-M27 recovery guarantees intact (old text survives until the new section is verified; partial or ambiguous writes are reported honestly).

Owner decisions taken while scoping: heading range with the same safety model (approach 1); **no migration** of old toggle days (a leftover dated toggle is a structural conflict repaired by hand).

## Assumptions to confirm

- The weekday is the English three-letter uppercase form `MON`..`SUN`, computed from the date with `diary_domain::calendar::weekday` (Sunday is 0).
- Notion's append `position` parameter (already used by `wire::append`) supports inserting after any block of the month page, not only after a toggle.

## Notion shape

```
YYYY (page)
  YYYY-MM (page)
    ## 2026-11-03 (TUE)      heading_2  - day boundary
    paragraph ...
    ## 2026-11-04 (WED)
    paragraph ...
```

A **day** is a `heading_2` whose text starts with a valid ISO date, plus every following sibling up to the next such heading or the end of the page. Blocks before the first day heading are not part of any day and are never read as one or modified. The weekday suffix is written but not required on read, so a hand-edited suffix does not change which day a heading is. Ordering is verified by the ISO date.

### Body headings

A day heading must always mean "a new day", so projected body headings can no longer be `heading_2`. `diary-domain::notion` collapses every Markdown heading level to `heading_3` (today: `Heading(n.min(3))`, which keeps levels 1 and 2). This is silent for levels 1-3 (a lossy level collapse, documented in the architecture text); levels 4-6 keep their existing warning. The change is local to the Notion projection and does not touch the iCloud renderer.

## Adapter changes (`diary-infrastructure/src/export/notion/`)

- `wire.rs`: `toggle(title)` becomes `heading(title)` (a `heading_2`). `new_toggle` becomes `new_heading`: the heading titled `title` in an append reply whose ID is not the old one. `Section::fingerprint` takes the heading ID instead of the toggle ID and still hashes every body block's ID, type, text and order. No new receipt column is needed; `Observation.ids` carries the heading ID.
- `look` (`adapter.rs`): list the month page children once and partition them into day ranges. `Located.dated` stores `(iso, heading_id)` and `Located.section` carries the target day's range. Strictly ascending dates still reject duplicates and disorder as structural conflicts. A `toggle` whose text is a valid ISO date anywhere in the month page is a structural conflict (legacy shape, no migration). Anything inside the target range outside the projection's output set stays a structural conflict (O12), including a stray `toggle`.
- `read_blocks` is reused for nested children. A new `read_range(children_slice)` builds the day's `Section` from the already listed siblings, so a month page is listed once per operation.
- `Run::section` becomes `Run::heading`. Insertion anchor:
  - first write of a date: the block immediately before the first day heading with a later date, or the last child of the page if none, or `position: start` if the later heading is the page's first child;
  - replace: directly after the **last block of the old range** (not the old heading), so the new range sits between the old range and the next day.
- Chunk append now targets the **month page**, positioned after the previous chunk's last block (the heading for the first chunk). Reply parsing must not trust list position (the toggle code already avoids this). New block IDs are the reply entries whose IDs were not known before the request, and the chunk's last ID is the last of those. `notion_steps` keeps its rows; the `append` step's `parent` field now holds the month page ID.
- `go` (replace): after the new range is complete it is read back and compared to the projection digest, then the old range is deleted block by block (heading last), each step checkpointed as today. A crash between insert and delete leaves two ranges for the same date, reported `uncertain` and found as a structural conflict by the next `look`. This keeps O10's safety property unchanged.
- Cost: deleting a range is N delete requests where it used to be one toggle delete. Acceptable for diary-sized days; the 60 s throttle budget and cancel behavior are unchanged.

## Out of scope

Migrating or converting old toggle days; reading both formats; a configurable title format or weekday language; changes to year or month page titles; any new dependency or database migration.

## Open items

- **Q1 - Append reply content:** confirm in the live test what `PATCH /v1/blocks/{page}/children` with `position` returns for a page that already has many children (new blocks only, or the whole child list). The ID logic above works either way, but the live run must record which.
- **Q2 - Range delete order:** deleting the heading last keeps a partial delete visible as a headed range; the live test confirms Notion accepts deleting a block whose previous sibling was already trashed.

## Testing and evidence

- `diary-domain`: weekday title format; body headings of levels 1-3 become `heading_3`; round trip digest unchanged.
- Fake-HTTP matrix (`export/notion/tests.rs`, `fake.rs` gains an insert-after-block mode if it lacks one): create into an empty month; create after an earlier day; backdated create before a later day; first-child backdate (`position: start`); replace; crash after the new heading is created; crash between insert and delete; lost reply on the heading append; foreign block in a range; legacy dated toggle; duplicate day headings; edited body text (Changed flow).
- `scripts/smoke/export.py` Notion legs and `scripts/smoke/notion-live.py` updated for the new shape; the live round trip stays owner-run with the existing `DIARY_NOTION_LIVE*` variables.
- Full gate: `scripts/gate.sh`.

## Docs and records

- `docs/architecture/configuration-and-exports.md` (Notion paragraph) and the P1-M27 overview/work plan: replace "daily toggle section" with the heading range. Record in the P1-M27 overview that O3, O10 and O12 stand with a wording change (toggle becomes heading range) and that O8's ordering rule now applies to day headings.
- New follow-up entry in the P1-M29 owner-requested revisions overview with a verifiable checklist (added before any code, per AGENTS.md): domain tests, adapter tests, smoke legs, live round trip, docs, gate.
- `docs/phase-1/changelog.md` entry. No `ux-terms.md` change: its only "toggle" hit is the Snippet checkbox, unrelated to Notion.
