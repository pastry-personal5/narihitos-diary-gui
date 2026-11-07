# P1-M24: Paragraph review and acceptance work plan

Status: Done

Implementation stages and acceptance scenarios for the [overview](milestone-24-paragraph-review-overview.md). The behavioral contract is [Paragraph alignment and review](../../../architecture/editing-and-agents.md#paragraph-alignment-and-review); this plan only sequences it and fixes the module boundaries. Owner decisions O1–O3 are in the overview.

## Existing code to reuse

- `diary-application::authentic`: `AuthenticStore::{load, save}`, `SaveRequest{operation_id, date, expected, body}`, `resource_key(date)`. Accepts are ordinary saves.
- `diary-application::operations`: ledger `Record.result`, `OperationLedger`, `classify`; lost replies resolve through `AuthenticStore::operation`.
- `diary-application::generation`: `Store`, `Generation.candidates`, `Frozen::baseline()`, `Store::discard`; `stale_baseline` is already computed in `diary-api/src/generations.rs`.
- `diary-api`: per-feature route modules (`generations.rs`, `authentic.rs`) and session/lease admission; `src-tauri/src/bridge/*` for typed commands; SSE kinds already list `review_invalidated`.
- Client: `src/application/generation/*`, `activity/*`, Authentic pane and CodeMirror module; save/dirty guards in `navigation`.

## Module plan

| Layer          | New file                                                                            | Responsibility                                                                                                         |
| -------------- | ----------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| domain         | `diary-domain/src/review.rs` (+ `review/tests.rs`)                                  | `units`, `align`, `compose`, `Hunk`, `Unit`, `ReviewError{Unsupported,Limit}`; pure, no I/O                            |
| application    | `diary-application/src/review.rs` (+ `review/tests.rs`)                             | `ReviewStore`, `Review`, `start`, `decide`, `end`, `invalidate`, accepted-set digest; uses `AuthenticStore` and ledger |
| infrastructure | `diary-infrastructure/src/authentic.rs` (modify)                                    | Persist the accept receipt (hunk, revision, before/after digests) with the save receipt in one transaction             |
| api            | `diary-api/src/reviews.rs` (+ tests)                                                | Three routes, body limits, error mapping, `review_invalidated` publication                                             |
| bridge         | `src-tauri/src/bridge/review.rs`                                                    | `start_review`, `decide_review`, `end_review` with capability grants                                                   |
| client         | `src/application/review/{contracts,controller}.ts`, `src/components/ReviewPane.tsx` | Review state machine, validation, per-hunk UI, editor read-only compartment                                            |

## Interfaces (decided here so stages agree)

- Domain: `Hasher` trait (`fn sha256_hex(&self, bytes:&[u8])->String`, implemented in infrastructure, O2); `units(text:&str)->Result<Vec<Unit>,ReviewError>` with `Unit{kind:UnitKind, range:Range<usize>}`; `align(h:&dyn Hasher, b:&[Unit],c:&[Unit], b_text, c_text)->Result<Vec<Hunk>,ReviewError>`; `compose(b_text,b_units,c_text,&[Hunk],accepted:&BTreeSet<HunkId>)->String` (Replace/Insert copy candidate bytes, so `c_text` is required). `Hunk{id,kind:Replace|Insert|Delete,baseline:Range<usize>,candidate:Range<usize>}`: ranges are byte ranges into the respective text; Insert has an empty baseline range at the boundary, Delete an empty candidate range. `RULES_VERSION=1`.
- Application: `start(session,date,candidate_id)->Result<ReviewView,ReviewError>`; `decide(review_id,{operation_id,hunk_id,action:Accept|Reject,expected_revision,expected_set_digest})->Result<Decision,ReviewError>`; `end(review_id)->Result<(),ReviewError>`. Errors map to `unsupported`, `limit`, `stale_baseline`, `stale_revision`, `set_mismatch`, `busy`, `uncertain`, `not_found`.
- Receipt stored in `Record.result` as JSON `{hunk,revision,before,after}`. Digest encoding (fixed here): SHA-256 over length-prefixed fields (u32 big-endian length + bytes). A hunk-ID preimage is `rules_version, baseline digest, candidate digest, kind, baseline start, candidate start`; a set digest hashes the hunk IDs in ascending byte order. The empty set has the digest of zero IDs and is what the first accept sends as `expected_set_digest`.

## Plan review corrections (2026-11-07)

Checked against the contract and current code (R02). Decisions that fix gaps in the draft:

1. **Compose input:** takes the candidate text and defines empty ranges for Insert/Delete (Interfaces above).
2. **Unsupported input:** an unclosed fence is valid CommonMark (runs to end of file), so it is not an `unsupported` fixture. `unsupported` is tested on the span verifier with synthetic gaps, overlaps and non-whitespace holes. Link reference definitions emit no `pulldown-cmark` event; their bytes form a non-whitespace gap that attaches to the preceding unit (fixture added); any other gap that is not whitespace is `unsupported`. The Authentic body is plain Markdown with no front matter or header (`diary-domain::authentic` has none), so no prefix rule beyond leading whitespace is needed.
3. **Receipt path:** an accept is its own operation kind `review_accept`, not an ordinary body save. Its input hash covers date, baseline revision, candidate digest, hunk ID, expected revision and expected set digest; the composed body is derived and not hashed, so an accept never collides with a manual save of the same body. `AuthenticStore` gains one method (or an optional receipt on `SaveRequest`) that writes the ledger `result` in the same transaction as the save; plain saves are unchanged.
4. **Recovery order:** `decide_review` looks up the operation ID before the review, so after a restart the recorded receipt returns instead of `not_found` (T06, T09). The existing `get_operation` bridge command and `Coordinator`-side lookup are reused; T09 asserts a review-accept ID resolves through it.
5. **Serialization:** accepts, rejects and end reserve `resource_key(date)` on the existing `Coordinator` (the same key autosave uses); no new lock.
6. **Invalidation:** the store learns of revision changes from `save_committed` and the existing `AuthenticWatch`. T08 adds: the review's own accepts and their watcher echo do not invalidate it. End-review while an accept is uncertain still blocks manual save until reconciled.
7. **Discard guards (T10):** cover both `DELETE /jobs/{id}/result` and `DELETE /generations/{job}`.
8. **Start and bridge:** the client reuses the Authentic flush-before-admission path from generation; bridge work includes one main-window permission and capability entry per command, following the source-trash pattern.
9. **Limits:** check `baseline.len() * candidate.len() > 4_000_000` before allocating the table (`u16` cells suffice at 4,000 units). A common prefix may be trimmed; a common suffix must not, because it breaks the earliest-index tie rule (`[A,A]→[A]`).
10. **Dependency:** `pulldown-cmark = "=0.13.5"` with `default-features = false`; enable only `Options::ENABLE_TABLES`. Task 0 checks transitive licenses and MSRV against workspace `rust-version` 1.98 with `cargo metadata` before notices are written.
11. **Layout:** `ReviewPane.tsx` is a component rendered inside the Authentic pane (per [UX](../../../ux-gui.md#save-review-and-transition-feedback) and the containment model), not a new pane.

## Unified-merge-style presentation (2026-11-07)

Owner: the review unit stays one paragraph (a group of sentences); the UI should resemble CodeMirror's unified merge view.

- **View model:** the server returns, with every start/decision, `ReviewView{revision, body, set_digest, hunks:[{id, kind, label?, range, text}]}`. `body` is the current composed saved text; each pending hunk's `range` is a byte range into `body` (the baseline text it would remove or replace; empty for Insert, positioned at the boundary) and `text` is the candidate text it would add (empty for Delete). Domain adds `view(accepted, rejected) -> pending hunks with ranges`. Rejected hunks leave the list; accepted hunks are plain text in `body`. Ranges are re-derived on every reply, never shifted client-side.
- **GUI (hand-coded, no dependency):** a CodeMirror 6 extension in `src/components/` renders `ReviewView` onto the read-only `body`: red strikethrough mark over `range` (Delete/Replace), green inline widget after it showing `text` (Insert/Replace), a gutter marker on changed lines, structural label badge, and a small Accept/Reject button pair widget at the end of each change (buttons are real `<button>`s with accessible names containing a short excerpt; Enter/Space activate; Tab order follows the document). Look and behavior mirror `unifiedMergeView({ allowInlineDiffs: true, mergeControls: true, gutter: true })`. Buttons call the review controller (server-authoritative durable accept), not CodeMirror's own `acceptChunk`/`rejectChunk`, which would mutate the editor document locally. If the owner prefers `@codemirror/merge` itself, it needs a dependency approval and a custom `mergeControls` function wired to the controller; the server flow is unchanged either way.

## Stages and tasks

Each task is test-first: write the named scenario, see it fail, implement, see it pass, commit.

### Stage A — domain (T01–T03)

0. Add pinned `pulldown-cmark` to `diary-domain` (O1), regenerate lockfile and license notices (`scripts/notices`), run the gate.
1. T01 tokenizer (offset iterator over `pulldown-cmark`, spans verified byte-exact): fixtures for prose with soft/hard wraps, heading, nested list continuation, blockquote, fenced code containing blank lines and `#`, table, CRLF, leading whitespace prefix unit, trailing blank attachment, an unclosed fence (valid, runs to end), and a link reference definition (attaches to the preceding unit). `unsupported` is tested on the span verifier with synthetic gaps/overlaps. Property: `units` ranges concatenate to the input.
2. T02 alignment: the contract examples (`[A,B,C]→[A,B2,X,C]`, deletion `[A,B,C]→[A,C]`), repeated paragraphs tie rule, all-new and all-removed documents, ID stability across runs, limit errors at 4,001 units and over 4 M comparisons (checked before allocation), and `[A,A]→[A]` earliest-index tie.
3. T03 compose: accept-all equals candidate bytes for every T02 fixture; accept-none equals baseline; each single accept; empty baseline; whitespace-only difference.

### Stage B — application (T04–T08)

4. T04 start guards: stale baseline, dirty queue, absent baseline as virtual empty (no file written), wrong session/date.
5. T05 durable accept with a fake `AuthenticStore` and ledger: receipt and save commit together; memory unchanged on failure.
6. T06 recovery: drop the reply then replay by operation ID (looked up before the review, also after restart) (no double publish, no regression after a later accept); restart keeps saved body, review gone.
7. T07 serialization: concurrent accept/reject/end deterministic order; failed accept retry/discard; uncertain blocks decisions; committed hunk cannot be toggled.
8. T08 invalidation: external, local and server revision changes drop pending hunks, keep accepted text, emit one `review_invalidated`; the review's own accepts and watcher echoes do not invalidate.

### Stage C — transport (T09–T10)

9. T09 routes/bridge: schema, bounds, session mismatch, forged IDs, capability grants, no generic HTTP.
10. T10 guards: both discard routes and regenerate refused while a review or admitted accept exists, allowed after settle; date change ends only the outgoing review; close cancels runs and drops reviews after accepts settle.

### Stage D — UI (T11–T12)

11. T11 Vitest with a fake bridge: the decoration builder maps `ReviewView` to strike/insert/gutter/button DOM; per-hunk Accept/Reject, structural labels, read-only editor only during review, ack-gated accepted state, stale and invalidated messages, transition guards, focus restoration.
12. T12 accessibility assertions: named controls, one polite status source, no announcement flood.

### Stage E

Add `scripts/smoke/review.py` on the real server binary (start, accept, reject, external change, kill/restart), run `scripts/gate.sh`, update docs listed in C15. Delegate test and gate runs to `test-runner`.

## Risks to resolve in Stage B before coding the UI

- Receipt path is decided (correction 3); the store change (task 5) touches the shared Authentic save path, so autosave regression tests must stay green.
- The tokenizer is the main correctness risk; keep it conservative (`unsupported` over guessing).

## Delivered differences from this plan

- **Serialization (correction 5):** there is no separate resource lock in the session coordinator. Decisions serialize through a per-review `busy` flag, the date's ledger reservation and the store write lock, which autosave also uses.
- **Invalidation (correction 6):** no `review_invalidated` stream event was added, because a review is not a job. The server answers the next decision with `review_invalidated`, and the client re-reads the revision on `authentic_changed` and drops the review once.
- **Receipt:** carried as the tail of the operation result (`seq:sha hunk,before,after`) rather than a new column; the intent step stores the same tail so crash reconciliation reproduces it.
- **Blockquote:** a whole blockquote is one unit (coarser than "blockquote paragraph", never splits a structure).
- **Offsets:** pending hunks are located with UTF-16 offsets so the client never recomputes positions; Authentic bodies cannot contain `\r`.
- **Known rough edge:** a last paragraph with and without a trailing newline differs byte-wise and shows as a replacement, as the byte-exact contract requires.
