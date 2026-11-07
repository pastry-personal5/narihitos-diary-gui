# P1-M24: Paragraph review and acceptance overview

Status: Done

Add deterministic, lossless paragraph review of a candidate against the saved Authentic, with serialized durable accepts, transient rejects, and guards for stale baselines, end-review, date change, close and discard. The [work plan](milestone-24-paragraph-review-architecture.md) defines stages and scenarios; this plan does not certify runtime behavior.

## Entry conditions

P1-M22 (candidates) and P1-M23 (activity/results, `stale_baseline`, terminal discard) are Done. Follow [Paragraph alignment and review](../../../architecture/editing-and-agents.md#paragraph-alignment-and-review), [API](../../../architecture/api-and-processes.md) (`start_review`, `decide_review`, `end_review`, `review_invalidated`, `discard_transient_result`), [UX](../../../ux-gui.md#save-review-and-transition-feedback), [background AI](../../../architecture/ai-job-experience.md) and [handoff E6/E13](../../../phase-1/implementation-handoff.md). No review code exists yet.

## Scope

- Stage A: pure domain — lossless unit tokenizer, deterministic LCS alignment, hunks, exact recomposition, limits.
- Stage B: application review store and coordinator — start, serialized accept/reject/end, durable accepted-set transitions through the normal Authentic save, invalidation.
- Stage C: server routes, typed bridge commands, `review_invalidated`, review-dependent discard guards.
- Stage D: Review UI in the Authentic pane, unified-merge-style inline (red struck removals, green insertions, gutter marker, per-paragraph Accept/Reject, labeled structural units, manual-edit pause), transition guards, regenerate.
- Stage E: scenarios, full gate, documentation.

Out of scope: history, AI-assisted merge or repair, automatic rebase, export, persistent reviews (restart loses pending hunks by design), new prompts or adapters, live-provider proof. Packaged native proof moves to P1-M28 (O3).

## Owner decisions (2026-11-07)

- **O1 — Tokenizer:** adopt `pulldown-cmark` for source-range parsing in `diary-domain`, pinned to an exact version with license-notice review (Stage A first task). Units must still concatenate byte-exactly; if its offsets cannot preserve exact spans for an input, review is `unsupported`.
- **O2 — Digests:** SHA-256 is computed in infrastructure behind a hasher port the domain and application receive by injection. `diary-domain` gains no `sha2` dependency.
- **O3 — Evidence:** close on automated tests, the synthetic built-server smoke and the full gate. Packaged native keyboard, reduced-motion and responsiveness proof moves to P1-M28; no human exit gate. Native VoiceOver stays exempt.

## Completion checklist

Check only when evidence exists. Scenario IDs refer to the work plan.

### Plan review

- [x] R01 — Owner answered O1–O3 (2026-11-07); recorded here and in the [changelog](../../../phase-1/changelog.md).
- [x] R02 — Plan checked against current code and contracts (2026-11-07); corrections recorded in the work plan's [Plan review corrections](milestone-24-paragraph-review-architecture.md#plan-review-corrections-2026-11-07).

### Stage A — domain

- [x] C01 — Units are byte-exact and concatenate to the input; structures (heading, list item, blockquote paragraph, fence, table) are indivisible and labeled; untokenizable input is `unsupported`, never whole-file replacement (T01).
- [x] C02 — LCS with earliest-baseline-then-earliest-candidate ties; replacement/deletion/insertion pairing per differing region; stable hunk IDs include rules version, digests and positions; limits of 4,000 units and 4 million comparisons (checked before allocation) return a review-limit error (T02).
- [x] C03 — Accepting every hunk reproduces candidate bytes exactly; any accepted subset composes from the immutable baseline in original order, including an absent (virtual empty) baseline and whitespace-only changes (T03).

### Stage B — application

- [x] C04 — Start requires clean queue and baseline equal to the candidate's captured baseline; otherwise regenerate. Starting writes no file (T04).
- [x] C05 — Accept is one durable `review_accept` operation saved through the Authentic store with `operation_id`, expected revision and expected prior accepted-set digest; hunk, resulting revision and before/after digests are recorded in the same transaction as the save receipt; memory updates only after (T05).
- [x] C06 — Lost reply resolves through the original operation ID before review-existence or stale checks, including after restart; replay repairs memory only when the current set equals the before-digest and never publishes twice or regresses later accepts. Restart keeps the saved body and drops the review (T06).
- [x] C07 — Accepts, rejects and end-review are serialized per review/resource. A failed accept keeps the saved set and offers retry/discard; an uncertain accept blocks further decisions and manual save until reconciled. Reject writes nothing; committed hunks cannot be toggled (T07).
- [x] C08 — Any unrelated revision change (local, server, external) emits `review_invalidated`; accepted text stays saved; no rebase (T08).

### Stage C — transport

- [x] C09 — `POST /reviews`, `POST /reviews/{id}/decisions`, `DELETE /reviews/{id}` and bridge commands `start_review`, `decide_review`, `end_review` validate identity, bounds, session and capability grants; no generic HTTP access (T09).
- [x] C10 — `discard_transient_result`, `DELETE /generations/{job}` and regeneration wait for admitted accepts and refuse while a review uses the result; date change and close end the review after accepts settle; background generation and other dates' candidates are untouched (T10).

### Stage D — UI

- [x] C11 — Review renders the unified-merge-style view from `ReviewView` (inline strike/insert, gutter, Accept/Reject per hunk with structural labels); Authentic editing is paused (read-only) only during review and resumes on end; source editing stays independent; accepted state shows only after acknowledgement (T11).
- [x] C12 — Stale candidates show `Entry changed — regenerate to review`; transitions use existing save/dirty guards; focus returns to the invoker; polite announcements for accept/reject/end/invalidated without per-keystroke noise (T11/T12).

### Stage E — proof and docs

- [x] C13 — T01–T12 automated matrix passes, including E6 cases (ack loss, restart, stale baseline, end-review races, structural/limit errors). Synthetic built-server smoke covers start, accept, reject, invalidation, restart.
- [x] C14 — Full gate passes (`scripts/gate.sh`: fmt, Clippy `-D warnings`, cargo test, Prettier, ESLint zero warnings, Vitest, tsc); execution delegated to `test-runner`.
- [x] C15 — README, architecture, handoff E6/E13, phase plan, roadmap, changelog, index and AGENTS reflect delivered behavior and evidence limits.

## Evidence record (2026-11-07)

- Domain: `diary-domain` `review::tests` (T01–T03, set digests, view offsets). Application: `diary-application` `review::tests` (T04–T08 with a fake store: failed accept, lost reply, replay without regression, restart receipt, busy/uncertain/end, external-change invalidation). Infrastructure: `review_accept_*` tests (receipt in the save transaction, replay by ID, mismatch against a plain save, every crash boundary). API: `t09_*` tests (validation, forged IDs, set mismatch, both discard routes and regeneration refused while a review exists). Bridge: `bridge::review::tests`. Client: `tests/review-controller.test.ts`, `review-editor.test.ts`, `review-validation.test.ts`.
- Smoke: `python3 scripts/smoke/review.py` on the real server binary (start writes nothing, reject writes nothing, durable accept, replayed accept, invalidation, kill between publication and receipt, restart receipt) and `scripts/smoke/agent.py`/`authentic.py` still pass.
- Gate: `scripts/gate.sh` passed (see the final run in the changelog entry).
- Evidence limits: no packaged native keyboard, reduced-motion or responsiveness proof (P1-M28); no live provider run was required; Qwen live proof stays exempt (GPU throttling). Closing the window with an accept in flight relies on the lease-end path rather than a dedicated wait.
