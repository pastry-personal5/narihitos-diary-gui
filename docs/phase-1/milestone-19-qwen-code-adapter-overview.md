# P1-M19: Qwen Code adapter overview

Status: Done 2026-11-06

Add the Qwen Code adapter behind the P1-M16 `AgentRunner` port, reusing the P1-M17 supervisor, the P1-M18 shared host, profile status and the asset loader, with exact-version fixtures and an agent-run live test. Chat, Generate, job store and any run-starting route stay out (P1-M21/P1-M22). The [work plan](milestone-19-qwen-code-adapter-architecture.md) defines findings, owner decisions, design, files, sequencing and tests.

## Entry conditions

- P1-M16 (port, bounds, validator, capability model), P1-M17 (supervisor, Codex adapter, profile status routes/bridge/Settings) and P1-M18 (shared host, Claude adapter) are Done. Follow [agent port and profiles](../architecture/editing-and-agents.md#agent-port-and-profiles), [trust and process lifecycle](../architecture/editing-and-agents.md#trust-and-process-lifecycle), [required evidence](../architecture/editing-and-agents.md#required-evidence) and handoff E5.
- The [owner decisions](milestone-19-qwen-code-adapter-architecture.md#owner-decisions) Q1–Q6 of 2026-11-06 are recorded: `ready` after the version check, `0.26.x` allowlist, `qwen3.6:35b-a3b` model-only pin, agent-run live test with no turn cap, `--safe-mode` isolation (amended from `--bare` by the spike), always-set `QWEN_HOME`.
- The 2026-11-06 preliminary spike closed the open items (`--safe-mode` isolation, stdin-only prompt); see the [findings](milestone-19-qwen-code-adapter-architecture.md#findings-that-shaped-the-plan).

## Scope

Included: the Qwen adapter (`check` with `--version` only, `run` with a fixed argv, stdin prompt, stream-json events, result parsing, error mapping, validation through the M16 validator, `Run` cancellation epoch); `qwen/generate-candidate.md` and `qwen/chat.md` assets; reuse of the shared host; profile status for Qwen profiles through the existing routes; fixtures captured from the installed version; an env-gated live test; embedding and fake-CLI smoke; affected docs, including the `ready` wording.

Not included: chat or generation routes, job store, SSE job events, Chat or Generate UI; the Apple adapter; changing `agents_available` (stays `false`); any new dependency; confinement or retention guarantees; reading `settings.json`, credentials or memories; passing a key on argv; writing the profile's settings (including an effort pin); MCP, hooks, extensions, skills or tools inside runs.

## Completion checklist

Check only with recorded evidence. Planning does not complete these items.

- [x] C01 — The Qwen adapter reuses the shared `Host` (private run directory, allowlisted environment, stop flags, cancel-all and shutdown, executable resolution, probe runner). Codex and Claude behavior and all M17/M18 tests are unchanged. Evidence: the M17/M18 suites pass unmodified apart from the `ProfileService::new` signature; `git diff --stat` shows no copy of host code.
- [x] C02 — `check` resolves the executable (override or launch `PATH`), runs `--version` within 5 s with `QWEN_HOME` set to `profile_home`, never reads settings, credentials or memories, never runs a model turn, and returns only `missing`, `busy`, `unsupported_version` (any version outside `0.26.x`), `unavailable`, `failed` or `ready`. Evidence: `qwen_check_maps_each_status`, `qwen_unknown_version_is_unsupported`, fake CLIs for every branch.
- [x] C03 — `run` builds the fixed argv with no user text and no key in it, sends the rendered prompt on stdin, disables tools (`--max-tool-calls 0`), recording and profile customizations (`--safe-mode`), and sends the model exactly as the asset gives it, with the private run directory as cwd and an environment limited to the allowlist, `QWEN_HOME` and `QWEN_TELEMETRY_ENABLED=0`. Evidence: `qwen_argv_is_fixed_and_contains_no_user_text`, `qwen_argv_isolation_flags`, `qwen_runs_in_private_cwd_with_allowlisted_env`, `qwen_env_excludes_ambient_credentials`.
- [x] C04 — Stream-json is parsed with split records and unknown event kinds ignored and counted; the terminal result is required; a generation result is validated by the M16 validator and a chat result is bounded as a `Reply`. Evidence: `qwen_split_records_are_reassembled`, `qwen_missing_result_is_invalid_output`, `qwen_malformed_structured_output_is_invalid_output`, `qwen_fenced_output_is_rejected`, `qwen_exit_55_without_result_is_failed`, `qwen_init_model_mismatch_is_unavailable`, `qwen_init_version_drift_is_unavailable`, shared M16 generation fixtures.
- [x] C05 — Failures map to the closed error allowlist (endpoint or credential failure → `unavailable`, rate limit → `rate_limited`, context → `context`, exit 55, error result or nonzero exit → `failed`), with no prompt, response, path, endpoint or raw stderr in `AgentError` or any log. Evidence: `qwen_errors_map_to_allowlist`, `qwen_logs_carry_no_content`.
- [x] C06 — Cancel, timeout, lease end and shutdown behave as for Codex and Claude (TERM, grace, KILL, reap, escaped-descendant detection, late output dropped). Evidence: `qwen_cancel_terminates_group`, `qwen_late_output_after_cancel_is_dropped`, `lease_end_cancels_qwen_runs`.
- [x] C07 — Captured redacted fixtures of the allowlisted version (success, structured output, auth failure from an empty home, exit 55 without a result; no rejected-model case exists because unknown models fall back; whether text deltas exist is recorded) are stored with the captured version in their path and replayed. Evidence: the fixture files, `recorded_qwen_stream_parses`, `qwen_fixtures_are_redacted`.
- [x] C08 — Assets `qwen/generate-candidate.md` and `qwen/chat.md` exist, pass the loader, reuse the Claude wording where it applies, and follow the [asset contract](../../assets/prompts/README.md). Qwen capability for each action is `ready` instead of `assets_missing`. Evidence: loader tests, updated embedded-table and capability tests, `scripts/smoke/agent.py` finds both in the built binary.
- [x] C09 — An env-gated live test (`DIARY_QWEN_LIVE=1`, ignored by default) runs one synthetic schema-1 generation, one chat reply and one mid-run cancellation against the installed Qwen Code with the configured profile and its local endpoint. It is run by the agent, not a human exit gate, and records versions before and after, timing, the accepted pin and flags, and profile-home writes. Evidence: the recorded run in the evidence record; if the endpoint is down or a pin or flag is rejected, the failure and the report-and-ask outcome.
- [x] C10 — `GET /profiles` and `POST /profiles/{alias}/check` report Qwen profiles with real status and capability; Settings shows them with no UI change beyond text. Evidence: API tests for a Qwen profile (ready, unsupported version, missing home, unknown alias, reload), Vitest unchanged or extended for any new text.
- [x] C11 — Nothing starts a run from an API, bridge command or UI; `agents_available` stays `false`; no dependency, `Cargo.toml` or `package*.json` change. Evidence: empty manifest/lockfile diff and the API test for `agents_available`.
- [x] C12 — Affected docs state actual behavior and evidence limits, including that Qwen `ready` means installed and supported only: editing-and-agents (table row and status paragraph), api-and-processes if routes text changes, handoff E5 and the M19 row, prompts README inventory, phase plan, roadmap, changelog, docs index and AGENTS.md. Evidence: the updated files in the milestone diff.

## Plan-review checklist

- [x] R01 — Compare the plan with the maintained contracts, the M17/M18 code and the installed Qwen Code CLI and bundle; findings are in the work plan (F2 and F3 corrected the planning interview).
- [x] R02 — Map each finding to a C item and a named test.
- [x] R03 — Format the changed plans, check local links and whitespace, and review the scoped diff.

## Implementation gate

- [x] G01 — `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [x] G02 — `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [x] G03 — `cargo test --manifest-path src-tauri/Cargo.toml`
- [x] G04 — `npx prettier --check .`
- [x] G05 — `npx eslint . --max-warnings 0`
- [x] G06 — `npx vitest run`
- [x] G07 — `npx tsc --noEmit` (additional project check)
- [x] G08 — `python3 scripts/smoke/agent.py` (embedding and real-server Qwen profile status with a fake `qwen`)

Run affected tests while iterating and `scripts/gate.sh` once at the end. If a command cannot run, leave its item unchecked and record the blocker. No human test is an exit gate.

## Evidence record

Delivered 2026-11-06 against the installed `qwen 0.26.4` and the owner's local endpoint.

- C01: `QwenAdapter` holds `Arc<Host>`; the Codex and Claude suites (`agent_codex`, `agent_claude`, `agent_process`, `agent_generation`) pass with no expectation edits; the only edits to existing tests are the `ProfileService::new` signature and the flipped Qwen capability assertion.
- C02–C06: `tests/agent_qwen.rs` (23 tests, fake `qwen` mode in `examples/fake_cli.rs`) and `qwen.rs` unit tests cover every named test, including `qwen_exit_55_without_result_is_failed`, `qwen_init_model_mismatch_is_unavailable` and `qwen_init_version_drift_is_unavailable`; `lease_end_cancels_qwen_runs` goes through the shared host.
- C07: `tests/fixtures/agent/qwen-0.26.4/` (success generation, success chat with deltas, no-auth error, exit 55 without result) replayed by `recorded_qwen_stream_parses`; `qwen_fixtures_are_redacted` guards paths, endpoint and session ids.
- C08: both assets embedded; loader, embedded-table and capability tests extended; `scripts/smoke/agent.py` finds both in the built binary.
- C09: `DIARY_QWEN_LIVE=1 DIARY_QWEN_LIVE_HOME=$HOME/.qwen cargo test --test qwen_live -- --ignored` passed on the final code: generation 3.6 s, chat 2.2 s, cancel 0.8 s, version 0.26.4 before and after, no run directory left. Earlier live attempts failed and led to the findings F17 and F18 in the work plan.
- C10: `tests/profiles_qwen.rs` (ready, unsupported version, missing home, unknown alias, cache) and the real-server smoke (`/profiles/third/check` is `ready`).
- C11: no `Cargo.toml`, lockfile or `package*.json` change; no route starts a run; `agents_available` stays `false` (smoke).
- C12: docs updated in this diff.
- Findings beyond the plan: Qwen's `--json-schema` makes the CLI reject a plain-text answer, so the generation asset must tell the model to call `structured_output`; the first live run showed a cold local model taking over 100 s.
- Evidence limits: one CLI patch, one machine and one local endpoint; `ready` does not prove the endpoint is up; endpoint-down, rate-limit and context patterns are synthetic; the model pin is advisory (guarded by the init record); a local model can still answer in plain text (6 of 6 and 2 of 3 spike runs succeeded), which surfaces as `failed` for that candidate and is not retried here; runs write usage records into the profile home.
