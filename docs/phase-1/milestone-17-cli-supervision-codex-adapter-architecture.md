# P1-M17: CLI supervision and Codex adapter work plan

Status: Done

This plan defines the generic CLI supervisor, the Codex adapter behind the P1-M16 `AgentRunner` port and read-only profile status. Scope and checklist are in the [overview](milestone-17-cli-supervision-codex-adapter-overview.md); the contracts are [agent port and profiles](../architecture/editing-and-agents.md#agent-port-and-profiles), [trust and process lifecycle](../architecture/editing-and-agents.md#trust-and-process-lifecycle), [required evidence](../architecture/editing-and-agents.md#required-evidence) and handoff E5.

## Findings that shaped the plan

Read from the architecture, the code and the local CLI on 2026-11-06.

| #   | Finding                                                                                                                                                                                                                                | Decision                                                                                                                                                                                         | C item, test                         |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------ |
| F1  | `diary-infrastructure` already has `libc`, `serde_json`, `tempfile` (dev); only `diary-server` has `tokio`. M16 put the port, `Run`, `CancelToken`, limits and constants for the process bounds in `diary-application`.                | Supervisor and adapter are synchronous (`std::process`, `libc`, threads) in infrastructure and implement `AgentRunner`. No new dependency.                                                       | C14                                  |
| F2  | Installed `codex-cli` is 0.167.0; the research inspected 0.166.0 and no 0.166.0 binary exists to capture fixtures.                                                                                                                     | Allowlist exactly `codex-cli 0.167.0` (owner). Any other output is `unsupported_version`.                                                                                                        | C06 `unknown_version_is_unsupported` |
| F3  | `codex exec --help` (0.167.0) offers `--json`, `--output-schema`, `-o/--output-last-message`, `-C`, `--ephemeral`, `--ignore-user-config`, `--ignore-rules`, `--skip-git-repo-check`, `-s read-only`, `-m`, `-c key=value`, stdin `-`. | Fixed argv built from these flags (D3). Prompt on stdin, schema in a private file, final text read from the last-message file.                                                                   | C07                                  |
| F4  | `codex login status` with an empty `CODEX_HOME` printed `Not logged in` and exited 1; the owner's profile printed `Logged in using ChatGPT`. It is a non-inference command.                                                            | Status check uses it with the selected `CODEX_HOME` (D5). Exit 0 → ready, exit 1 with `Not logged in` → `unauthenticated`, anything else → `failed`. The app never reads login files.            | C06 `check_maps_each_status`         |
| F5  | M16 left `Candidate` fields public and said the release binary embedding was unproven because nothing linked the loader.                                                                                                               | The adapter links the loader into `diary-server` through profile listing; a smoke script greps the built binary. Make `Candidate` construction validator-only if it is cheap (see Review focus). | C13                                  |
| F6  | The API contract already names `list_profiles`/`check_profile` (`GET /profiles`, `POST /profiles/{id}/check`), and startup reports `agents_available: false` with `AgentAdapterUnverified`.                                            | Implement those two routes only. Profile id is the alias. Leave startup `agents_available` and its reason alone: no run can start yet.                                                           | C11, C14                             |
| F7  | A process-group kill misses a descendant that called `setsid`. Architecture requires native supervision or a profile-specific unsupported outcome.                                                                                     | Detect escaped descendants by process-table ancestry on macOS (D4) and report `failed`; see Q4.                                                                                                  | C04 `escaped_descendant_is_detected` |
| F8  | M16 `Run::finish` already converts success after a committed cancel into `cancelled`.                                                                                                                                                  | The adapter routes every event and the result through `Run`; the supervisor only supplies bytes and a terminal state.                                                                            | C05                                  |
| F9  | The Codex pins `gpt-5.6-luna`/`medium` came from assets unverified.                                                                                                                                                                    | Send them as `-m` and `-c model_reasoning_effort=` exactly as the asset gives them; the live test is the verification (Q5).                                                                      | C10                                  |

## Owner decisions

Interview 2026-11-06, recorded in the [changelog](changelog.md) and [AGENTS.md](../../AGENTS.md).

- **Q1 — Exposure. Decided: read-only profile status.** Routes `GET /profiles` and `POST /profiles/{alias}/check`, typed bridge commands and a read-only Settings list. No run-starting route, command or UI. The alternative was library-only.
- **Q2 — Live Codex turn. Decided: automated test, no human gate.** Develop an env-gated automated live test, run it, and record the result. No human test is an exit gate. The alternatives were an owner-run smoke gate and deferral to P1-M21.
- **Q3 — Versions. Decided: allowlist `codex-cli 0.167.0` only.**

- **Q4 — Escaped descendants. Decided: detect via the process table.** Before KILL and again after reaping, walk the macOS process table (`proc_listallpids` and `proc_pidinfo(PROC_PIDTBSDINFO)` through the existing `libc` crate) for processes whose ancestry includes the run's leader; kill survivors by PID only if their start time still matches the earlier snapshot, and report the run `failed` with a count in the log. If `libc` lacks the needed symbols, stop and ask: adding a dependency is not approved.
- **Q5 — Pins rejected by the live test. Decided: report and ask.** Leave the pins in the asset, record the failure and ask the owner what to change. The adapter never falls back silently.
- **Q6 — User config. Decided: ignore config and rules.** Runs pass `--ignore-user-config --ignore-rules`; auth still comes from `CODEX_HOME`. Not a confinement claim.
- **Q7 — Live usage. Decided: about 5 real turns** on profile `xpp8` while developing (generation, chat, cancel, plus a couple of capture or retry runs); ask before exceeding that.

## Design decisions

**D1 Layout.** `diary-infrastructure/src/agent/` gains `process.rs` (generic supervisor), `codex.rs` (adapter, argv, event parsing, error mapping) and `profiles.rs` (config profile → `ProfileHandle`, status and capability listing). `agent.rs` re-exports. Adapter flags stay in `codex.rs` only. The application crate gets no new code except a `ProfileView` value if the API needs one.

**D2 Supervisor.** `Supervisor::run(Spec, &Run, sink) -> Outcome` where `Spec { executable: PathBuf (absolute), argv: Vec<OsString>, env: Vec<(OsString, OsString)>, stdin: Vec<u8>, cwd: PathBuf, deadlines, limits }`.

- Spawn with `Command::new(executable).args(argv).env_clear().envs(env).current_dir(cwd)` and `process_group(0)`; no shell, stdin piped, stdout and stderr piped. Write stdin on its own thread, then drop it. A write error after the child exits is ignored.
- Two reader threads read stdout and stderr into a channel. Stdout is split on `\n` into records; a record over 1 MiB or total stdout+stderr over 10 MiB fails the run. Stderr is counted and discarded except the last 2 KiB kept in memory for mapping error classes by pattern, never logged or returned.
- The control loop waits on the channel and `try_wait` with 100 ms ticks, tracks time since the last byte (120 s) and total time (600 s), and polls `CancelToken`. Limits are constants from `diary-application`; tests build a `Limits` with small values.
- Termination (cancel, timeout, limit breach, shutdown): mark stopping, close stdin, `killpg(pgid, SIGTERM)`, wait up to 2 s, `killpg(pgid, SIGKILL)`, `waitpid` the leader, drain pipes for at most 1 s, then report terminal. Terminal is never reported before the leader is reaped.
- `Outcome { exit: Exit::{Success, Status(i32), Signal(i32)}, stop: Option<StopReason::{Cancelled, Inactivity, Wall, RecordTooLarge, OutputTooLarge}>, escaped: usize }`.

**D3 Environment and working directory.** The child gets only `HOME`, `PATH`, `LANG`/`LC_ALL` (from the validated locale or the server's), `TMPDIR` (the run directory), and `CODEX_HOME` set to `profile_home`. `PATH` is the executable's directory plus `/usr/bin:/bin:/usr/sbin:/sbin`; the live test confirms whether Codex needs more (for example a Node install) and the list is extended only with a recorded reason. No credential variable is ever passed. The run directory is a fresh `0700` directory created under `paths.runtime_root/agent-runs/` with a random name, holds the schema file and last-message file, and is removed best effort after the run; startup removes stale ones. It is never inside the workspace, sources root or configuration directory. Rejected otherwise.

**D4 Escaped descendants.** Per Q4. Before the TERM step, snapshot descendants of the leader (PID plus start time) by walking `proc_pidinfo`; after the leader is reaped, re-scan. Any snapshot member still alive with the same start time is killed and counted in `escaped`; a nonzero count makes the adapter return `failed` even if the model produced output. No signal is sent by PID alone. Cleanup after `SIGKILL` of the server itself is not claimed.

**D5 Codex `check`.** Resolve the executable once: the profile's `executable` override if set, else `codex` searched on the server's launch `PATH`; canonicalize to an absolute file that is executable, else `missing`. A missing or non-directory `profile_home` is `unavailable`. Run `<exe> --version` and parse `codex-cli X.Y.Z`; anything but `0.167.0` is `unsupported_version`; failure, timeout or unparsable output is `failed`. Then `<exe> login status` with `CODEX_HOME`: exit 0 → `ready`, exit 1 and `Not logged in` → `unauthenticated`, otherwise `failed`. Each command has a 5 s deadline through the same supervisor and a small output bound. `busy` is returned while a supervised run exists. No model turn is used to probe.

**D6 Codex `run`.** Fixed argv for 0.167.0 (flags live in one function):

`exec --json --ephemeral --ignore-user-config --ignore-rules --skip-git-repo-check --sandbox read-only --color never -C <run dir> --output-schema <schema file> --output-last-message <last file> -m <model> -c model_reasoning_effort="<effort>" -`

- `<model>` and `<effort>` come from the asset front matter unchanged; neither can contain user text because they are asset values, checked as non-empty single-line by the M16 loader. No diary or chat text appears in argv.
- The schema file is the closed schema-1 generation object for `Mode::Candidate`; chat runs omit `--output-schema` and read the plain last message as a `Reply`. The schema text is a code constant derived once from the validator's struct; the compatibility test checks field names against the M16 assets. Prompt text on stdin comes from `Prompt::render`.
- `--ignore-user-config` and `--ignore-rules` give deterministic runs. They are not a confinement claim: the owner trusts the installed CLI's normal access, and `CODEX_HOME` auth is still used.
- NDJSON events map to `AgentEvent::Phase`/`Delta` through `Run::accept`. Event kinds are matched against the recorded 0.167.0 fixtures; an unknown kind is ignored and counted, a record that is not JSON fails `invalid_output`. Only validated deltas reach the sink and stay provisional.
- On success the last-message file (bounded to `MAX_RESULT_BYTES`) goes to `validate_generation(bytes, snippet_count)` or the chat reply bound, then `Run::finish`. A zero exit with no last message, a missing terminal event or `escaped > 0` is `failed`/`invalid_output`.
- Error mapping reads structured error events first (`turn.failed`, `error`), then the stderr tail, into `unavailable` (unauthorized), `rate_limited`, `context`, `timeout`, `cancelled` or `failed`. Mapping uses fixed class patterns recorded with the fixtures; no raw text is stored in `AgentError`.

**D7 Logging.** Job-less for now: log at start and end `agent_run` with profile alias hash, mode, status code, duration and byte counts only, through the existing diagnostic logger. Never prompts, responses, paths or stderr.

**D8 Profile status API.** `GET /profiles` reads the current config, builds a `ProfileHandle` per profile, returns `[{alias, type, status, reason?, capabilities:{chat, generate}}]` where capability comes from the M16 `capability(...)` with `adapter_registered = true` for Codex only. Non-Codex profiles return `unavailable` and the M16 reason. Statuses are computed without checks (`status: "unchecked"` is not a `ProfileStatus`, so the list reports the last cached check or `unavailable` if none); `POST /profiles/{alias}/check` runs the check on a blocking thread, caches the result keyed by config generation and returns it. Unknown alias → `not_found`; a profile removed by reload drops its cache. Responses carry no paths or stderr. The routes need the same token and instance checks as existing routes and are listed in the API contract. The cache keeps list latency independent of the CLI.

**D9 Bridge and Settings.** `list_profiles` and `check_profile` Tauri commands with typed DTOs and the minimal capability grants; Settings shows alias, type, status text, capability and a Check button per profile, with in-progress and failure states. Wording lives in `SETTINGS_MESSAGE`-style constants; the text says status only, no Generate. Settings never blocks on a slow check.

**D10 Fake CLI and fixtures.** A small test helper executable (`tests/support` example target, no dependency) behaves by argv scenario: normal NDJSON success, split records, malformed final output, auth error, usage limit, context window, flooding stderr, silent hang, TERM-ignoring child, grandchild in group, `setsid` escapee, late output after TERM. `scripts/smoke/agent.py` uses a tiny shell fake `codex`. Fixtures of the real 0.167.0 stream and error shapes are captured by the live test from a synthetic prompt, redacted to event kinds and structure, stored under `tests/fixtures/agent/codex-0.167.0/` with the version in the file name. Fixtures are the compatibility record, not a vendor-stability promise.

**D11 Live test.** `tests/codex_live.rs`, `#[ignore]` and also requires `DIARY_CODEX_LIVE=1`. It uses the owner's existing Codex profile `xpp8` (`profile_home` `/Users/user1/.codex-pp8`, passed as `CODEX_HOME`; no login file is read or copied), a synthetic one-snippet packet, and runs: generation → validator accepts; chat → reply; cancellation mid-run → no process remains. It records version, elapsed time, whether the pins were accepted, and writes fixtures when `DIARY_CODEX_CAPTURE` is set. It is run by the agent, spends account usage, and never runs in the gate.

## Files

| Path                                                                                                                                          | Change                                                  |
| --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| `src-tauri/crates/diary-infrastructure/src/agent/{process,codex,profiles}.rs`, `agent.rs`                                                     | New supervisor, adapter, profile listing.               |
| `src-tauri/crates/diary-infrastructure/tests/` (supervisor, codex adapter, `codex_live`, fixtures, fake)                                      | New tests and fixtures.                                 |
| `src-tauri/crates/diary-api/src/` (routes, state, tests)                                                                                      | `GET /profiles`, `POST /profiles/{alias}/check`, cache. |
| `src-tauri/src/bridge.rs`, command registration, capabilities                                                                                 | `list_profiles`, `check_profile`.                       |
| `src/components/SettingsView.tsx` and tests, application model                                                                                | Read-only profile list and check action.                |
| `scripts/smoke/agent.py`, `scripts/gate.sh` unchanged                                                                                         | Embedding and real-server profile status smoke.         |
| `docs/architecture/{editing-and-agents,api-and-processes}.md`, handoff, prompts README, phase docs, roadmap, changelog, docs index, AGENTS.md | Record what M17 delivered and what stays pending.       |

## Sequencing and tests

1. **Supervisor** (D2–D4): spawn, drain, bounds, deadlines, termination, escape detection with the fake helper. Tests: C01–C04.
2. **Codex adapter** (D5–D7): check, argv, stream parsing, validation, error mapping, cancel epoch against the fake. Tests: C05–C08.
3. **Live capture** (D10–D11): run the live test, capture and redact fixtures, add replay tests (C09) and record the pin outcome (C10).
4. **Profile status** (D8–D9): API routes and cache, bridge commands, Settings list. Tests: C11–C12.
5. **Embedding smoke** (C13) and **docs** (C15); run `scripts/gate.sh` and the smoke once; fill the evidence record.

Steps 1–2 depend on nothing external; step 3 needs the installed, logged-in Codex and spends usage; if it cannot run, leave C09/C10 unchecked and report the blocker.

## Review focus

- Terminal state is never reported before the leader is reaped and pipes are drained or abandoned within 1 s.
- Nothing logs or returns prompt, response, path or stderr; `AgentError` stays content-free.
- The environment is an allowlist built from scratch, not filtered from the server's.
- Escape detection never signals by PID alone.
- Whether to make `Candidate` constructible only through the validator (M16 known gap) is a small change worth taking here.

## Risks and limits

- **Live evidence depends on one installed version and the owner's account.** Fixtures prove 0.167.0 only; any update makes the profile `unsupported_version` until fixtures are added. Account terms, retention and tool access remain the owner's trusted-CLI choice.
- **Unverified pins until C10 runs.** `gpt-5.6-luna`/`medium` may be rejected; see Q5.
- **Escape detection is best effort.** It sees descendants alive at scan time; a process that detaches and exits its ancestry chain before the scan can be missed. SIGKILL of the server itself leaves cleanup unclaimed.
- **Allowlisted `PATH` may be too small** for the CLI's own launcher; extend only with a recorded reason.
- **Read-only status is not enablement.** `ready` means installed, supported version and logged in, not that a candidate will validate; no route starts a run until P1-M21/P1-M22.
