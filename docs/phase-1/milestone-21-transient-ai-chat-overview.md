# P1-M21: Transient AI Chat overview

Status: Active (plan approved 2026-11-06, O1–O3 accepted; stage A delivered, B–D open)

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

- [ ] C01 — One runner owns agent runs; the `Coordinator` fake transitions are gone from the production path. Evidence: runner tests with a fake `AgentRunner`; `git grep` shows no test-only transition used by routes.
- [ ] C02 — At most one agent run per client; a second admission returns `busy` after the idempotent-lookup check, and an identical retry returns the admitted job. Evidence: `second_run_is_busy`, `retry_returns_same_job`, `changed_payload_is_mismatch`.
- [ ] C03 — Progress snapshots expose only observed stage, counts and elapsed time; deltas are batched at 50 ms or 16 KiB and `Replace` supersedes earlier text; a slow SSE consumer gets `resync_required`, never unbounded buffering. Evidence: fake-clock tests `delta_batching_bounds`, `replace_supersedes`, `slow_consumer_resyncs`.
- [ ] C04 — Cancel commits once; late deltas and results are dropped; lease expiry, session close and server shutdown cancel the run. Evidence: `cancel_wins_over_late_success`, `lease_end_cancels_run`, `shutdown_cancels_run`.

### Stage B — chat service

- [ ] C05 — `open_chat` fixes profile and the saved writing locale; unknown, unready or removed profiles are refused with the closed profile reason; config reload never changes an open chat's profile or locale. Evidence: API tests per case.
- [ ] C06 — A turn builds `ChatPacket` from the request's active-document snapshot (kind, date, unsaved flag, body), the server-held transcript and the new message; no document means no document context; the 64 KiB message, 100 turn, 128 KiB transcript and 256 KiB packet bounds fail visibly as `context` and nothing is truncated. Evidence: `chat_bounds_fail_without_truncation`, `no_document_sends_no_context`.
- [ ] C07 — The transcript is memory-only: it is erased on close, lease expiry and exit, is retained within the transient byte cap, is returned by `GET /chats/{id}` after a client reload, and a failed or cancelled turn adds no assistant message. Evidence: `transcript_survives_reload_within_lease`, `transcript_erased_on_close_and_expiry`, `failed_turn_leaves_transcript_intact`.
- [ ] C08 — Agent errors map to the closed API codes with no model text; chat records are typed and content-free. Evidence: error-table test, seeded-string log scan over a failing chat for all four kinds.
- [ ] C09 — Bridge commands (`open_chat`, `send_chat`, `get_chat`, `close_chat`, cancel via existing job command) use minimum capabilities and typed validation. Evidence: bridge tests, capability guard test.

### Stage C — UI

- [ ] C10 — Send responds within one frame with a submitting state; stages, elapsed time and Cancel show truthfully; editing, autosave and navigation stay usable during a run. Evidence: Vitest with a fake bridge and fake clock.
- [ ] C11 — Each user message shows its context chip (`Entry 2026-11-06, unsaved edits included`, `Source …`, or `No document`); a profile change or New chat starts a fresh session; Send is disabled with a reason when no profile is ready, the draft is empty or a run is active. Evidence: Vitest per state.
- [ ] C12 — Provisional text is visibly marked and replaced by the validated reply; reload re-fetches the transcript and an in-flight job. Evidence: Vitest `provisional_then_final`, `reload_recovers_transcript`.
- [ ] C13 — Accessible: labeled controls, a polite live region for completion without moving focus, reduced-motion respect, IME composition does not trigger Send. Evidence: Vitest and accessibility queries; native VoiceOver proof is not required.

### Stage D — proof and docs

- [ ] C14 — A chat conformance matrix runs over all four kinds with fake children: success, busy, cancel, timeout, flood, late result, reload during run, no content in errors. Evidence: `chat_conformance.rs`.
- [ ] C15 — Env-gated, agent-run live tests send one real chat turn and one cancel per adapter through the built server (Codex profile `xpp8`, Claude default login, Qwen local endpoint, Apple on-device), recording versions and timing. No human gate. A kind that is unavailable here leaves its item unchecked with the exact reason.
- [ ] C16 — `agents_available` is true only when a profile is ready; no Generate route or UI exists; no `Cargo.lock` or `package*.json` change.
- [ ] C17 — Affected docs state actual behavior and evidence limits: editing-and-agents, api-and-processes, ai-job-experience, logging, handoff E5/E11/E13 and the M21 row, phase plan, roadmap, changelog, docs index, README, AGENTS.md.

## Plan-review checklist

- [ ] R01 — Compare the plan with the maintained contracts and the M16–M20 code; findings are in the work plan.
- [ ] R02 — Map each finding to a C item and a named test.
- [ ] R03 — Format the changed plans, check local links and whitespace, and review the scoped diff.

## Implementation gate

Run at the end of each stage; a stage does not start while the previous gate is red.

- [ ] G01 — `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [ ] G02 — `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] G03 — `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] G04 — `npx prettier --check .`
- [ ] G05 — `npx eslint . --max-warnings 0`
- [ ] G06 — `npx vitest run`
- [ ] G07 — `npx tsc --noEmit`
- [ ] G08 — `python3 scripts/smoke/agent.py` extended for chat routes with fake CLIs

If a command cannot run, leave its item unchecked and record the blocker. No human test is an exit gate.
