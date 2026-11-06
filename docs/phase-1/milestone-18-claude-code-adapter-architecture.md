# P1-M18: Claude Code adapter work plan

Status: Done 2026-11-06

This plan adds the Claude Code adapter behind the P1-M16 `AgentRunner` port on top of the P1-M17 supervisor and profile status. Scope and checklist are in the [overview](milestone-18-claude-code-adapter-overview.md); the contracts are [agent port and profiles](../architecture/editing-and-agents.md#agent-port-and-profiles), [trust and process lifecycle](../architecture/editing-and-agents.md#trust-and-process-lifecycle), [required evidence](../architecture/editing-and-agents.md#required-evidence) and handoff E5. It follows the [M17 work plan](milestone-17-cli-supervision-codex-adapter-architecture.md) and repeats none of it.

## Findings that shaped the plan

Read from the architecture, the M17 code and the local CLI (`claude` at `~/.local/bin/claude`) on 2026-11-06. Only non-inference commands were run. The plan was drafted against 2.1.318; the review the same day found 2.1.319 installed (F9).

| #   | Finding                                                                                                                                                                                                                                                                                                                          | Decision                                                                                                                                                                                                                                                             | C item, test                                     |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| F1  | `CodexAdapter` (about 600 lines) owns the run directory, stop flags, `Active` counter, env builder, executable resolution, probe runner and `cancel_all`/`shutdown`. `ProfileService` holds an `Arc<CodexAdapter>`, and `diary-server` wires lease end to `CodexAdapter::cancel_all`. Copying it would fork the lifecycle code.  | Extract the protocol-independent part into a shared host; Codex and Claude become thin protocol modules over it (D1). Lease end and shutdown call the host once.                                                                                                     | C01, C06                                         |
| F2  | `claude auth status` exits 1 with JSON `{"loggedIn": false, ...}` for an empty `CLAUDE_CONFIG_DIR`, and 0 when logged in (the owner's default login reported `loggedIn: true`, `authMethod: claude.ai`). It also prints `configDirectory`/`projectsDirectory` paths and **creates a `backups` directory** inside the config dir. | Use `auth status` as the non-inference auth probe: parse only `loggedIn` (bool). Never log or return the JSON. Record the `backups` side effect as an evidence limit; the profile home is app-owned, not the owner's default `~/.claude` unless they configure that. | C02 `claude_check_maps_each_status`              |
| F3  | `--bare` skips keychain/OAuth ("Anthropic auth is strictly `ANTHROPIC_API_KEY`"), so it would break the owner's subscription login.                                                                                                                                                                                              | Do not use `--bare`. Isolate with `--setting-sources ""`, `--strict-mcp-config`, `--disable-slash-commands`, `--tools ""` and `--no-session-persistence` instead; the live test proves what still loads (D3).                                                        | C03 `claude_argv_disables_tools_and_persistence` |
| F4  | The CLI offers `-p`, `--output-format stream-json` (needs `--verbose`), `--json-schema <json>`, `--model`, `--effort low\|medium\|high\|xhigh\|max`, `--max-budget-usd`, `--permission-mode`, stdin prompt.                                                                                                                      | Fixed argv built only from these (D4). The schema is a code constant passed inline as one argv element (no user text), so no schema file is needed unless the live capture shows an argv-size or quoting problem.                                                    | C03                                              |
| F5  | stream-json event shapes, where structured output appears for `--json-schema`, and error/`is_error` shapes are **not verified**; no fixture exists and no inference was run.                                                                                                                                                     | A capture spike (sequencing step 2) records them from the real CLI before the parser is written. Parser code follows the fixtures, not memory.                                                                                                                       | C04, C07                                         |
| F6  | `assets/prompts/claude/` does not exist; `PromptAssets` reports `assets_missing`, and `ProfileService::view` treats only Codex as registered.                                                                                                                                                                                    | Add the two assets and register Claude in the capability mapping (D6, D7).                                                                                                                                                                                           | C08, C10                                         |
| F7  | A Claude turn can use tools (Bash, Read, Edit) by default, unlike Codex's read-only sandbox flag.                                                                                                                                                                                                                                | `--tools ""` removes them for both actions; a test asserts the flag. This is hygiene, not a confinement claim (owner decision D12 stands).                                                                                                                           | C03                                              |
| F8  | Claude CLI may print update notices or telemetry to stderr and may phone a version/update endpoint at startup.                                                                                                                                                                                                                   | Stderr is already count-only. Set `DISABLE_AUTOUPDATER=1` and `DISABLE_TELEMETRY=1` in the child env only if the live run shows they are needed; record the reason (extension of the allowlist, as M17 allows).                                                      | C02, C09                                         |
| F9  | The installed CLI moved from 2.1.318 to 2.1.319 within hours, so it self-updates. Fixtures named after a hard-coded 2.1.318 would not match what the capture records.                                                                                                                                                            | Name the fixture directory after the version the capture actually reports (`claude-<version>/`). Set `DISABLE_AUTOUPDATER=1` unconditionally (D3) so a run or probe cannot change the binary mid-session; the reason is the observed drift.                          | C02, C07 `claude_unknown_version_is_unsupported` |
| F10 | Without a settings source Claude still loads `CLAUDE.md` and memory from its working directory and ancestors, and uses its own coding system prompt. `--setting-sources ""` does not cover this.                                                                                                                                 | Run with the private run directory as cwd (as Codex does), so no project `CLAUDE.md` is found. The spike confirms nothing else loads. Do not add a hard-coded `--system-prompt`; all instructions stay in the asset (AGENTS.md prompt rule).                         | C03 `claude_runs_in_private_cwd`                 |
| F11 | Q3 says "default login", but D3 sets `CLAUDE_CONFIG_DIR=<profile_home>`. On macOS the login lives in the keychain, and its entry may be keyed by the config directory, so an explicit `~/.claude` may not find a login that an unset variable finds.                                                                             | The spike first runs `auth status` with the owner's chosen `profile_home` (no inference). If it reports `loggedIn: false` for the default directory, stop at Q5 and ask; never fall back to the keychain or copy credentials.                                        | C02, C09                                         |
| F12 | Without `--include-partial-messages` the stream carries whole assistant messages, not text deltas.                                                                                                                                                                                                                               | The spike decides: no partial flag means `Phase` events only and no `Delta`. Add the flag only if the fixtures show usable deltas and the M16 bounds hold; record the choice.                                                                                        | C04                                              |

## Owner decisions

Interview 2026-11-06, recorded in the [changelog](changelog.md) and [AGENTS.md](../../AGENTS.md).

- **Q1 — Versions. Decided: the `2.1.x` family.** Any `2.1.*` output of `claude --version` is supported; anything else is `unsupported_version`. Fixtures come from the single patch the capture reports (2.1.319 at review), so other 2.1 patches are an evidence limit, not proven; the live test re-proves the installed version.
- **Q2 — Pins. Decided: `model: claude-sonnet-4-6`, `reasoning_effort: low`.** Sent as `--model claude-sonnet-4-6 --effort low`, unchanged from the asset. If the live test rejects either value, report and ask; never fall back silently.
- **Q3 — Live test. Decided: your default login, about 5 real turns** (capture, generation, chat, cancel, one spare), agent-run, no human exit gate. Ask before exceeding 5. The capture spike's success and structured-output turns are the same turns as the live generation and chat (record once, reuse). Auth-failure (empty config dir) and rejected-pin captures are expected to fail before any model output and are not counted; if either bills a turn, count it and ask before going past 5. "Default login" depends on F11.
- **Q4 — Refactor. Decided: extract the shared host** from `CodexAdapter` with no Codex behavior change.
- **Q5 — Isolation. Decided: ignore user, project and local settings and MCP.** If that drops the login, stop and ask.
- **Q6 — Spend cap. Decided: none.** No `--max-budget-usd`; the 10-minute wall deadline bounds a run.

## Implementation outcomes

Recorded 2026-11-06 after the capture spike and live run (2 captured turns plus 3 live turns = 5, plus two zero-cost failure captures).

- **Default login (F11, Q5).** Confirmed: `claude auth status` reports `loggedIn: true` with `CLAUDE_CONFIG_DIR` unset and `false` when it is set to `~/.claude`. Owner chose to omit the variable when `profile_home` resolves to `$HOME/.claude` and set it for any other directory. No config schema change.
- **`USER` is required.** With a stripped environment the keychain login is not found until `USER` is passed (`LOGNAME` is not needed and, set together with `USER`, broke the lookup). The Claude child gets `USER` plus `DISABLE_AUTOUPDATER=1`; no other ambient variable, and no `ANTHROPIC_*`.
- **Stream shapes (F5, F12).** Recorded under `tests/fixtures/agent/claude-2.1.319/`. A generation `result` carries `structured_output` and the same JSON as `result` text; the validator receives the text. Errors arrive as a `result` with `is_error: true`, `terminal_reason: api_error`, and `api_error_status` (404 for a rejected model); auth failure is `Not logged in`. Text deltas exist only with `--include-partial-messages`, so chat passes it and generation does not.
- **Pins accepted.** `claude-sonnet-4-6` with `--effort low` ran generation, chat and cancel.
- **Fixture content.** The recorded structured output uses the synthetic id `snippet-1` so it passes the validator.
- **Not observed.** Usage-limit and context patterns are synthetic; other 2.1 patches; phone or other accounts.

## Design decisions

Per the owner decisions above.

**D1 Shared host.** Move `RunDir`, `StopFlag`, `Active`, env construction (parameterised by the home variable name and extra pairs), `select_executable`, the probe runner, `cancel_all`/`shutdown`, `sweep_stale_runs` and the `AgentRun` logging into `agent/host.rs`. `CodexAdapter` and a new `ClaudeAdapter` each hold a `Host` and implement only `check`, argv, event parsing and error mapping. `ProfileService` takes both adapters and dispatches on `AgentKind`; `diary-server` calls `cancel_all`/`shutdown` on the host once. `GENERATION_SCHEMA`, `classify` and the shared output helpers move out of `codex.rs` into the host or `output.rs` so Claude does not import from the Codex module. M17 tests keep their names and behavior.

**D2 Claude `check`.** Resolve the executable (override, else `claude` on the launch `PATH`). Missing or non-directory `profile_home` → `unavailable`. `--version` output must be `2.1.<n> (Claude Code)`, else `unsupported_version` (`failed` on error/timeout/unparsable). Then `claude auth status` with `CLAUDE_CONFIG_DIR`: parse stdout JSON for `loggedIn`; true → `ready`, false → `unauthenticated`, no valid JSON or other failure → `failed`. 5 s each through the supervisor, small output bound, `busy` while a run exists. No model turn; credential files and the keychain are never touched by the app.

**D3 Environment.** As Codex D3 with `CLAUDE_CONFIG_DIR=<profile_home>` replacing `CODEX_HOME`, omitted when `profile_home` is `$HOME/.claude` (see Implementation outcomes), plus `USER`. Add `DISABLE_AUTOUPDATER=1` always (F9). Add `DISABLE_TELEMETRY=1` only with the recorded reason from F8. The child's cwd is the private run directory (F10). No credential or `ANTHROPIC_*` variable is ever passed; an `ANTHROPIC_API_KEY` in the server environment must not reach the child (`claude_env_excludes_anthropic_variables`), so the profile's own login is what is used.

**D4 Claude `run`.** Fixed argv, flags in one function, finalised from the capture spike:

`-p --output-format stream-json --verbose --no-session-persistence --setting-sources "" --strict-mcp-config --disable-slash-commands --tools "" --permission-mode dontAsk [--json-schema <GENERATION_SCHEMA>] --model claude-sonnet-4-6 --effort low`

- The child runs in the private run directory. The rendered prompt goes on stdin; no diary or chat text appears in argv. Model and effort come from the asset unchanged.
- Generation passes the same closed schema-1 object as Codex (`GENERATION_SCHEMA`, shared) and takes the result from the terminal `result` event; chat omits `--json-schema` and takes the plain result text as a `Reply`. Exact field names are fixed by the fixtures (F5).
- Events map to `AgentEvent::Phase`/`Delta` through `Run::accept`. Unknown kinds are ignored and counted; a non-JSON record fails `invalid_output`; a missing terminal `result`, `is_error: true` or `escaped > 0` is `failed`/`invalid_output`. Only validator-approved output becomes a `Candidate`.
- Error mapping uses the shared `classify` patterns plus Claude-specific patterns recorded with the fixtures; unobserved patterns are marked synthetic as in M17.

**D5 Logging.** Same content-free `agent_run` record as Codex.

**D6 Assets.** `assets/prompts/claude/{generate-candidate,chat}.md` with `provider: claude`, `model: claude-sonnet-4-6`, `reasoning_effort: low`, and the Codex body reused with only provider-specific wording changes (no tool talk). They satisfy the existing placeholder rules; the loader test and the embedded-table test add `claude`.

**D7 Status API.** `ProfileView` marks Claude as registered; the "unchecked until checked" behavior and caching are unchanged. Qwen and Apple stay `adapter_unimplemented`/`assets_missing`. No route, bridge command or Settings change except test coverage.

**D8 Fake CLI and fixtures.** Extend the `fake_cli` example with a `claude` mode (scenarios mirroring Codex: success, split records, malformed final, auth failure, rate limit, context, flood, hang, TERM-ignoring, late output) and add a `claude` shell fake to `scripts/smoke/agent.py`. Real fixtures go under `tests/fixtures/agent/claude-<captured version>/`, redacted to kinds and structure, with the version in the path.

**D9 Live test.** `tests/claude_live.rs`, `#[ignore]` and gated by `DIARY_CLAUDE_LIVE=1` plus `DIARY_CLAUDE_LIVE_HOME=<profile_home>`; split into `live_check_and_generation`, `live_chat`, `live_cancel` so each runs alone; records version, elapsed time, whether pins and flags were accepted, and that no run directory or escaped process remains.

## Files

| Path                                                                                                       | Change                                                      |
| ---------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| `src-tauri/crates/diary-infrastructure/src/agent/{host,claude}.rs`, `codex.rs`, `profiles.rs`, `agent.rs`  | Shared host extracted, Claude adapter, dispatch.            |
| `src-tauri/crates/diary-infrastructure/tests/` (`agent_claude.rs`, `claude_live.rs`, fixtures, `fake_cli`) | New tests and fixtures; Codex suites unchanged.             |
| `src-tauri/crates/diary-server/src/` and `diary-api/src/profiles/`                                         | Wire the second adapter into profile service and lease end. |
| `assets/prompts/claude/{generate-candidate,chat}.md`, `assets/prompts/README.md`, `agent/assets.rs` tests  | Claude assets and inventory.                                |
| `scripts/smoke/agent.py`                                                                                   | Fake `claude`, embedded-asset check.                        |
| `docs/architecture/editing-and-agents.md`, handoff, phase docs, roadmap, changelog, docs index, AGENTS.md  | Record what M18 delivered and what stays pending.           |

## Sequencing and tests

1. **Owner decisions** Q1–Q6, recorded (done 2026-11-06).
2. **Capture spike** (F5, F11, F12): first `--version` and `auth status` on the chosen `profile_home` (no inference); if not logged in, stop at Q5. Then, within the Q3 budget, run the minimum real turns to record stream-json success, structured output, auth failure and a rejected flag; store redacted fixtures; finalise D4's argv and event mapping.
3. **Shared host** (D1): extract with the M17 suites as the regression net. Tests: all M17 tests unchanged (C01).
4. **Claude adapter** (D2–D5, D8): check, argv, parsing, validation, errors, cancel. Tests: C02–C07.
5. **Assets and status** (D6–D7). Tests: C08, C10.
6. **Live test and smoke** (D9): run, record the outcome (C09), extend `scripts/smoke/agent.py`.
7. **Docs** (C12), `scripts/gate.sh` once, evidence record.

If step 2 or 6 cannot run (no login, budget refused), leave C07/C09 unchecked and report the blocker rather than shipping a parser from memory.

## Review focus

- The host extraction changes no Codex behavior; M17 tests pass without edits to expectations.
- `ANTHROPIC_*` and other ambient credentials never reach the child; the app never reads the keychain or `profile_home` files.
- Nothing logs or returns prompt, response, path, `auth status` JSON or stderr.
- Isolation flags do not silently drop the login; if they do, the plan stops at Q5.
- The child cannot load a project `CLAUDE.md` or memory (private cwd) and cannot self-update during a run.
- Live-test turns stay within the Q3 budget.

## Risks and limits

- **Claude self-updates.** A patch newer than the fixtures is accepted under 2.1.x but unproven; the auto-updater is disabled for supervised children only, not for the owner's own shell use.
- **Protocol unverified until step 2.** Event and structured-output shapes may force changes to D4; the exact-version fixture is the compatibility record, not a vendor-stability promise. A CLI outside 2.1.x is `unsupported_version` until fixtures are added; other 2.1 patches are accepted but unproven.
- **`auth status` writes a `backups` directory** in the profile home and its JSON contains paths; only `loggedIn` is read.
- **Isolation flags are hygiene, not confinement.** The owner trusts the installed CLI; account terms and retention are unverified.
- **Subscription limits and cost.** Live turns spend the profile's account usage; `--max-budget-usd` bounds API-key use only if that is what the profile uses.
- **Read-only status is not enablement.** `ready` means installed, supported and logged in; no route starts a run until P1-M21/P1-M22.
