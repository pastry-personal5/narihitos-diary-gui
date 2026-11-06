# P1-M18: Claude Code adapter overview

Status: Done 2026-11-06

Add the Claude Code adapter behind the P1-M16 `AgentRunner` port, reusing the P1-M17 supervisor, profile status and asset loader, with exact-version fixtures and an agent-run live test. Chat, Generate, job store and any run-starting route stay out (P1-M21/P1-M22). The [work plan](milestone-18-claude-code-adapter-architecture.md) defines findings, owner decisions, design, files, sequencing and tests.

## Entry conditions

- P1-M16 (port, bounds, validator, capability model) and P1-M17 (supervisor, Codex adapter, profile status routes/bridge/Settings) are Done. Follow [agent port and profiles](../architecture/editing-and-agents.md#agent-port-and-profiles), [trust and process lifecycle](../architecture/editing-and-agents.md#trust-and-process-lifecycle), [required evidence](../architecture/editing-and-agents.md#required-evidence) and handoff E5.
- The [owner decisions](milestone-18-claude-code-adapter-architecture.md#owner-decisions) Q1–Q6 of 2026-11-06 are recorded: `2.1.x` allowlist, `claude-sonnet-4-6`/`low` pins, default login with about 5 live turns, shared-host extraction, settings isolation, no spend cap.

## Scope

Included: the Claude adapter (`check` with `--version` and `auth status`, `run` with a fixed argv, schema as an argument or private file as the pinned version requires, stdin prompt, stream-json events, result parsing, error mapping, validation through the M16 validator, `Run` cancellation epoch); `claude/generate-candidate.md` and `claude/chat.md` assets; sharing the process-host plumbing with the Codex adapter instead of copying it; profile status for Claude profiles through the existing routes; fixtures captured from the pinned version; an env-gated live test; embedding and fake-CLI smoke; affected docs.

Not included: chat or generation routes, job store, SSE job events, Chat or Generate UI; Qwen and Apple adapters; changing `agents_available` (stays `false`); any new dependency; confinement or retention guarantees; reading Claude credential files or the keychain; MCP, hooks, plugins, skills or tools inside runs.

## Completion checklist

Check only with recorded evidence. Planning does not complete these items.

- [x] C01 — Codex and Claude share one process host (private run directory, allowlisted environment, stop flags, cancel-all and shutdown, executable resolution, probe runner). Codex behavior and all M17 tests are unchanged. Evidence: the M17 suites pass unmodified apart from the moved constructor; `git diff --stat` shows no copy of the host.
- [x] C02 — `check` resolves the executable (override or launch `PATH`), runs `--version` and `auth status` within 5 s each with `CLAUDE_CONFIG_DIR` set to `profile_home`, never reads credential files or the keychain, never runs a model turn, and returns only `missing`, `busy`, `unsupported_version` (any version outside `2.1.x`), `unavailable`, `unauthenticated`, `failed` or `ready`. Evidence: `claude_check_maps_each_status`, `claude_unknown_version_is_unsupported`, fake CLIs for every branch.
- [x] C03 — `run` builds the fixed argv with no user text in it, sends the rendered prompt on stdin, disables all tools, MCP, slash commands, session persistence and user/project settings, and sends model and effort exactly as the asset gives them, with the private run directory as cwd and the auto-updater disabled. Evidence: `claude_argv_is_fixed_and_contains_no_user_text`, `argv_has_no_user_text_and_disables_tools` (unit), `claude_runs_in_private_cwd_with_allowlisted_env`.
- [x] C04 — Stream-json is parsed with split records and unknown event kinds ignored and counted; the terminal `result` event is required; a generation result is validated by the M16 validator and a chat result is bounded as a `Reply`. Evidence: `claude_split_records_are_reassembled`, `claude_missing_result_is_invalid_output`, `claude_malformed_structured_output_is_invalid_output`, `claude_fenced_output_is_rejected`, shared M16 generation fixtures.
- [x] C05 — Failures map to the closed error allowlist (unauthenticated → `unavailable`, usage/rate limit → `rate_limited`, context → `context`, `is_error` result or nonzero exit → `failed`), with no prompt, response, path or raw stderr in `AgentError` or any log. Evidence: `claude_errors_map_to_allowlist`, `claude_logs_carry_no_content`.
- [x] C06 — Cancel, timeout, lease end and shutdown behave as for Codex (TERM, grace, KILL, reap, escaped-descendant detection, late output dropped). Evidence: `claude_cancel_terminates_group`, `claude_late_output_after_cancel_is_dropped`, `lease_end_cancels_claude_runs`.
- [x] C07 — Captured redacted fixtures of the allowlisted version (success, structured output, auth failure, rejected model or effort; whether text deltas exist is recorded) are stored with the captured version in their path and replayed. Evidence: the fixture files and `recorded_claude_stream_parses`.
- [x] C08 — Assets `claude/generate-candidate.md` and `claude/chat.md` exist, pass the loader, reuse the Codex wording where it applies, and follow the [asset contract](../../assets/prompts/README.md). Claude capability for each action is `ready` instead of `assets_missing`. Evidence: loader tests, updated embedded-table test, `scripts/smoke/agent.py` finds both in the built binary.
- [x] C09 — An env-gated live test (`DIARY_CLAUDE_LIVE=1`, ignored by default) runs one synthetic schema-1 generation, one chat reply and one mid-run cancellation against the installed Claude Code with the configured profile, with the owner's default login within about 5 real turns, the capture turns included. It is run by the agent, not a human exit gate. Evidence: the recorded run in the evidence record; if a pin or flag is rejected, the failure and the report-and-ask outcome.
- [x] C10 — `GET /profiles` and `POST /profiles/{alias}/check` report Claude profiles with real status and capability; Settings shows them with no UI change beyond text. Evidence: API tests for a Claude profile (ready, unauthenticated, unknown alias, reload), Vitest unchanged or extended for any new text.
- [x] C11 — Nothing starts a run from an API, bridge command or UI; `agents_available` stays `false`; no dependency, `Cargo.toml` or `package*.json` change. Evidence: empty manifest/lockfile diff and the API test for `agents_available`.
- [x] C12 — Affected docs state actual behavior and evidence limits: editing-and-agents, api-and-processes if routes text changes, handoff E5 and the M18 row, prompts README inventory, phase plan, roadmap, changelog, docs index and AGENTS.md. Evidence: the updated files in the milestone diff.

## Plan-review checklist

- [x] R01 — Compare the plan with the maintained contracts, the M17 code and the local Claude Code CLI; findings are in the work plan.
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
- [x] G08 — `python3 scripts/smoke/agent.py` (embedding and real-server Claude profile status with a fake `claude`)

Run affected tests while iterating and `scripts/gate.sh` once at the end. If a command cannot run, leave its item unchecked and record the blocker. No human test is an exit gate.

## Evidence record

Delivered 2026-11-06 against the installed `claude 2.1.319`.

- C01: `agent/host.rs` holds the shared plumbing; `agent_codex`, `agent_process` and `agent_generation` pass with no expectation edits; `ClaudeAdapter` reuses `Arc<Host>`.
- C02–C06: `tests/agent_claude.rs` (19 tests, fake `claude` in `examples/fake_cli.rs`) and `claude.rs` unit tests cover every named test; `lease_end_cancels_claude_runs` goes through the shared host.
- C07: `tests/fixtures/agent/claude-2.1.319/` (success, partial chat, auth failure, rejected model) replayed by `recorded_claude_stream_parses`.
- C08: both assets embedded; loader and embedded-table tests extended; `scripts/smoke/agent.py` finds them in the built binary.
- C09: `DIARY_CLAUDE_LIVE=1 DIARY_CLAUDE_LIVE_HOME=$HOME/.claude cargo test --test claude_live -- --ignored` passed: generation 4.7 s, chat 2.8 s, cancel 0.9 s, no run directory left. Five billed turns in total (two captures, three live); the bogus-model and empty-config captures cost nothing. Pins `claude-sonnet-4-6`/`low` were accepted.
- C10: `tests/profiles_claude.rs` (ready, unauthenticated, unknown alias, cache) and the real-server smoke (`call /profiles/other/check` is `ready`).
- C11: no `Cargo.toml`, lockfile or `package*.json` change; no route starts a run; `agents_available` stays `false` (smoke).
- C12: docs updated in this diff.
- Findings beyond the plan: the default login needs `CLAUDE_CONFIG_DIR` unset (omitted for `~/.claude`) and `USER` in the child environment; see the work plan's Implementation outcomes.
- Evidence limits: one CLI patch and one account; usage-limit and context patterns are synthetic; `auth status` writes a `backups` directory in the config directory; the live run used `~/.claude`, which Claude also uses for its own project memory directory.
