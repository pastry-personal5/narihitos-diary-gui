# P1-M22: Candidate generation overview

Status: Active (plan reviewed; Stage A started 2026-11-06)

Add Generate: one to five sequential, independently validated candidates from eligible selected source snapshots, bound to an immutable date, locale, profile and saved Authentic baseline. It reuses the P1-M21 job layer (`Coordinator::admit_agent`, `run::execute`, the chat service pattern). The [work plan](milestone-22-candidate-generation-architecture.md) defines findings, owner decisions, design, files, sequencing and tests.

## Entry conditions

- P1-M16 (`GenerationPacket`, `Style`, generation validator, `generate-candidate` assets), P1-M17–P1-M20 (four adapters, one registry) and P1-M21 (job layer, chat, AI Chat pane) are Done.
- Follow [context, output and limits](../architecture/editing-and-agents.md#context-output-and-limits), the [background AI contract](../architecture/ai-job-experience.md), the [API](../architecture/api-and-processes.md) (`generate_candidates`, `read_candidate`) and handoff E3/E5/E11/E13.
- [Owner decisions](milestone-22-candidate-generation-architecture.md#owner-decisions) Q1–Q5 and O1–O2 of 2026-11-06 are recorded.

## Scope

Included:

- **Stage A, service and setting:** generation service (snapshot capture, eligibility, frozen context, sequential loop inside one job, independent outcomes, transient candidate store with its own cap, discard) and the persisted `generation.count` user setting.
- **Stage B, routes and bridge:** `POST /generations`, `GET /generations/{job}`, `GET /candidates/{id}`, discard; extended progress snapshot; typed bridge commands and capability grants.
- **Stage C, UI:** Generate button and disabled reasons, dirty-source dialog (Save, Exclude, Cancel), read-only candidate cards for the selected date with provisional text, completion message, Cancel, stale-baseline label, count control in Settings.
- **Stage D, proof and docs:** fake-child matrix, env-gated live candidate plus cancel per adapter, smoke extension, affected docs.

Not included: review, accept or reject (P1-M24); the cross-date results badge, off-date routing and reconnect discovery (P1-M23); persistent candidates; macOS notifications; a new Cargo or npm dependency; any change to adapter argv, pins, fixtures or prompt assets.

## Completion checklist

Check only with recorded evidence. Planning does not complete these items.

### Stage A — service and setting

- [x] C01 — Admission rejects an empty or fully ineligible selection before any inference; timestamp-conflict or unknown-date snippets are ineligible; the server re-checks that each selected source's bytes are current at capture. Evidence: `empty_selection_rejected`, `ineligible_snippets_excluded`, `changed_source_refused`.
- [x] C02 — The job freezes date, locale, profile, count, snapshots and baseline (`absent` or `{sequence, sha256}`); later checkbox, re-import, setting or other-date edits never retarget it. Evidence: `context_is_frozen`, `absent_baseline_allowed`.
- [x] C03 — Packets order snippets deterministically, label them `snippet-N`, omit paths, front matter, location and audio, and use the styles standard, concise, reflective, chronological, thematic in order; one model call per candidate, no hidden retry or fallback. Evidence: `packet_order_and_labels`, `styles_in_order`, `one_call_per_candidate`.
- [x] C04 — A failed candidate does not remove earlier ones and the loop continues; the job succeeds with partial counts if at least one candidate is valid and fails if none is; the reservation spans the whole loop including the gaps. Evidence: `partial_failure_keeps_ready`, `zero_valid_is_failure`, `busy_between_candidates`.
- [x] C05 — Cancel keeps finished candidates, labels the interrupted candidate and unstarted slots separately, and drops late results; lease end, close and shutdown cancel the run. Evidence: `cancel_keeps_ready_and_counts_slots`, `late_result_dropped`, `lease_end_cancels_generation`.
- [x] C06 — The candidate store is session-owned, capped at 5 jobs and 8 MiB per session and counted in the transient byte cap; a full store refuses admission with a code that asks for explicit discard; discard removes only that transient result, leaves a content-free admission record and never reruns inference. Evidence: `store_cap_refuses_admission`, `discard_keeps_admission_record`, `retry_after_discard_is_unavailable`.
- [x] C07 — `generation.count` is a validated integer 1–5, default 3, saved through the guarded section write; an invalid section degrades alone and keeps its raw data. Evidence: `generation_defaults_and_bounds`, `generation_write_conflicts`, `generation_invalid_section_preserved`, `generation_newer_schema_untouched`.

### Stage B — routes and bridge

- [ ] C08 — `POST /generations`, `GET /generations/{job}` (manifest of IDs and statuses, no bodies), `GET /candidates/{id}` and discard exist; retry with the same `request_id` returns the same job before the busy check, and a changed payload is a mismatch. Evidence: API tests per route and case.
- [ ] C09 — The progress snapshot adds origin date, baseline, candidate index and the requested, finished, ready, failed, cancelled and not-started counts plus the `between_candidates` phase; it holds no bodies or raw diagnostics. Evidence: snapshot tests and a body-scan test.
- [ ] C10 — A saved Authentic revision that differs from the frozen baseline reports `stale_baseline`; candidates stay readable. Evidence: `stale_baseline_reported`.
- [ ] C11 — Errors map to the closed API codes with no model text; generation records are typed and content-free. Evidence: `failure_reports_a_code_and_no_text` and seeded-string log scans.
- [ ] C12 — Bridge commands `start_generation`, `get_generation`, `read_candidate`, `discard_generation` use minimum capabilities and typed validation, avoid names containing `file`, and cancel reuses the job command. Evidence: bridge tests and the capability guard test.

### Stage C — UI

- [ ] C13 — Generate responds within one frame with a submitting state and is disabled with a visible reason for no selection, ineligible selection, active run (naming its date or chat) or no ready profile. Evidence: Vitest per state with a fake bridge and fake clock.
- [ ] C14 — Dirty selected sources open a dialog with Save, Exclude and Cancel; nothing is sent until resolved; Authentic is flushed before the baseline binds. Evidence: Vitest per choice.
- [ ] C15 — Candidate cards are read-only, marked provisional until final validation, show the original date, profile, count and per-candidate status, never focus or move the caret, and keep scroll with a New output affordance; they survive date changes in the session. Evidence: Vitest `provisional_then_final`, `cards_survive_date_change`.
- [ ] C16 — Completion issues one polite message such as `2 candidates ready; 1 failed`; Cancel is keyboard-operable; stale candidates show `Entry changed — regenerate to review`. Evidence: Vitest and accessibility queries; native VoiceOver proof is not required.
- [ ] C17 — The Settings count control reads and saves `generation.count` and a new request uses the saved value. Evidence: Vitest and a settings round trip.

### Stage D — proof and docs

- [ ] C18 — A generation conformance matrix runs over all four kinds with fake children: success, partial failure, cancel mid-candidate, timeout, flood, late result, retry without rerun, stale baseline, changed source, no content in errors. Evidence: `diary-api` generation tests and the M20 matrix in generation mode.
- [ ] C19 — Env-gated, agent-run live tests produce one real candidate and one cancel per adapter through the built server with synthetic snippets, recording versions and timing. No human gate. A kind unavailable here stays unchecked with the exact reason.
- [ ] C20 — `scripts/smoke/agent.py` covers generation refusals, admission, manifest, candidate read and discard with fake CLIs; no review route or UI exists; no `Cargo.lock` or `package*.json` change.
- [ ] C21 — Affected docs state actual behavior and evidence limits: editing-and-agents, api-and-processes, ai-job-experience, user-settings, logging, handoff E3/E5/E11/E13 and the M22 row, phase plan, roadmap, changelog, docs index, README, AGENTS.md.

## Plan-review checklist

- [x] R01 — Compare the plan with the maintained contracts and the M16–M21 code; record findings in the work plan.
- [x] R02 — Map each finding to a C item and a named test.
- [x] R03 — Format the changed plans, check local links and whitespace, and review the scoped diff.

## Implementation gate

Run G01–G07 at the end of each stage; a stage does not start while the previous gate is red. G08 is the final integration gate after the generation routes and smoke extension exist (Stage D).

- [x] G01 — `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [x] G02 — `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [x] G03 — `cargo test --manifest-path src-tauri/Cargo.toml`
- [x] G04 — `npx prettier --check .`
- [x] G05 — `npx eslint . --max-warnings 0`
- [x] G06 — `npx vitest run`
- [x] G07 — `npx tsc --noEmit`
- [ ] G08 — `python3 scripts/smoke/agent.py` extended for generation with fake CLIs

If a command cannot run, leave its item unchecked and record the blocker. No human test is an exit gate.

## Evidence record

2026-11-06 — Plan review R01–R03 committed as `205291d`. Stage A started with C07: the client loader accepts `generation: {count}` in schema 1, supplies default 3 without writing an absent file, validates integers 1–5, and exposes `generation_count` through the typed settings snapshot. The existing guarded settings command accepts an explicit generation section update. No count control or generation action is enabled yet.

Evidence: test-runner subagent passed all 27 client `user_settings` tests (including the four named C07 tests) and all 19 adapter tests. Tests cover fresh-store persistence, bounds and malformed payloads, stale writers, invalid-section preservation/replacement, newer-schema refusal and response validation. G01–G07 passed on this slice: the Rust suite needed an unsandboxed rerun for localhost socket fixtures (595 passed, 16 ignored; `/tmp/p1-m22-g03.log`). G08 remains unchecked because generation smoke coverage does not exist yet. Changed-doc local files/anchors and `git diff --check` passed. This was the initial C07 slice; the Stage A service evidence below supersedes its incomplete-service status.

2026-11-06 — Stage A service implemented in `diary-application::generation`: fresh import-bound capture, frozen context, deterministic packets, sequential slots under one coordinator job, shared cancellation across gaps, independent outcomes and a session-owned store. The store reserves remaining capacity before inference, keeps at most five jobs/8 MiB, shrinks reservations after terminal acknowledgement and releases content charges on explicit discard while retaining request/job metadata. The API/lease hooks follow in Stage B; no run-starting generation surface exists yet.

Affected evidence: 58 application tests and workspace Clippy passed via the test runner. Named generation tests group the C01–C06 scenarios: `empty_selection_rejected_and_ineligible_snippets_excluded`, `changed_source_refused_wrong_import_date_refused_stale_config_refused`, `packet_order_and_labels_context_is_frozen_absent_baseline_allowed`, `styles_in_order_and_one_call_per_candidate`, `partial_failure_keeps_ready_and_zero_valid_is_failure`, `cancel_keeps_ready_and_counts_slots_late_result_dropped`, `busy_between_candidates_and_cancel_between_candidates`, `lease_end_cancels_generation`, `store_cap_refuses_admission_and_discard_keeps_admission_record`, `discard_releases_content_bytes`, `output_budget_failure_keeps_ready`, and `shared_transient_cap_charges_generation_and_preserves_metadata`. Pump tests additionally bound a multibyte replacement by UTF-8 bytes and suppress a pending preview after cancellation. Stage A G01–G07 passed (609 Rust tests, 16 ignored; 228 Vitest tests across 29 files; logs `/tmp/p1-m22-stage-a/`). Stage A is complete; Stage B follows.

### Limits

No packaged-app proof; candidates are not recoverable after a server restart by design; provider retention follows each CLI or Apple policy; Apple's 4096-token context refuses long selections as `context`; Qwen candidates are slow (about a minute each).
