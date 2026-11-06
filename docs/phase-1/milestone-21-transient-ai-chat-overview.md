# P1-M21: Transient AI Chat overview

Status: Done 2026-11-06

Add the first run-starting surface: a profile-fixed, memory-only AI Chat that sends the active editor snapshot, streams a provisional reply and cancels cleanly. It also builds the real job layer (single-run `busy` gate, runner, bounded SSE progress) that P1-M22 reuses for Generate. The [work plan](milestone-21-transient-ai-chat-architecture.md) defines findings, owner decisions, design, files, sequencing and tests.

## Entry conditions

- P1-M16 (port, `ChatPacket`, `Run`, bounds), P1-M17–P1-M20 (four adapters behind one `Agents` registry, profile status, typed diagnostics) are Done. Follow [context, output and limits](../architecture/editing-and-agents.md#context-output-and-limits), the [background AI contract](../architecture/ai-job-experience.md), the [API chat routes](../architecture/api-and-processes.md) and handoff E5/E11/E13.
- [Owner decisions](milestone-21-transient-ai-chat-architecture.md#owner-decisions) Q1–Q6 of 2026-11-06 are recorded.

## Scope

Included:

- **Stage A, job layer:** replace the P1-M3 fake `Coordinator` transitions with a runner that executes an `AgentRunner` run on a worker, one active agent run per client, `busy` rejection, idempotent admission, bounded progress snapshots, 50 ms / 16 KiB delta batching, `Replace`, cancel and terminal states over the existing `/jobs` and `/events`.
- **Stage B, chat service:** `POST /chats`, `POST /chats/{id}/turns`, `GET /chats/{id}`, `DELETE /chats/{id}`; memory-only transcript held for the lease; fixed profile and locale per chat; matching bridge commands and capability grants; typed diagnostics for chat events.
- **Stage C, UI:** the AI Chat pane (Profile selector, transcript, composer, Send/Cancel, stage and elapsed feedback, context chip per message, New chat) and the toolbar AI activity summary; `agents_available` becomes true when at least one profile is ready.
- **Stage D, proof and docs:** conformance and fake-clock tests, an env-gated live chat turn plus cancel per adapter, affected docs.

Not included: Generate, candidates, source selection and packets (P1-M22); the results badge, off-date result routing and the full activity details (P1-M23); paragraph review (P1-M24); persistent or cross-restart transcripts; model tools or persistent model sessions; macOS notifications; a new Cargo or npm dependency; any change to adapter argv, pins or fixtures.

## Completion checklist

Check only with recorded evidence. Planning does not complete these items.

### Stage A — job layer

- [x] C01 — One runner owns agent runs; the `Coordinator` fake transitions are gone from the production path. Evidence: runner tests with a fake `AgentRunner`; `git grep` shows no test-only transition used by routes.
- [x] C02 — At most one agent run per client; a second admission returns `busy` after the idempotent-lookup check, and an identical retry returns the admitted job. Evidence: `second_run_is_busy`, `retry_returns_same_job`, `changed_payload_is_mismatch`.
- [x] C03 — Progress snapshots expose only observed stage, counts and elapsed time; deltas are batched at 50 ms or 16 KiB and `Replace` supersedes earlier text; a slow SSE consumer gets `resync_required`, never unbounded buffering. Evidence: fake-clock tests `delta_batching_bounds`, `replace_supersedes`, `slow_consumer_resyncs`.
- [x] C04 — Cancel commits once; late deltas and results are dropped; lease expiry, session close and server shutdown cancel the run. Evidence: `cancel_wins_over_late_success`, `lease_end_cancels_run`, `shutdown_cancels_run`.

### Stage B — chat service

- [x] C05 — `open_chat` fixes profile and the saved writing locale; unknown, unready or removed profiles are refused with the closed profile reason; config reload never changes an open chat's profile or locale. Evidence: API tests per case.
- [x] C06 — A turn builds `ChatPacket` from the request's active-document snapshot (kind, date, unsaved flag, body), the server-held transcript and the new message; no document means no document context; the 64 KiB message, 100 turn, 128 KiB transcript and 256 KiB packet bounds fail visibly as `context` and nothing is truncated. Evidence: `chat_bounds_fail_without_truncation`, `no_document_sends_no_context`.
- [x] C07 — The transcript is memory-only: it is erased on close, lease expiry and exit, is retained within the transient byte cap, is returned by `GET /chats/{id}` after a client reload, and a failed or cancelled turn adds no assistant message. Evidence: `transcript_survives_reload_within_lease`, `transcript_erased_on_close_and_expiry`, `failed_turn_leaves_transcript_intact`.
- [x] C08 — Agent errors map to the closed API codes with no model text; chat records are typed and content-free. Evidence: `failure_reports_a_code_and_no_text` (API) and the M20 seeded-string log scans, which already run chat-mode failures; no new server-level scan was added (see the evidence record).
- [x] C09 — Bridge commands (`open_chat`, `send_chat`, `get_chat`, `close_chat`, cancel via existing job command) use minimum capabilities and typed validation. Evidence: bridge tests, capability guard test.

### Stage C — UI

- [x] C10 — Send responds within one frame with a submitting state; stages, elapsed time and Cancel show truthfully; editing, autosave and navigation stay usable during a run. Evidence: Vitest with a fake bridge and fake clock.
- [x] C11 — Each user message shows its context chip (`Entry 2026-11-06, unsaved edits included`, `Source …`, or `No document`); a profile change or New chat starts a fresh session; Send is disabled with a reason when no profile is ready, the draft is empty or a run is active. Evidence: Vitest per state.
- [x] C12 — Provisional text is visibly marked and replaced by the validated reply; reload re-fetches the transcript and an in-flight job. Evidence: Vitest `provisional_then_final`, `reload_recovers_transcript`.
- [x] C13 — Accessible: labeled controls, a polite live region for completion without moving focus, reduced-motion respect, IME composition does not trigger Send. Evidence: Vitest and accessibility queries; native VoiceOver proof is not required.

### Stage D — proof and docs

- [x] C14 — A chat conformance matrix runs over all four kinds with fake children: success, busy, cancel, timeout, flood, late result, reload during run, no content in errors. Evidence: the M20 `agent_conformance.rs` matrix already runs the lifecycle in chat mode through the registry, and `diary-api/src/chats/tests.rs` adds the chat-level cases (see the evidence record).
- [x] C15 — Env-gated, agent-run live tests send one real chat turn and one cancel per adapter through the built server (Codex profile `xpp8`, Claude default login, Qwen local endpoint, Apple on-device), recording versions and timing. No human gate. A kind that is unavailable here leaves its item unchecked with the exact reason.
- [x] C16 — `agents_available` is true only when a runner is wired and a profile has a ready chat capability (assets valid, adapter registered; not a live status check); no Generate route or UI exists; no `Cargo.lock` or `package*.json` change.
- [x] C17 — Affected docs state actual behavior and evidence limits: editing-and-agents, api-and-processes, ai-job-experience, logging, handoff E5/E11/E13 and the M21 row, phase plan, roadmap, changelog, docs index, README, AGENTS.md.

## Plan-review checklist

- [x] R01 — Compare the plan with the maintained contracts and the M16–M20 code; findings are in the work plan.
- [x] R02 — Map each finding to a C item and a named test.
- [x] R03 — Format the changed plans, check local links and whitespace, and review the scoped diff.

## Implementation gate

Run at the end of each stage; a stage does not start while the previous gate is red.

- [x] G01 — `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [x] G02 — `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [x] G03 — `cargo test --manifest-path src-tauri/Cargo.toml`
- [x] G04 — `npx prettier --check .`
- [x] G05 — `npx eslint . --max-warnings 0`
- [x] G06 — `npx vitest run`
- [x] G07 — `npx tsc --noEmit`
- [x] G08 — `python3 scripts/smoke/agent.py` extended for chat routes with fake CLIs

If a command cannot run, leave its item unchecked and record the blocker. No human test is an exit gate.

## Evidence record

### Stage A (2026-11-06, commit c6fb0ee)

C02, C03 and C04 (cancel part): `second_run_is_busy`, `retry_returns_same_job`, `changed_payload_is_mismatch`, `touch_bumps_sequence_until_terminal`, `delta_batching_bounds`, `replace_supersedes`, `phase_flushes_pending_text_and_is_recorded`, `cancel_wins_over_late_success` in `diary-application/src/run/tests.rs`. Gap and byte-bound resync reuse the existing `ordered_replay_reports_gap_even_when_notification_was_lost` and `encoded_byte_bounds_force_resync_without_losing_current_state`; events carry no text, so a slow consumer can only lose notifications, never content.

### Stage B (2026-11-06)

- C01: `diary-api/src/chats.rs` runs `run::execute` on `spawn_blocking`; `Coordinator::admit_agent` is the gate. The fixture transitions remain only under the `scaffold-smoke` feature.
- C04: `cancel_wins_over_late_success_and_leaves_transcript_empty`, `transcript_erased_on_close_and_lease_end_cancels_the_run` (the run's token is cancelled by `lease_ended`, which the server also wires to `Agents::cancel_all`).
- C05: `open_refuses_unknown_unready_and_malformed`, `without_a_runner_chat_is_unavailable`; the profile and locale are fixed in `Chat` at open.
- C06/C07: `chat_bounds_fail_without_truncation`, `no_document_sends_no_context`, `success_appends_both_turns_failure_and_cancel_append_none`, `late_result_for_a_replaced_turn_is_ignored` (application); `turn_replies_and_transcript_survives_reload_within_lease`, `oversize_message_is_context_and_nothing_is_admitted`, `failure_reports_a_code_and_no_text` (API).
- C09: bridge tests in `src/bridge/chat.rs` (fixed routes, input bounds, closed response vocabularies, chat error codes pass through the allowlist); the command inventory test in `tests/architecture.test.js` lists the four commands and their permissions.
- Design note: the shared host serializes runs, so a turn started in the instant after a cancel, before the cancelled process is reaped, fails `busy` and the text returns to the composer.

### Stage C (2026-11-06)

`tests/chat.test.ts` (controller: lazy open, provisional then final, chips, failure restores text, busy, cancel, profile change starts a new chat, own-job events only, reload adoption, `not_found`) and `tests/chat-ui.test.tsx` (disabled reasons, provisional marker plus Cancel, completion live region, IME-safe Cmd+Enter, failed message back in the composer, plain error text). The submitting state is set synchronously before the first await. No native VoiceOver or Korean IME proof was run (P1-M28).

### Stage D (2026-11-06)

Live, agent-run on macOS 26.7.1 through the real `diary-api` router, job layer, chat service and adapter, with `DIARY_CHAT_LIVE=<kind> cargo test -p diary-server --test chat_live -- --ignored --nocapture --test-threads=1`: Apple chat turn 0.42 s and cancel settled 0.11 s; Codex 0.167.0 (`~/.codex-pp8`) 3.6 s and 0.11 s; Claude Code 2.1.319 (default login) 2.6 s and 0.83 s; Qwen Code 0.26.4 (local endpoint) 66.5 s and 0.21 s. Each turn showed a stage while running and reached `done` with an assistant message; each cancel reached a terminal state with no reply added. These runs sent only synthetic text. `scripts/smoke/agent.py` now also checks chat refusals, open, read and close on the real server binary, and `agents_available`.

`agents_available` was first derived from startup availability, which reports `agent_adapter_unverified` for every agent profile, so the real-server smoke caught it staying false; it now uses the ready chat capability of the profile directory.

### Limits

No packaged-app proof; the transcript is not recoverable after a server restart by design; provider retention of chat content follows each CLI or Apple policy; Apple's 4096-token context refuses long documents as `context`.
