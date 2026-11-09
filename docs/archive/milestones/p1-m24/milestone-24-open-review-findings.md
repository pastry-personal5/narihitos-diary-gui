# P1-M24: Open review findings

Status: Superseded by [M32 resolution](../../../phase-1/milestone-32-release-evidence.md#final-post-hardening-results-2026-11-10)

Owner approved and M32 resolved these five historical issues on 2026-11-10; the original report below is preserved.

Five issues from the post-delivery `/code-review --fix` of the paragraph review (2026-11-07) were not fixed because each needs a behavior or design decision. The six fixed findings are not listed here. Each item below is unverified against the running app; none has a test yet.

## 1. Trailing separator in paragraph units

- **Problem:** a unit owns its trailing separator, so a baseline whose last paragraph has no trailing newline compares unequal to the same paragraph in the candidate.
- **Effect:** a bogus whitespace-only Replace is shown. Accepting only the following Insert merges two paragraphs: baseline `P1\n\nP2` plus candidate `P3` saves `P1\n\nP2P3`.
- **Where:** `src-tauri/crates/diary-domain/src/review.rs` (around line 228).
- **Decision needed:** how to treat the separator (normalize a missing final separator, or compare units without it and re-add it on accept).

## 2. `Reviews` mutex held during `store.load()`

- **Problem:** the reject path reads from disk while holding the global `Reviews` mutex, although the comment says the store call runs outside it.
- **Effect:** a slow iCloud-backed read blocks every `Reviews` call, including `has_date` and `uses_job`, which the generations handlers call on async executor threads. Generate and Discard can stall.
- **Where:** `src-tauri/crates/diary-application/src/review.rs` (around line 275).
- **Decision needed:** restructure so the revision is checked without the lock held (read first, then re-validate under the lock).

## 3. Typing lost during `start()`

- **Problem:** `start()` flushes the Authentic queue, then awaits the server. `show()` replaces the buffer with the server body unconditionally.
- **Effect:** text typed between the flush and the response is replaced, unsaved and not flagged dirty.
- **Where:** `src/application/review/controller.ts` (around line 99).
- **Decision needed:** pause editing before the flush (read-only until the response), or reject the response if the buffer changed.

## 4. False invalidation in `onAuthenticChanged`

- **Problem:** it compares a file loaded before an awaited decision against `now.revision`, which that decision may have advanced.
- **Effect:** with back-to-back accepts, the watcher event for accept 1 loads revision 2; accept 2 finishes (revision 3) before the load returns; 2 ≠ 3 calls `invalidate()` and wrongly ends the review.
- **Where:** `src/application/review/controller.ts` (around line 245).
- **Decision needed:** compare against the revision captured before the await, or re-check on any change.

## 5. Stale view after `decided` / `set_mismatch`

- **Problem:** after a lost reply, the retry can return `decided` or `set_mismatch`, and the client does not refresh the view. A lost-reply reject has no operation record, so its retry returns `decided`.
- **Effect:** stale Accept/Reject buttons remain on hunks already decided.
- **Where:** `src/application/review/controller.ts` (`decide()`, around line 180). The retry keeps its operation ID for retryable errors since the partial fix.
- **Decision needed:** refresh the view on these outcomes, and whether rejects need a durable operation record.

## Evidence limits

Not run after the fixes: the full gate, `python3 scripts/smoke/review.py`, and the tests of crates other than `diary-api`, `diary-infrastructure` and `diary-application`. No test covers any item above.
