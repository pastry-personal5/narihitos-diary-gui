# P1-M27: Notion export overview

Status: Delivered (2026-11-07)

Register a real Notion adapter behind the P1-M25 export coordination, projecting the saved Authentic Markdown into year/month pages and one ascending daily toggle section per date, with bounded chunking, service-limit handling and honest partial or ambiguous-write recovery. The [work plan](milestone-27-notion-export-architecture.md) defines stages and scenarios. This plan does not certify runtime behavior until the evidence below exists.

## Entry conditions

P1-M25 and P1-M26 are delivered ([P1-M25](../archive/milestones/p1-m25/milestone-25-export-coordination-overview.md), [P1-M26](milestone-26-icloud-export-overview.md)). Follow [Notion hierarchy, projection and recovery](../architecture/configuration-and-exports.md#notion-hierarchy-projection-and-recovery), [Export input and shared reconciliation](../architecture/configuration-and-exports.md#export-input-and-shared-reconciliation), [UX](../ux-gui.md) export status, and [handoff](implementation-handoff.md) E8.

## Owner decisions (2026-11-07)

- **O1 - Evidence:** close on a fake-HTTP matrix, an agent-run env-gated live round trip and the full gate. No human gate. Production Notion export stayed unregistered until the live round trip passed (2026-11-07); the opt-in gate has been removed.
- **O2 - TLS dependency:** approved enabling the `native-tls` feature on the existing pinned `reqwest` 0.13.5 (macOS Security.framework), with license-notice review before merge. No other dependency. Any further feature (for example `blocking`, `query`) needs a new owner approval.
- **O3 - Write scope:** create new daily sections and replace an already-exported day through checkpointed delete/append steps, with partial or `uncertain` outcomes reported honestly.
- **O4 - UI:** a separate Notion toolbar button and `export.start.notion` palette action beside the iCloud one; both reuse the existing dialog, confirmation, cancel and abandon flows.
- **O5 - Checkpoints:** one additive migration adds a `notion_steps` table (adapter-internal, keyed by operation ID). Operation `checkpoint` stays a small state label.
- **O6 - Live test authorization:** the owner authorizes real writes to a dedicated throwaway Notion root page supplied through `DIARY_NOTION_LIVE=1`, `DIARY_NOTION_LIVE_TOKEN` and `DIARY_NOTION_LIVE_ROOT`. The test leaves the pages it creates for the owner to inspect and delete; neither the test nor the app archives or deletes Notion pages.
- **O7 - Outcome surfacing:** projection warnings and a throttle wait beyond the 60 s budget appear as Global Messages only, not in the confirmation preview.
- **O8 - Page order:** year and month pages are created on demand and Notion cannot position a new child page, so ascending order is enforced and verified for daily sections inside a month page only. A pre-existing duplicate year, month or date remains a structural conflict. The architecture wording changes to match (stage D).
- **O9 - Destination key (review, 2026-11-07):** the key is the root page ID plus the Notion `bot.workspace_id`, fetched lazily. The adapter is registered from valid config but reports `available=false` with a typed reason until `GET /v1/users/me` succeeds; `root()` returns `<root_id>@<workspace_id>` only once known, so no export starts, and no receipt is written, under a guessed namespace.
- **O10 - Replace strategy (review, 2026-11-07):** replace inserts a new toggle for the date directly after the old one, verifies it, then deletes the old toggle. Old text survives until the new section is verified. The daily block ID therefore changes on a replace (stable only between replaces); a crash can leave two same-date sections, reported `uncertain`. This supersedes the in-place delete/append wording in O3 and the architecture note (stage D updates it). Notion's block delete moves the block to trash; the app never deletes or archives pages.
- **O11 - Dependencies (review, 2026-11-07):** `diary-infrastructure` gains `reqwest` (`json`, `native-tls`; the unused `stream` feature is dropped) and `tokio` as normal dependencies, and `axum` as a dev-dependency for the loopback fake. All are already pinned in the lockfile; no new crate. Any further feature still needs approval.
- **Revision (owner, 2026-11-07, P1-M29 T13-T16):** a day is a `heading_2` titled `YYYY-MM-DD (WED)` plus its following sibling blocks instead of a daily toggle; read "toggle" in O3, O8, O10, O12 and Q4 as "heading range". O10 still holds: insert the new range after the old one, verify it, then delete the old range (heading last). Old toggle days are not migrated. See the [heading-day spec](../superpowers/specs/2026-11-07-notion-heading-days-design.md).
- **O12 - Foreign blocks (review, 2026-11-07):** if a daily section (or the old section during replace) contains any block type outside the projection's output set (for example `child_page` or `child_database`), the adapter returns a typed structural conflict and writes nothing. Edited text blocks still follow the Changed confirmation flow.

## Scope

- Stage A: revalidated API facts and an offline projection (Markdown to Notion blocks) plus the HTTP client with request queue, retry policy and typed errors, against a loopback fake.
- Stage B: hierarchy resolution and the adapter (`observe`, `apply`, `read_back`) with `notion_steps` checkpoints, chunking, create and replace, 503 committed-ID reconciliation and restart recovery.
- Stage C: wiring (config to adapter, `exports_available` per target), separate Notion action in the UI, Global Messages for warnings and throttling.
- Stage D: fake-HTTP matrix, `scripts/smoke/export.py` Notion legs, the live round trip, full gate, docs.

Out of scope: image or attachment upload, remote image fetch, clearing a destination, archiving Notion pages, any claim of atomic or compare-and-swap replacement, a user-configurable API base URL, new prompts or adapters, packaged-app proof (P1-M32).

## Open items

- **Q1 - Workspace identity:** `GET /v1/users/me` documents `bot.workspace_id`, but the documented example omits it. Stage A fixtures assume it; the live test confirms. If absent, the destination key falls back to the bot user ID and the work plan records the change (a bot user ID is per integration, so this narrows the namespace and is acceptable only because a changed key never reuses receipts).
- **Q2 - Rate budget:** the documented average request rate is revalidated in stage A and sets the queue pacing; the work plan does not fix a number.
- **Q3 - Reservation key (answered in review):** `Service::key()` hashes target, `adapter.root()` and the Diary workspace ID, not the Notion workspace. O9 therefore puts the Notion workspace into `root()`. One reservation then serializes every Notion export to that root. Stage A tests it; no extra lock is added beyond the in-process hierarchy lock.
- **Q4 - Nesting:** confirm in the live test how a toggle's body nests within the two-level per-request limit; deeper Markdown nesting becomes literal text with a warning.

## Completion checklist

Check only when evidence exists.

### Plan review

- [x] R01 - Owner answered O1-O8 (recorded above). Evidence: this document; changelog and AGENTS.md entries added.
- [x] R02a - Plan reviewed against current code on 2026-11-07; corrections and O9-O12 recorded in the work plan and here. Evidence: this document and the work plan "Plan review corrections".
- [x] R02b - Stage A step 0 re-read the code touched by the port changes (`apply`/`key()` call sites, cancel, the migration list) and the migration mechanism; corrections are recorded in the [work plan](milestone-27-notion-export-architecture.md#implementation-notes-and-deviations-2026-11-07). Evidence: the service held one adapter and compared every remote digest to the snapshot SHA; both were fixed (per-target adapters, `digest_of`, `ready()`).

### Stage A - client and projection

- [x] C01 - Markdown projects deterministically to supported blocks; unsupported Markdown becomes literal text with a counted warning; scalar-safe 2,000-character rich-text splitting (T01). Evidence: `diary-domain` `notion::tests` (7 tests: block order, determinism, literal text with counted warnings, depth restructuring, scalar-safe splitting, oversize continuation, chunking).
- [x] C02 - Chunks respect 100 children, nesting, 1000 blocks and the 400 KB encoded budget, with per-chunk digests (T02). Evidence: `chunks_respect_count_blocks_and_bytes`; `long_unicode_entries_chunk_and_round_trip` (251 blocks need three requests, digest round trip).
- [x] C03 - Client reads `Retry-After` (header or `additional_data.retry_after`), retries only reads and explicitly rejected 429/529 within five attempts and 60 s, never retries a write after a timeout or 5xx, and logs no body or token (T03). Evidence: `a_short_throttle_waits_and_retries_but_a_long_one_stops_cleanly`, `server_errors_on_writes_are_not_retried_but_reads_are`, `the_client_reports_the_wait_and_never_exposes_bodies`, `credential_and_network_problems_are_typed`. The client logs nothing and the transport marks the credential header sensitive.
- [x] C04 - `native-tls` is enabled on `reqwest` in `diary-server` with notice review; the API host is a constant with no override, and the in-memory fake is injected through the `Http` port only in tests and the `fault-injection` build (T04). Evidence: `scripts/collect-notices.py` regenerated `assets/third-party` (9 new license files, 14 new packages, all MIT and/or Apache-2.0; only `security-framework` and siblings compile on macOS).

### Stage B - adapter

- [x] C05 - Paginated hierarchy read finds unique year, month and date targets by recorded IDs; duplicates or disordered dailies are structural conflicts (T05). Evidence: `pagination_reads_every_page_of_a_long_month`, `duplicate_and_disordered_hierarchy_is_a_structural_conflict`.
- [x] C06 - Concurrent year and month creation never yields duplicates (T06). Evidence: `concurrent_exports_never_create_duplicate_year_or_month_pages`.
- [x] C07 - Backdated first, middle and last daily insertion uses `position`; later chunks follow the last verified child (T07). Evidence: `daily_sections_stay_ascending_for_first_middle_and_last_insertion`.
- [x] C08 - Replace inserts a new section after the old one, verifies its content and order, then deletes only the old section covered by the authorized observation; a crash at any step leaves a recorded, reconcilable state (T08). Evidence: `replace_inserts_a_new_section_after_the_old_one_then_removes_the_old_one`, `a_crash_between_the_new_section_and_the_old_delete_keeps_the_old_text`, `an_external_edit_between_observe_and_apply_fails_without_writing`; service test `an_external_edit_needs_confirmation_before_replacement`.
- [x] C08b - A section holding foreign block types, or a month page with an unrecorded duplicate date, is a typed structural conflict with no write (T05, T08). Evidence: `a_foreign_block_in_a_section_is_a_structural_conflict_with_no_write`; duplicate-date coverage in `duplicate_and_disordered_hierarchy_is_a_structural_conflict`.
- [x] C09 - Lost replies, dropped requests and 5xx on writes end `uncertain` and are never replayed; the service proves success only by read-back (T09). Notion documents no committed-ID format, so no ID adoption exists. Evidence: `a_lost_reply_is_unknown_never_replayed_and_the_read_back_sees_the_commit`, `server_errors_on_writes_are_not_retried_but_reads_are`; service tests `a_lost_reply_after_the_last_chunk_is_proven_by_read_back`, `a_lost_reply_mid_write_parks_uncertain_and_is_never_replayed`. Identical external appends are not adopted: any duplicate is a structural conflict.
- [x] C10 - A kill after the Notion write is proven by read-back on restart and `apply` is never called again (T10). Kill at every individual request boundary is not separately exercised. Evidence: service test `a_lost_reply_mid_write_parks_uncertain_and_is_never_replayed` (restart recovery writes nothing); `scripts/smoke/export.py` leg 6 kills after the write and recovers.

### Stage C - wiring and UI

- [x] C11 - Adapter registered only with valid `notion` config; unavailable with a typed reason until workspace identity is fetched; `exports_available` is per target (T11). Evidence: `both_targets_are_available_and_notion_creates_then_verifies_a_noop`, `workspace_identity_is_fetched_lazily_and_gates_the_destination_key`, `a_malformed_root_page_id_registers_nothing`. The temporary `DIARY_NOTION_EXPERIMENTAL` opt-in was removed after C14 passed (O1).
- [x] C12 - Notion button and `export.start.notion` action with accessible names, keyboard path and reasons; warnings and failures reach Global Messages with app-authored text (T12). A throttle message says to retry in a few minutes; the exact wait is not carried. Evidence: `tests/export.test.ts` (`Notion export`: target, destination wording, warning notice, five typed failures, recorded failure reason, uncertain wording), palette and shortcut-table tests in `tests/settings-ui.test.tsx`, `tests/shortcuts.test.ts`.

### Stage D - proof and docs

- [x] C13 - `python3 scripts/smoke/export.py` Notion legs pass on the fault-injection server against the file-backed in-memory fake service. Evidence: `python3 scripts/smoke/export.py` PASS (leg 6: hierarchy, verified no-op, external edit confirmation, replace with a new section, kill and recovery, token absent from logs).
- [x] C14 - Live round trip passes (create, no-op, backdated insert, replace, long Unicode, restart) and records actual service behavior for Q1, Q4 and page ordering. Evidence: owner-run `scripts/smoke/notion-live.py` against a throwaway root page passed on 2026-11-07 (pasted output): `bot.workspace_id` present (Q1); daily sections ascending `2026-11-03, 11-07, 11-10, 11-20` after inserting 11-10, 11-03, 11-07, 11-20; a nested list item arrives as a child of its bullet, section children `paragraph, bulleted_list_item (has children), quote` (Q4); long Unicode entry, external edit then confirmed replace and restart no-op passed; page creation rejects a `position` (`page_position_accepted: False`), which confirms O8. Earlier attempts failed on an empty token and on the script's missing session heartbeat, not on the app.
- [x] C15 - Full gate passes (`scripts/gate.sh`). Evidence: `scripts/gate.sh` G01-G07 all exited 0 on 2026-11-07 after the opt-in gate was removed (733 Rust tests passed, 16 ignored env-gated live tests; 349 frontend tests).
- [x] C16 - Architecture note (O8), handoff E8, phase plan, roadmap, changelog, docs index, README (user-visible) and AGENTS.md reflect delivered behavior and limits. Evidence: Architecture contract (O8 order wording, registration note), handoff E8, phase plan, roadmap, changelog, docs index, README and AGENTS.md updated; the work plan records every deviation. The entries say the live proof is pending.
