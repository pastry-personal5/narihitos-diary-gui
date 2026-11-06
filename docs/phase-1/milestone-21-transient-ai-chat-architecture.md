# P1-M21: Transient AI Chat work plan

Status: Done 2026-11-06

This plan adds the job layer and the memory-only chat on top of the four adapters. Scope and checklist are in the [overview](milestone-21-transient-ai-chat-overview.md); the contracts are [context, output and limits](../architecture/editing-and-agents.md#context-output-and-limits), the [background AI contract](../architecture/ai-job-experience.md), the [API](../architecture/api-and-processes.md) (`open_chat`, `send_chat`, `close_chat`, jobs, SSE), [trust and process lifecycle](../architecture/editing-and-agents.md#trust-and-process-lifecycle) and handoff E5/E11/E13. It follows the [M20 work plan](milestone-20-apple-foundation-models-adapter-architecture.md) and repeats none of it.

## Findings that shaped the plan

Read from the code and docs on 2026-11-06. No inference was run.

| #   | Finding                                                                                                                                                                                                           | Decision                                                                                                                       | C item  |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ------- |
| F1  | The only job machinery is the P1-M3 `Coordinator` in `diary-application/src/lib.rs`: admission, idempotency, retained-byte caps, replay ring, `transition`/`cancel`/`publish`. Nothing executes an `AgentRunner`. | Keep admission, idempotency, ring and caps; add a runner that drives transitions from real runs.                               | C01–C04 |
| F2  | `ChatPacket`, `Mode::Chat`, `Run`, `CancelToken`, the bounds (64 KiB / 100 turns / 128 KiB / 256 KiB) and `encode_packet` exist from P1-M16, and `AgentEvent` has `Phase`, `Delta`, `Replace`.                    | Reuse unchanged; M21 adds no adapter or packet code.                                                                           | C06     |
| F3  | `agents_available` is hard-coded `false` in `diary-api` status and in the bridge DTO; tests assert it.                                                                                                            | Derive it from "at least one profile `ready`"; update the two assertions.                                                      | C16     |
| F4  | The AI Chat pane is a heading placeholder (`App.tsx`); the Profile selector, composer and toolbar AI summary do not exist. `writing_locale` is already a saved user setting.                                      | Build the pane and a minimal toolbar summary; read the saved locale on the server at `open_chat`.                              | C05,C10 |
| F5  | The API contract lists `open_chat`, `send_chat`, `close_chat` and `get_transient_result` but not a transcript read. Owner Q4 requires reload recovery.                                                            | Add `GET /chats/{id}` as a contract extension and document it (see Open items O1).                                             | C07     |
| F6  | Apple's on-device context window is 4096 tokens (M20 spike) while the common packet bound is 256 KiB, so long documents will fail on Apple as `context`.                                                          | Surface `context` as "too long for this model: shorten the document, start a new chat or pick another profile"; no truncation. | C06,C15 |
| F7  | The one-run limit is per client and shared by chat and generation, and a chat turn is a job.                                                                                                                      | One busy gate in the runner, reused by M22.                                                                                    | C02     |
| F8  | `phase-1.md` still says P1-M20 Active in its header paragraph although the milestone and roadmap say Done.                                                                                                        | Fix the header with this plan's doc edits.                                                                                     | C17     |
| F9  | M16 reused the 1 MiB draft bound for a chat reply and left it "to revisit in P1-M21". The architecture sets no reply schema or limit.                                                                             | Keep 1 MiB; a longer reply fails `invalid_output`. Revisit only if live proof shows a problem (O2).                            | C08     |

## Owner decisions

Interview 2026-11-06.

- **Q1 — Scope. Decided: full vertical slice** (job layer, chat service, routes, bridge, AI Chat pane). `agents_available` follows profile readiness.
- **Q2 — Streaming. Decided: stream provisional text**, marked provisional and replaced by the validated reply; batching and `Replace` per the job contract.
- **Q3 — Live proof. Decided: one real chat turn and one cancel per available adapter**, env-gated and agent-run, no human gate (Codex `xpp8`, Claude default login, Qwen local endpoint, Apple on-device).
- **Q4 — Reload. Decided: the server keeps the memory-only transcript for the lease**; the UI re-fetches it after reload or reconnect.
- **Q5 — Snapshot visibility. Decided: a context chip on each user message**, no confirmation prompt, per the documented contract.
- **Q6 — Process. Decided: plan and review, then stop for owner approval**; implementation then proceeds in gated stages A–D, one commit per green stage.

## Open owner items

O1–O3 were approved by the owner on 2026-11-06 with their defaults.

- **O1 — `GET /chats/{id}` extends the API contract** (transcript read, bounded by the retained-byte cap). Default: add it and update `api-and-processes.md`.
- **O2 — Chat reply bound.** Default: keep 1 MiB.
- **O3 — Locale unsupported by the profile.** Default: `unsupported_language` is shown as-is; no automatic fallback to another locale.

## Design decisions

**D1 Runner.** `diary-application` gets a `JobRunner` that owns the single active `Run`, spawns the blocking `AgentRunner::run` on a worker thread, and maps `AgentEvent` to coordinator transitions. Phases update the progress snapshot; `Delta`/`Replace` go through a batcher (50 ms or 16 KiB, whichever first) into a bounded provisional buffer that is never an `AgentOutput`. Admission order: idempotent lookup, then session and capacity, then `busy`.

**D2 Terminal handling.** On exit the runner calls `Run::finish`, so a cancel committed first wins. Terminal jobs keep their bounded metadata; the chat reply is stored with the chat, not the job. Lease expiry, session close and shutdown call `request_cancel` and wait for the worker to reach a terminal state before admitting another run.

**D3 Chat store.** `ChatService` holds `chat_id -> { profile handle, locale, transcript }` in session-owned memory, counted in the transient byte cap. `open_chat` validates the profile (`ready` required) and snapshots the saved locale. `send_chat` takes `{request_id, message, document?}`, builds a `ChatPacket` from the held transcript, and admits one job. On success it appends the user and assistant turns together; on failure or cancel it appends neither, so the transcript stays consistent with what the UI shows. Exceeding a bound is `context`; the server never drops old turns. `DELETE /chats/{id}` and lease end erase the chat and cancel its run.

**D4 Routes.** `POST /chats` → `{chat_id, profile, locale}`; `POST /chats/{id}/turns` → job; `GET /chats/{id}` → transcript and current job id; `DELETE /chats/{id}`. Cancel uses the existing `POST /jobs/{id}/cancel`. SSE events carry job state, progress snapshot and batched provisional text; a gap requires `resync_required` and a `GET` of chat and job.

**D5 Diagnostics.** Add content-free `chat` fields to the existing agent run records (action `chat`, turn index, counts) in the existing typed vocabulary; no new event code is needed unless the codec requires one.

**D6 Bridge.** Typed Tauri commands `open_chat`, `send_chat`, `get_chat`, `close_chat` (names avoid the capability guard's banned substring, see M17) with TypeScript validation in `src/adapters`.

**D7 UI.** A chat controller in `src/application` (reducer plus fake-bridge tests) owns transcript, running job, provisional text and chip data; the active-editor snapshot comes from the existing editor resource model, with `unsaved` taken from the buffer's dirty state. The pane follows [ux-gui](../ux-gui.md#save-review-and-transition-feedback) and uses native controls; streamed text stays out of global React state beyond the current provisional string.

## Files

| Area                                                | Change                                                                       |
| --------------------------------------------------- | ---------------------------------------------------------------------------- |
| `diary-application`                                 | `JobRunner`, `ChatService`, DTO-free types, tests                            |
| `diary-api`                                         | chat routes, job progress DTO, `agents_available` derivation, tests          |
| `diary-server`                                      | wire runner with the `Agents` registry                                       |
| `src-tauri/src/bridge*`                             | chat commands, validation, capability grants                                 |
| `src/application`, `src/adapters`, `src/components` | chat controller, validation, AI Chat pane, toolbar summary, Vitest           |
| `src-tauri/crates/*/tests`                          | `chat_conformance.rs`, live tests (env-gated, ignored by default)            |
| `scripts/smoke/agent.py`                            | chat routes against fake CLIs                                                |
| docs                                                | contracts, handoff, phase plan, roadmap, changelog, index, README, AGENTS.md |

## Sequencing and tests

1. **Stage A** — runner, busy gate, batching, cancel, lease/shutdown. Gate.
2. **Stage B** — chat service, routes, bridge, diagnostics, conformance matrix. Gate.
3. **Stage C** — UI and Vitest. Gate.
4. **Stage D** — live tests per adapter, smoke, docs, final gate.

Run affected tests while iterating and the full gate at each stage end. Live tests need `DIARY_CODEX_LIVE`, `DIARY_CLAUDE_LIVE`, `DIARY_QWEN_LIVE`, `DIARY_APPLE_LIVE` with their profile variables and spend account usage; run them once on the final code.

## Review focus

- Cancel and lease races: a late success must never append to the transcript.
- The busy gate must not be bypassed by a retry with a new `request_id`.
- Transient memory: transcripts, provisional text and replay ring all count toward the cap.
- No document text, prompt or reply in logs or error bodies.
- UI never claims progress the server did not observe.

## Risks and limits

- **Read-only status is no longer the limit:** chat is the first path that spends model usage; live tests do so by design.
- **Apple context** is small (F6); long documents will be refused there.
- **Provider retention** of chat content follows each CLI or Apple policy; the app promises only its own memory-only transcript.
- **Evidence limits:** no packaged-app, native VoiceOver or Korean IME proof here; those stay with P1-M28.
- **Contract drift:** `GET /chats/{id}` and the reply bound are additions pending O1/O2.

## Stage A record (2026-11-06)

Delivered: `Coordinator::admit_agent` (busy gate after the idempotent lookup, `Error::Busy` → 409 `busy`), `Coordinator::touch` (progress bumps a job's sequence), and `diary-application::run` (`execute` with a batched pump: 50 ms / 16 KiB, `Replace`, phase flush, cancel-wins via `Run::finish`). Tests: `second_run_is_busy`, `retry_returns_same_job`, `changed_payload_is_mismatch`, `touch_bumps_sequence_until_terminal`, `delta_batching_bounds`, `replace_supersedes`, `phase_flushes_pending_text_and_is_recorded`, `cancel_wins_over_late_success`. Gate G01–G07 passed. Design change from D1: provisional text is not carried in SSE; events only bump the job sequence and the client queries the snapshot (matches the contract's rule to rebuild from queries). Still open: C01 wiring, lease/shutdown cancel (C04), `slow_consumer_resyncs` (C03) — all in stage B with the routes.

## Completion

Stages B–D are recorded in the [evidence record](milestone-21-transient-ai-chat-overview.md#evidence-record). Deviation from D1: `agents_available` uses the profile directory's chat capability, not the startup availability list.
