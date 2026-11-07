# P1-M27: Notion export work plan

Status: Delivered (2026-11-07), plan reviewed (2026-11-07)

Stages and scenarios for the [overview](milestone-27-notion-export-overview.md). The behavioral contract is [Notion hierarchy, projection and recovery](../architecture/configuration-and-exports.md#notion-hierarchy-projection-and-recovery); this plan only sequences it and records the facts revalidated on 2026-11-07.

## Revalidated service facts (2026-11-07)

From the Notion reference (append block children, request limits, users). Re-check the live behavior in the live test.

- Append: at most 100 children per request, up to two nesting levels per request, `position` of `start`, `end` or `after_block`; existing blocks cannot be moved.
- Payload: at most 1000 block elements and 500 KB; rich-text `content` at most 2,000 characters; arrays at most 100 elements; violations return 400 `validation_error`.
- Throttling: 429 `rate_limited` and 529 `service_overload` are retryable; `Retry-After` is whole seconds, repeated as `additional_data.retry_after`.
- Notion's own guidance retries 5xx for GET and DELETE. This plan is stricter: retry only reads and explicitly rejected (429/529) requests; never retry a POST, PATCH or DELETE after a timeout or 5xx.
- No documented `position` for creating a child page: year and month pages append at the end (overview O8).
- `GET /v1/users/me` returns `bot.workspace_id` (Q1).
- `reqwest` 0.13.5 offers `native-tls` (OS TLS, ALPN included); `default-tls` is rustls, which this project does not enable.

## Existing code to reuse

- `diary-application::export`: `ExportAdapter`, `Observation`, `Write`, `Applied`, decision table, confirmations, `cancel`, `abandon_uncertain`. `Target::Notion` already exists.
- `diary-infrastructure/src/export/{store,fake,icloud}.rs`: ledger, reservations and the iCloud adapter are the behavioral reference; the Notion adapter passes the same scenarios.
- Config: `notion.root_page_id`, `notion.token` already parse and reload (`config.rs`, `config/reload.rs`).
- `diary-domain` already depends on `pulldown-cmark` (P1-M24); reuse it for projection.
- API and UI: `target` already flows through `POST /exports`; reuse `useExport`, `ExportDialog`, `export/controller.ts` and Global Messages.
- Calls to the adapter already run inside `tokio::task::spawn_blocking`, so a captured `tokio::runtime::Handle::block_on` drives the async `reqwest` client. No `blocking` feature is needed. `diary-infrastructure` has no `tokio`, `reqwest` or `axum` edge today (O11 adds them). Handle::block_on panics on a runtime thread, so a test asserts the adapter is only reached from blocking threads.

## Port changes

Keep the port minimal; record each change here.

1. `apply` gains the operation ID so the adapter can key `notion_steps`. The iCloud adapter and fake ignore it.
2. The adapter reports a projection warning count through the existing operation reason or checkpoint label so the UI can raise a Global Message; no new endpoint. The exact carrier is settled in stage C and recorded here.
3. Cancel: the port has no cancel signal, yet cancel must stop at a step boundary. `apply` also receives a cheap `&CancelFlag` (an `AtomicBool` wrapper owned by the service); the iCloud adapter and fake ignore it.
4. Notes carrier (settled in review, replaces "settled in stage C"): a new defaulted port method `notes(&self, op: &OperationId) -> Notes` returns content-free counts (`projection_warnings`, `throttle_wait_secs`). The service reads it after `apply` or on a failed throttle and raises the Global Messages of O7. The iCloud adapter and fake return `Notes::default()`. No new endpoint.
5. Availability: `root()` stays synchronous and infallible. Until workspace identity is fetched (O9) the adapter returns `available=false` from `observe` with a typed `Unavailable` reason (`identity_pending`, or `permission_denied`/`unauthorized` from the failed call) and `root()` returns `<root_id>@pending`, which `Service::key()` must never persist: the service refuses to create an intent when `observe` is unavailable, so the placeholder key is unreachable. Stage A tests that.
6. `Observation.ids` carries the daily block ID plus child IDs as an allowlisted, bounded encoding. `fingerprint` hashes the remote section's normalized children and order; `digest` hashes the projected content for read-back comparison.

## Module plan

| Layer          | File                                                      | Responsibility                                                                                      |
| -------------- | --------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| domain         | `export/notion_blocks.rs` (or equivalent)                 | Pure, deterministic Markdown to blocks projection, warnings, chunking, digests                      |
| infrastructure | `export/notion/{client,queue,hierarchy,steps,adapter}.rs` | HTTP client, credential-scoped queue and retry, hierarchy resolution, `notion_steps` store, adapter |
| infrastructure | `Cargo.toml`                                              | Add `reqwest` (`json`, `native-tls`), `tokio`; dev-dep `axum` (O11); drop unused `stream`           |
| infrastructure | migration                                                 | Additive `notion_steps` table (`0007_notion_steps.sql`)                                             |
| server         | `diary-server/src/main.rs`                                | Register adapter from `notion` config; base URL fixed to `https://api.notion.com` outside tests     |
| bridge/ui      | `bridge/export.rs`, `src/components`, `registry.json`     | `export.start.notion` action and toolbar button; per-target availability; Global Messages           |
| scripts        | `scripts/smoke/export.py`, `scripts/smoke/notion-live.py` | Loopback-fake legs; env-gated live round trip                                                       |

## Adapter design

- **Auth and headers:** `Authorization: Bearer <token>` and `Notion-Version: 2026-03-11`. The token never reaches argv, logs, errors or traces; failures return allowlisted typed codes only.
- **Destination key (O9):** root page ID plus `bot.workspace_id`, fetched lazily and cached for the adapter's life; a changed value is a new namespace. Unknown identity means unavailable, never a guessed key. A failed fetch retries on the next `observe`, bounded by the queue rules.
- **Request queue:** one queue per credential serializes requests and applies pacing from the revalidated rate (Q2). A pause from `Retry-After` blocks the whole queue. Retry limit: five attempts and 60 s, jittered. A longer requested delay ends the export as retryable-failed with a typed throttle reason and the wait, with no early retry; if a write could have committed, the export is `uncertain` instead.
- **Hierarchy:** paginate the root, year and month children. Titles identify candidates only; receipts record page and block IDs. Duplicate year, month or date, or disordered dailies, is a structural conflict. The only tolerated same-date pair is the recorded old and new section of an in-flight replace (identified by `notion_steps` block IDs, never by title). Create a missing year page, then month page, by `POST /v1/pages` with the parent page ID, persisting intent first and reconciling after any lost reply.
- **Daily section:** a toggle block titled with the ISO date inside the month page. Insert with `position` (`start` if earliest, otherwise `after_block` of the previous dated section). Body blocks go under the toggle in chunks; each later chunk uses `after_block` of the last verified child. _(Superseded 2026-11-07: a day is now a heading range; see P1-M29 T13-T16.)_
- **Projection:** paragraphs, headings, lists, quotes and code map to supported blocks. Anything else, and nesting beyond two levels per request, becomes literal text with a counted warning. No remote image fetch or upload. Rich text splits on Unicode scalar boundaries at 2,000 characters.
- **Chunking:** at most 100 children, 400 KB encoded (measured after serialization, including wrapper and Unicode bytes) and the 1000-block cap, whichever is hit first. Each chunk has a digest stored with its step.
- **Checkpointing:** before each request write a `notion_steps` row (operation ID, step index, kind, parent, anchor, normalized content and chunk digest, prior observation hash). After the response read back IDs and content, then mark the step. A 503 that returns committed IDs stores only the allowlisted IDs and reconciles those objects.
- **Foreign blocks (O12):** before any write, classify the daily section's children. Any type outside the projection output set (including `child_page`, `child_database`, synced or embedded blocks) is a typed structural conflict with no write.
- **Create:** absent daily section, then insert and verify all children in order. **Replace (O10):** record the old section's block ID, child IDs and fingerprint; insert a new toggle with `after_block` set to the old one; append and verify its children in chunks; verify complete content and order; only then delete the old toggle (the block and its children, only if it still matches the authorized observation) and write the receipt with the new block ID. Crash cases: before the new toggle exists nothing changed; with both present the old section is intact and the state is `uncertain` until read-back shows the new section complete (then finish by deleting the old one only with step provenance) or the user abandons; after the old one is deleted, read-back proves success. A partial replace is reported as `uncertain` and resolves by read-back match or confirmed abandon (M26 flow). Notion delete moves blocks to trash, which keeps the removed content recoverable by the user. _(Superseded 2026-11-07: a day is now a heading range; see P1-M29 T13-T16.)_
- **Unknown outcomes:** a timeout, lost reply or 5xx on a write returns `Unknown`. The service reads back and never replays. Zero matches without definitive non-commit evidence stays `uncertain`, including after restart. Identical external content alone never authorizes cleanup.
- **Concurrency:** the existing `export:<destination key>` reservation serializes exports to one root (Q3). An in-process lock around the hierarchy phase protects non-export reads. Notion has no compare-and-swap; say so in the docs and test observed drift detection instead.
- **Test seam:** the base URL is an adapter constructor argument. Production wiring passes the constant; only tests and the `fault-injection` build pass a loopback URL. It is never a config field, so a token cannot be sent to an arbitrary host.

## Recovery design

- Restart: for each unfinished Notion export, load its `notion_steps`, read back the hierarchy and daily section, then resolve to `Succeeded`, re-ask confirmation (write never began) or `uncertain` (write began, unproven). `apply` is never called on recovery.
- Cancel before `apply` stops cleanly; during `apply` the adapter checks the cancel flag (port change 3) at each step boundary and the service reports the read-back outcome. Once the old section's delete has begun, cancel is ignored until that step ends. No remote rollback.
- Terminal states delete the stored snapshot; `notion_steps` rows are deleted at terminal success, kept (content-free) until abandon for `uncertain`.

## Fake HTTP matrix (loopback Axum, no new dev-dependency)

A fake Notion server in the infrastructure test support holds pages and blocks in memory with injectable faults. Scenarios: paginated duplicate and disordered hierarchy; foreign blocks in a section (O12); identity fetch failure then success (O9); token and body absent from every log, error and trace; replace crash before, between and after the new section and the old delete (O10); concurrent year and month creation; backdated first, middle and last insertion; long Unicode entries over 2,000 characters and across chunk limits; 429 and 529 with short and over-budget `Retry-After`; deleted anchor; per-chunk failure; delayed commit after an empty read; 503 with committed IDs; identical external append; ambiguous duplicates; receipt loss; duplicate and expired confirmation recovery; kill at each step boundary.

## Stages and tasks

Each task is test-first.

### Stage A - client and projection (T01-T04)

0. Review step (R02b): re-read the `apply`/cancel call sites and the migration mechanism; record corrections below. Add the O11 dependency edges and `native-tls`, run the notice process, confirm the build and `scripts/gate.sh` lanes still pass.
1. T01 projection determinism, warnings, scalar-safe splitting.
2. T02 chunking limits and digests.
3. T03 client retry and queue rules, typed errors, redaction.
4. T04 base-URL override exists only in tests and the `fault-injection` build.

### Stage B - adapter (T05-T10)

5. T05 hierarchy read and structural conflicts.
6. T06 concurrent creation.
7. T07 insertion positions.
8. T08 insert-then-delete replace, foreign blocks and drift detection.
9. T09 ambiguous writes and provenance.
10. T10 restart and recovery.

### Stage C - wiring and UI (T11-T12)

11. T11 registration and per-target availability.
12. T12 component tests: Notion button and action, reasons, accessible names, Global Messages for warnings and throttle waits, SF Symbols through the existing host (add an allowlist entry only if the existing export symbol is not reused).

### Stage D - proof and docs

13. Extend `scripts/smoke/export.py` with Notion legs on the fault-injection server and the loopback fake.
14. Add `scripts/smoke/notion-live.py`, run only with `DIARY_NOTION_LIVE=1`: create, no-op, backdated insert, replace, long Unicode, restart. It records service behavior for Q1, Q4 and page ordering and never deletes pages. Enable production registration only after it passes.
15. Run the full gate once. Update docs per C16, including the O8 wording change in the Notion architecture section.

## Risks and notes

- A replace is not atomic; a crash mid-replace can leave two same-date sections. The old one stays intact until the new one is verified; the design reports the state, never hides it.
- The Notion token is a plain-text credential in private YAML (existing decision). Do not log it, and keep it out of traces and error bodies.
- Fake-HTTP evidence proves logic only; real limits and the `workspace_id` field are proven by the live test. A live failure keeps export disabled and is reported to the owner, not worked around.
- A throttle that outlasts the 60 s budget ends the export early; the user retries later. No background retry queue is added.

## Plan review corrections (2026-11-07)

- **Reservation key:** the plan assumed the key included Notion's workspace; `Service::key()` uses the Diary workspace ID and `adapter.root()`. Fixed by O9 (identity folded into `root()`, lazy and gated by availability).
- **Dependencies:** `reqwest` is pinned in the workspace but used by no crate; `diary-infrastructure` lacks `tokio`, `reqwest` and `axum`. The earlier "no new dev-dependency" claim was wrong. O11 records the edges; the unused `stream` feature is removed.
- **Port gaps:** cancel at step boundaries and the warning/throttle carrier were missing; port changes 3 and 4 settle them. Added the unavailable-until-identity rule (change 5).
- **Replace safety:** delete-then-append could lose the old text on a crash; replaced by O10. The hierarchy validator now tolerates exactly one recorded in-flight duplicate pair.
- **Foreign content:** replace could trash nested pages; O12 makes that a structural conflict.
- **Stage D doc changes:** architecture note `Notion hierarchy, projection and recovery` (O8 order wording, O10 replace, stable anchor only between replaces), the coordination paragraph that says Notion remains disabled, `AGENTS.md` P1-M27 entry, handoff E8.
- **Unverified:** the Notion limits and `Notion-Version: 2026-03-11` are carried from the earlier research; only the live test proves them.

## Implementation notes and deviations (2026-11-07)

Where the built code differs from the plan above, and why.

- **Dependencies (O11, narrower than planned):** `diary-infrastructure` gained no `reqwest`, `tokio` or `axum` edge. The adapter talks to a synchronous `Http` port (`export/notion/http.rs`); the `reqwest` (`native-tls` only) transport lives in `diary-server/src/notion_http.rs` and runs on the blocking threads the service already uses (`Handle::block_on`). The only manifest change is `reqwest` in `diary-server`; the lockfile gained 14 packages (`native-tls`, `hyper-tls`, `tokio-native-tls`, `security-framework(-sys)`, `openssl*`, `schannel`, `foreign-types*`, `openssl-probe`, `rustls-pki-types`, `zeroize`), of which only the macOS ones compile on this platform. The fake is an in-memory `FakeNotion` behind the same port, not a loopback Axum server; an optional state file lets the smoke script restart the server and edit the "remote".
- **No base-URL override:** the host is a constant. Tests and the `fault-injection` build swap the whole transport instead (`DIARY_FAKE_NOTION_FILE`).
- **Port (changes 1, 3, 4, 5, 6):** `apply` is unchanged and carries no operation ID or cancel flag. Added defaulted methods `ready()` (O9: the service builds no key and no intent until the workspace identity is known, so the `@pending` root is unreachable), `digest_of()` (the remote of a projection never hashes equal to the snapshot file) and `warnings()`. `notion_steps` rows are keyed by destination and date and cleared when the next write for that date begins, because the reservation allows one export per root. Cancel stops before `apply`, as for iCloud; once `apply` starts the export finishes and reports by read-back. There is no step-boundary cancel flag.
- **Notes carrier:** the warning count travels as the bounded reason tag `projection_warnings` on a succeeded export (`finish_export` keeps that tag only on success); a throttle beyond the budget is the typed failure `throttled`, with no wait value.
- **New typed reasons:** `unauthorized`, `throttled`, `network` and `structural_conflict` join `Unavailable`, with API codes `export_unauthorized`, `export_throttled`, `export_network` and `export_structural`, all allowlisted in the bridge.
- **Replace (O10):** implemented as planned. A same-date pair left by a crash between the new section and the old delete is not tolerated: it reads as a structural conflict and needs manual repair in Notion (the old text is intact). The planned recorded-pair tolerance and finishing the delete with step provenance are not built.
- **Foreign blocks (O12):** any child of a daily section outside the output set (and unknown types at any depth) is a structural conflict with no write.
- **Committed IDs on 503:** Notion documents no such field, so none is parsed; ambiguity is resolved by read-back only.
- **Startup:** no request is made at launch; the identity is fetched on first use. An unreachable service leaves the destination unavailable with a typed reason until it answers.
- **Production gate (O1):** the real transport was registered only with `DIARY_NOTION_EXPERIMENTAL=1` until the live round trip passed on 2026-11-07; the gate has since been removed and Notion registers whenever `notion` is configured.
- **Not built or not exercised:** kill at each request boundary, committed-ID reconciliation, and the `export.start` availability flag in the TypeScript settings DTO (`notion_exports_available` is served and typed in the bridge but the UI shows `export_disabled` text, as for iCloud).
