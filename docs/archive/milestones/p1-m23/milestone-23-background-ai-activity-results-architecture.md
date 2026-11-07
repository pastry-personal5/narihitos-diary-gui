# P1-M23: Background AI activity and results work plan

Status: Done

Implement the shared activity/results contract on the completed chat and generation workflows. Scope, owner decision and completion criteria live in the [overview](milestone-23-background-ai-activity-results-overview.md); this document specifies internal design under D17, without changing dependencies, prompts or adapter invocation.

## Findings from the current tree

Inspected 2026-11-06; these are planning observations, not passing runtime evidence.

| ID  | Finding                                                                                                                                                                                                                          | Planned response                                                                                                                                             | Checklist       |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------- |
| F1  | `diary-api::job_value` enriches generation list/get records; chat progress remains in the chat read. Native `bridge::Job` and frontend `SnapshotJob` retain only basic job state (plus a native error code).                     | Preserve a typed AI progress projection through list/get/subscription; add body-less chat identity/progress. Keep ordinary import/save consumers compatible. | C01–C03         |
| F2  | Native `Bridge::subscribe` already establishes a paginated baseline with the first page's watermark. No explicit frontend `list_jobs` command exists.                                                                            | Extend the existing mechanism and get command; add bounded list command, validators and minimum capability grant. Preserve watermark/gap handling.           | C02/C04/C05     |
| F3  | GenerationController survives editor mounts, but events schedule reads only for jobs it already knows. Chat reload convenience uses a remembered ID.                                                                             | Discover jobs by server identity rather than selected date or storage hint. Reconcile the entire live session before adopting results.                       | C03–C05         |
| F4  | App toolbar has a basic generation/chat line; elapsed hooks and completion messages are owned separately.                                                                                                                        | Shared metadata store and one announcement owner; expandable detail surface and retained results access in the existing toolbar.                             | C07–C10/C13     |
| F5  | M22 already implements immutable context, read-only cards, bounded previews, partial outcomes, staleness and terminal discard. Review fixes guard session/discard interleavings and check payload reservations before retention. | Reuse these workflows and buffers, with one owner per payload/reservation; preserve the review regression tests.                                             | C04/C08/C11/C12 |
| F6  | E13 assigns packaged performance to M23, while recent status text carries it to M28.                                                                                                                                             | Owner resolved: automated tests and full gate close M23; packaged proof belongs explicitly to M28. Native VoiceOver proof stays exempt.                      | C14–C16         |

## Owner interview and boundaries

The owner chose automated tests and the full gate for M23 completion, explicitly moving packaged proof to M28 (2026-11-06). There is no human exit gate and no further unresolved product question at initial planning. Existing decisions already specify nonblocking date changes, one agent run per client, in-app attention only, regeneration after baseline changes and transient session ownership. No new package is proposed. Ask before any later dependency/build-tool change or new behavior beyond those contracts.

## Design

### D1 — Body-less progress transport

Use a typed AI progress projection alongside the basic `{id, sequence, state}` job record. Generation uses its existing snapshot. Chat adds kind, request/instance/session/job identities, chat ID, profile, locale, phase, elapsed duration, observed activity, cancellation and typed failure from the chat/run/coordinator state. Counts and baseline apply only to generation; validate variants rather than inventing chat candidate counts. Keep phases a closed enum. Absent progress for ordinary jobs is valid.

Extend `/jobs` and `/jobs/{id}` consistently, the native `Job` DTO and subscription snapshot validation, `src/application/events.ts` and adapter validators. Add explicit `list_jobs(cursor?)` using the existing bounded HTTP list route and extend existing `get_job`; do not add a second generic get command. Events remain body-less hints; full progress/result reads are authoritative. Do not assume a progress change always produces a new lifecycle sequence: refresh active progress on a bounded schedule and read the latest known snapshot.

Discovery distinguishes retained content from content-free admission metadata. A discarded job remains deduplicated but must not cause endless body reads or reappear as an unread candidate result. Expose a closed availability indication where needed; do not copy source bodies, previews, prompts, stderr or paths into job records. Bound new metadata within the existing session budget. Avoid introducing inverse lock acquisition between coordinator and chat/generation stores; capture coherent snapshots without holding locks over I/O or inference.

### D2 — Application activity store

Add `src/application/activity/` with stable immutable metadata snapshots and subscriptions, owned for the live application session rather than an editor/date. Route keys are `{instance_id, session_id, job_id}` plus origin date/chat ID. Maintain separate lifecycle, observed phase, connection freshness, local admission and unread presentation state. Only a complete baseline can establish that a previously known job is absent; never erase jobs at an intermediate page.

Reuse generation/chat controllers for action preparation and bounded content reads; migrate duplicated activity/busy/announcement decisions into one authority. Text remains in their existing bounded buffers, referenced by identity, without duplicate full-result caches or double budget reservations. Cancellation requests keep the busy reservation until server terminal acknowledgement. Local ambiguous admissions also prevent a contradictory new request until reconciled.

### D3 — Reconnect, gaps and admission ambiguity

On disconnect retain last metadata marked stale. On resync or app activation/wake, validate server instance and lease, obtain the complete bounded list baseline, adopt unknown live-session AI jobs, then fetch manifests/current chat and individually bounded candidate bodies. Re-read complete previews rather than concatenate missing deltas. Serialize/coalesce refreshes and use session epochs plus per-job discard markers on every async continuation. Ignore superseded subscription deliveries and older sequences.

Resend an ambiguous submission only with its original request ID and frozen input; use admission deduplication before busy/source/profile/capacity checks. List discovery helps find the admitted run but does not justify a new inference call. Reconciliation never calls Generate/Send with a fresh ID. A transport outage alone does not prove cancellation or result loss. Only confirmed lease expiry, replacement instance/session or unavailable result establishes interruption/unavailability. Clear transient payloads/accounting on that boundary and retain a small content-free interruption notice. A fresh process without the owning lease cannot recover old transient state; no offline or cross-session resume is promised.

### D3a — Reconciliation ordering and failed reads

A list traversal is not an atomic snapshot. Preserve the first page's event watermark, finish all pages, then apply/replay events after that watermark through the existing subscription mechanism. A job inserted behind the pagination cursor must still be discovered by replay. If replay overflows, restart reconciliation rather than declaring the baseline complete. Enforce the existing page/record bounds, reject repeated or nonadvancing cursors, and retain the previous usable state when traversal fails. One store owns reconciliation; explicit list reads must not race the subscription into replacing a newer baseline.

Lifecycle sequence alone cannot order phase/text changes within a running job. Serialize each job's authoritative reads, coalescing another requested refresh into one follow-up; reject responses from an old session epoch or discard generation. Never let a same-sequence stale response overwrite newer progress. Poll active metadata at most once per second while connected, with at most one request per job in flight; terminal jobs are refreshed only for reconciliation, explicit access or baseline invalidation. Repeated failures back off to at most one attempt per 30 seconds, reset by a successful read or reconnection, without scheduling overlapping retry loops. Cancel and terminal hints bypass the normal refresh delay.

A timeout, malformed response, failed page or transient server error leaves observations stale and retryable; it does not delete payloads, free the busy reservation or mark results unavailable. Only an authoritative owning-session response or a confirmed session boundary establishes absence/unavailability. Validate cross-field invariants (job kind/origin, identities, terminal phase and count totals) at both bridge boundaries. Any added progress fields require an explicit compatibility/version decision under the existing API contract; record whether the change is additive rather than silently changing the API major.

### D4 — Time, silence and bounded rendering

Use server-observed elapsed duration and last protocol activity; advance elapsed with a local monotonic clock between snapshots, stop at terminal state, and refresh after sleep before claiming activity. Show the 30-second silence hint without changing the 120-second inactivity or 10-minute wall deadlines. SSE heartbeat and UI timers never advance agent activity.

Keep server 50 ms/16 KiB batching. Coalesce visible preview renders to at most ten per second, metadata-only updates offscreen and elapsed ticks at most once per second. Terminal/cancel/error updates bypass pending preview batches, which cannot later overwrite terminal state. Preserve bounded transport/backpressure and checked shared 64 MiB client/session accounting plus generation's 5-job/8 MiB store. An exhausted client budget produces a visible retry/discard choice without evicting retained work or triggering a tight read loop.

### D5 — Toolbar, results and focus

Replace the basic activity line with a compact summary and keyboard-operable expandable details within the Main Toolbar Strip. Active details show captured origin/profile/language, phase, counts, elapsed, last activity and Cancel. Disconnection visibly marks the last observation stale. The existing layout and SF Symbols remain in use; no new pane or modal busy overlay.

Owner decision (2026-11-06): on a fresh frontend baseline in the same surviving session, restore retained terminal results indicators silently; do not replay historical completion messages or live-region announcements. After that baseline, a terminal outcome first observed following a temporary disconnect announces once under the existing deduplication rule. Add `reload_restores_indicator_without_announcement` and `disconnect_new_completion_announced_once` to T08 acceptance coverage.

Terminal outcomes create one brief polite message and an accessible retained results indicator. Use the existing Global Messages facility for brief feedback and one coordinated live region so inline chat/cards and toolbar do not announce the same completion. Deduplicate by instance/session/job/terminal sequence, including reconnect. Candidate arrivals update counts without a completion toast for each candidate.

Results details list retained generation jobs and current chat outcome with their origin. View results is explicit: generation navigates to its date/candidate cards and chat opens its pane; apply existing pending-save/dirty-source guards first. If a guard cancels or fails, keep the result unread. Mark opened only after successful navigation; an outcome arriving later creates new unread attention. Dismiss marks attention read without deleting bodies. Opening/dismissing does not remove access to retained results in details. Discard remains explicit and terminal-only, applies to generation results through the existing command, releases only that payload and retains deduplication. Current chat has its existing close lifecycle; no new per-turn transcript deletion behavior is introduced. Review-dependent discard guards arrive in M24.

Do not auto-focus, navigate, scroll, change tabs or insert text on completion. Details close returns focus to the invoker. Busy semantics apply only to output. Announce important transitions, never elapsed ticks/token deltas; labels explain counts without fake percentages. Reduced motion removes decorative activity animation while textual progress remains. Reuse existing candidate New output/follow behavior. Saved-baseline invalidation refreshes retained manifests, including off-date results, and keeps the regeneration label without making another model call.

## Files and sequence

| Stage | Areas                                                                                                                                                                                                              | Deliverable                                                                                             |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- |
| A     | `diary-application` chat/run state as needed; `diary-api` job/chat/generation projections; `src-tauri/src/bridge*`, `events.rs`, `main.rs`, capabilities; `src/adapters`, `src/application/bridge.ts`, `events.ts` | Validated metadata, list/get discovery and compatible subscription baseline.                            |
| B     | `src/application/activity/`, generation/chat controllers and navigation payload budget                                                                                                                             | Session-owned store, reconciliation, cancellation/admission ownership, bounded rendering subscriptions. |
| C     | `src/components/App.tsx`, new activity components/hooks, `GlobalMessages`, chat/generation panes, styles                                                                                                           | Toolbar/details/result attention, guarded routing and coordinated accessibility.                        |
| D     | Existing Rust/API/bridge and Vitest suites; `scripts/smoke/agent.py` extension; maintained docs                                                                                                                    | Automated scenario evidence, real-server synthetic smoke and full gate.                                 |

Implement A → B → C → D. Run affected tests while iterating, then the full gate once the final implementation is ready. Delegate validation commands and first-pass failure analysis to `test_runner`; keep fixes and final evidence judgment in the main thread. No live inference is necessary. Any adapter/prompt change would require a separate scope/evidence review, not silent inclusion here.

## Acceptance matrix

These are proposed scenario names, not claims that tests already exist. Preserve existing coverage and add focused tests where behavior changes.

| ID  | Scenarios                                                                                                                                                                                   | Evidence and checklist                                                                                                                                                            |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T01 | `ai_progress_round_trip`, `ordinary_job_compatible`, `chat_discovery_without_text`, `invalid_phase_or_identity_refused`, `paginated_baseline_complete`                                      | Rust API/bridge + TS decoder tests; C01/C02. Bounds, ownership and no private seeded text in metadata/errors/logs.                                                                |
| T02 | `lost_admission_discovers_generation`, `same_id_retry_one_inference`, `mismatch_refused`, `lost_chat_reply_one_user_turn`                                                                   | Fake bridge and real-server fake-agent tests; C03/C06. Recovery after profile/selection changes uses original frozen input.                                                       |
| T03 | `gap_recovers_unknown_job`, `reload_same_lease_adopts_chat`, `wake_refreshes_before_active`, `restart_reports_interrupted`, `expired_lease_drops_transient`, `discarded_result_unavailable` | Controller/API tests; C03/C05. Read complete preview/manifests, not delta reconstruction; no fresh inference.                                                                     |
| T04 | `old_epoch_read_ignored`, `duplicate_and_reordered_events`, `partial_page_keeps_jobs`, `discard_during_candidate_read`, `replacement_session_discard_ignored`                               | Store/controller tests plus M22 regression suite; C04/C11. No resurrection or accounting leak.                                                                                    |
| T05 | `busy_names_origin`, `cancel_holds_slot_until_ack`, `terminal_cancel_race`, `composer_preserved`, `preparation_cancel_no_inference`                                                         | Fake-clock/controller/API tests; C06/C07.                                                                                                                                         |
| T06 | `silence_0_30_120`, `wall_deadline`, `lease_deadline`, `no_delta_adapter`, `heartbeat_not_model_activity`, `terminal_elapsed_frozen`                                                        | Fake clock/adapter tests; C07. Exercise existing deadlines rather than changing them.                                                                                             |
| T07 | `flood_ten_visible_renders_per_second`, `offscreen_metadata_only`, `terminal_bypasses_batch`, `utf8_budget_before_retention`, `slow_consumer_control_available`                             | Fake timers plus synthetic server tests; C08/C14. Assert preview/output bounds and heartbeat/cancel/save completion under stalled delivery; mocks do not prove WKWebView latency. |
| T08 | `details_keyboard_focus_return`, `single_polite_completion`, `no_tick_or_delta_announcements`, `reduced_motion_textual_progress`, `no_system_attention`                                     | UI accessibility/DOM assertions; C09/C10/C13. One announcement across toolbar/chat/cards and reconnect.                                                                           |
| T09 | `off_date_view_results_guarded`, `completion_preserves_editor_position`, `saved_baseline_changes_off_date`, `create_empty_stales_absent`, `locale_snapshot_unchanged`                       | Navigation/store/UI tests; C10/C12. Save failure or cancelled dirty-source choice leaves the result unopened.                                                                     |
| T10 | `partial_failure_then_cancel_keeps_ready`, `dismiss_keeps_bodies`, `open_clears_attention_not_access`, `discard_releases_payload_keeps_request`                                             | Application/UI/API tests; C11/C14. Zero hidden retry or file mutation.                                                                                                            |

## Review follow-up (2026-11-06)

The second plan review tightened pagination/replay ordering, same-sequence progress reads, bounded refresh/retry ownership and the distinction between a failed read and authoritative result loss. These are internal correctness requirements under D17. Implementation remains pending.

Add the following acceptance cases to T01/T03/T04/T07: `insert_behind_page_cursor_recovered_by_replay`, `replay_overflow_restarts_baseline`, `nonadvancing_cursor_refused`, `same_sequence_progress_cannot_regress`, `failed_read_retains_payload_and_reservation`, `polls_coalesced_and_backed_off`, and `terminal_refresh_bypasses_poll_delay`. Exercise actual API pagination plus fake-clock client ordering; checking a decoder alone is insufficient.

Owner decision (2026-11-06): on a fresh frontend baseline in the same surviving session, restore retained terminal results indicators silently; do not replay historical completion messages or live-region announcements. After that baseline, a terminal outcome first observed following a temporary disconnect announces once under the existing deduplication rule.

## Gate and evidence limits

M23 closes on C01–C16 and `scripts/gate.sh` (the six required commands plus TypeScript). Extend and run the existing synthetic fake-agent integration smoke against the built server. Record test names, command outcomes and short evidence locations in the overview when implementation occurs. Qwen live proof stays **exempt: GPU throttling**.

P1-M28 owns packaged proof of slow synthetic runs while editing/autosaving, toolbar/details and off-date results, native keyboard and reduced motion, reconnect/activation after sleep, 100 ms submission feedback target, maximum-stream editing responsiveness, cancel acknowledgement and actual render/memory bounds. Report measurements separately from provider latency. Native VoiceOver proof is not required; IME/key/resize obligations already assigned to M28 remain there. Passing mocks does not establish native performance, sleep behavior or adapter cancellation.

## Transport compatibility decision

M23 job progress fields are additive API v1 response fields, following the
existing generation projection and the API contract allowing additive fields.
Ordinary job records keep their basic shape. Chat discovery exposes only the
currently retained turn; earlier turn metadata does not imply retained output.
Native and frontend progress validation and adoption remain implementation work.

Replay compatibility implementation (2026-11-07): the SSE replay-complete marker
is opt-in through an acknowledged `x-diary-reconcile: 1` header. Legacy streams
are unchanged; native clients consume the marker internally, re-read replayed
job IDs and commit the baseline only afterward. The maintained API contract now
records this negotiated additive extension. A gap never emits the marker.

Implementation completed 2026-11-07; the archived overview records the passing full gate, scenario evidence and explicit P1-M28 limits.
