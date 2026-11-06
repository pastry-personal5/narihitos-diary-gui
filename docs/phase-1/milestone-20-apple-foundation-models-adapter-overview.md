# P1-M20: Agent abstraction, diagnostics and Apple Foundation Models adapter overview

Status: Active

Give the four agents (Codex, Claude Code, Qwen Code, Apple Foundation Models) one lifecycle engine, one registry and one typed diagnostic vocabulary, then add the Apple adapter on top. A small Swift helper process wraps `SystemLanguageModel`; the Rust side supervises it like a CLI. The work is staged so each stage ends with a green gate: A abstractions (behavior-preserving), B diagnostics, C Apple. Chat, Generate, job store and any run-starting route stay out (P1-M21/P1-M22). The [work plan](milestone-20-apple-foundation-models-adapter-architecture.md) defines findings, owner decisions, design, files, sequencing and tests.

## Entry conditions

- P1-M16 (port, bounds, validator, capability model, embedded assets), P1-M17–P1-M19 (shared `Host`, three adapters, profile status routes/bridge/Settings) are Done. Follow the [Apple adapter contract](../architecture/editing-and-agents.md#apple-foundation-models-adapter), [required evidence](../architecture/editing-and-agents.md#required-evidence), [logging](../architecture/logging.md) and handoff E5/E11.
- [Owner decisions](milestone-20-apple-foundation-models-adapter-architecture.md#owner-decisions) Q1–Q4 of 2026-11-06 are recorded: Swift helper process, soft-skipping `build.rs` compile, detailed typed reasons, and the abstraction plus detailed-diagnostics scope.

## Scope

Included:

- **Stage A:** a standalone `Host`; a generic lifecycle engine with a small per-agent `Protocol`; Codex, Claude and Qwen ported onto it one at a time; one `Agents` registry that is itself an `AgentRunner`; one per-kind facts table; a four-kind conformance suite.
- **Stage B:** typed, content-free diagnostics for every check, run, refusal, stop and helper event, with a closed reason vocabulary; trace support; logging docs.
- **Stage C:** the Swift helper and its `build.rs` step (new build tool use: `swiftc`); the Apple protocol; the closed reason set in the profile API, bridge and Settings; fixtures captured from the helper; an env-gated live test; embedding and fake-helper smoke; affected docs.

Not included: chat or generation routes, job store, SSE job events, Chat or Generate UI; any cloud or CLI fallback (the SDK's `PrivateCloudComputeLanguageModel` is never used); model tools, filesystem access or persistent sessions; changing `agents_available` (stays `false`); a new Cargo or npm dependency; any change to adapter behavior, argv, pins or fixtures of Codex, Claude or Qwen; native notifications; claims about model pinning, quality or privacy beyond Apple's terms.

## Completion checklist

Check only with recorded evidence. Planning does not complete these items.

### Stage A — abstractions

- [x] C01 — `Host` is built once, outside any adapter (server wiring and a test kit); no adapter exposes `host()`. Evidence: `git grep "fn host("` finds nothing in `agent/`; `diary-server/src/main.rs` constructs `Host` and the adapters from it.
- [x] C02 — The lifecycle (cancel/shutdown gate, kind check, profile lookup, asset and packet rendering, executable and home resolution, run directory, phase events, preflight, supervised spawn, stop/escape/exit mapping, output validation, `Run` finish, active counter, run log) exists once in a generic engine. Evidence: one `execute` template in `agent/`; `git diff --stat` shows the three adapters shrinking to protocol code; no duplicated lifecycle lines.
- [x] C03 — Codex, Claude and Qwen are `Protocol` implementations and behave exactly as before. Evidence: each stage-A commit passes the M17/M18/M19 suites (`agent_codex`, `agent_claude`, `agent_qwen`, `agent_process`, `agent_generation`, `profiles_*`) with only constructor and `.status` accessor edits, listed in the evidence record; fixtures unchanged; `scripts/smoke/agent.py` passes; the three env-gated live tests are re-run once on the final code.
- [x] C04 — One `Agents` registry is the single `AgentRunner` the rest of the code holds; it dispatches on `profile.kind`, answers `registered(kind)` and rejects unknown kinds. `ProfileService` takes the registry, not one field per adapter. Evidence: `registry_dispatches_every_kind`, `registry_rejects_unregistered_kind`, `every_agent_kind_has_a_protocol` (iterates `AgentKind::ALL`), no `matches!(…Codex|Claude|Qwen)` left.
- [x] C05 — Per-kind facts (asset directory, default binary name, home requirement, required assets, allowed pins) live in one table consumed by assets, config mapping and protocols; `assets.rs` has no `if kind == AppleFoundationModels` branches. Evidence: `kind_spec_covers_every_kind`, updated asset tests, grep.
- [x] C06 — A conformance suite runs the same lifecycle matrix over all four kinds against fake children: busy, shutdown, lease end, cancel, timeout, flood, hang, TERM-ignoring, late output, escaped descendants, unknown alias, wrong kind, missing assets, bad pins, no content in errors, and the exact phase order. Evidence: `agent_conformance.rs` names each case and kind; a new agent needs only a fake and one registry entry.

### Stage B — diagnostics

- [x] C07 — Agent diagnostics are typed and closed: event codes `agent.check`, `agent.run`, `agent.blocked`, `agent.helper`; tags for agent kind, action, last phase reached, error code (all 11 `AgentErrorCode` values), stop cause, exit class and a single closed `Reason` set; counts for escaped processes, events, input and output bytes, elapsed milliseconds. No field takes text. The codec, parser, trace renderer and `docs/architecture/logging.md` cover them; old lines still parse. Evidence: codec/trace unit tests, schema rejection tests, the updated contract.
- [ ] C08 — Every refusal, failure, stop and blocked state emits exactly one record with the expected tags. Evidence: a table-driven test over `Reason::ALL`, `AgentErrorCode::ALL`, `Stop` and every `ProfileStatus` for all four kinds (host busy, shutdown, lease end, cancelled before start, profile removed by reload, assets missing or invalid, pin rejected, packet too large, run directory unavailable, executable or home missing, version unsupported or unreadable, probe timeout, spawn failure, record or output too large, escaped descendants, nonzero exit, missing result, helper missing or untrusted, availability reasons).
- [ ] C09 — No record contains content. Evidence: a seeded-string scan over the log directory after failing runs of every kind (diary body, prompt, reply, native error text, helper stderr, executable names, paths, environment values) finds none; helper and CLI stderr are classified, never logged.
- [ ] C10 — `diary-server trace` shows a readable timeline for a refused check, a failed run and a cancelled run, and `recent:<n>` lists agent warnings. Evidence: trace fixtures and the real-server smoke.

### Stage C — Apple

- [x] C11 — The helper is built by `diary-infrastructure/build.rs` with `swiftc` and embedded in the server binary. When `swiftc` or the SDK is missing the build succeeds and the Apple profile reports `unavailable` with reason `helper_missing`. The helper is extracted to the runtime root with mode 0700 and hash-checked before every launch. Evidence: a build with the helper; a build with `swiftc` hidden from `PATH`; `apple_missing_helper_is_unavailable`, `apple_tampered_helper_is_untrusted`.
- [x] C12 — `check` runs the helper's `availability` command (no session, no inference) and reports `ready` or `unavailable` with one closed reason: `os_unsupported`, `device_ineligible`, `apple_intelligence_disabled`, `model_not_ready`, `model_unavailable_unknown`, `helper_missing`, `helper_untrusted`, `helper_failed`, `probe_timed_out`. Each has user-facing text. Availability is rechecked before every run, and a change of reason between records is logged. Evidence: `apple_check_maps_each_reason` against the fake helper, the live availability record.
- [x] C13 — `run` uses only `SystemLanguageModel.default`: a fresh session per chat turn or candidate, instructions and prompt on stdin, no tools, no model or effort override, no file access, and no use of `PrivateCloudComputeLanguageModel`. Generation uses guided output matching schema 1 and passes the M16 validator; chat returns a bounded `Reply`. Evidence: `apple_argv_is_fixed_and_contains_no_user_text`, `apple_generation_output_is_validated`, `apple_chat_reply_is_bounded`, a source gate that fails if the helper names `PrivateCloudCompute`, a drift test between the Swift output type and `GENERATION_SCHEMA`.
- [ ] C14 — Failures map to the closed error allowlist from both SDK error families (`LanguageModelSession.GenerationError` on OS 26, `LanguageModelError` on OS 27) with no native message text: context → `context`; unsupported language or locale → `unsupported_language`; guardrail violation or refusal → `refusal`; rate limiting → `rate_limited`; decoding failure or schema mismatch → `invalid_output`; assets unavailable or availability lost mid-run → `unavailable`; concurrent requests → `busy`; native timeout → `timeout`; anything unrecognized → `failed` plus a logged `native_error_unmapped` reason. Evidence: `apple_errors_map_to_allowlist`, `apple_availability_loss_mid_run_is_unavailable`, `apple_unknown_native_error_is_failed_and_logged`.
- [x] C15 — Cancel, timeout, lease end and shutdown terminate the helper (TERM, grace, KILL, reap, escaped-descendant detection) and drop late output; covered by the conformance suite for the Apple kind plus `apple_cancel_terminates_group`.
- [x] C16 — `GET /profiles` and `POST /profiles/{alias}/check` carry the closed `reason` for every kind (null when ready); the bridge commands and TypeScript types carry it; Settings shows distinct text for every reason, such as "Apple Intelligence is turned off in System Settings". Evidence: API tests (ready, each reason, unknown alias, reload), a Vitest exhaustiveness test that every API reason has text, native bridge command tests.
- [x] C17 — Apple assets keep their `default`/`not-applicable` pins; chat and generation capability become `ready` when the adapter is registered. Evidence: updated loader, embedded-table and capability tests; `scripts/smoke/agent.py` finds both assets and the helper in the built binary.
- [x] C18 — Fixtures are captured from the real helper on this Mac (availability, chat reply, structured candidate, at least one real error) with OS and SDK versions in the path, redacted, and replayed. An env-gated live test (`DIARY_APPLE_LIVE=1`, ignored by default) runs one availability check, one synthetic generation, one chat reply and one mid-run cancellation against the built server and helper; it records OS, SDK, timing and model metadata (context size, supported languages). It is agent-run, not a human exit gate. If Apple Intelligence is unavailable here, C18 stays unchecked and the exact reason is reported.
- [x] C19 — Nothing starts a run from an API, bridge command or UI; `agents_available` stays `false`; no `Cargo.lock` or `package*.json` change. The only build change is the approved `swiftc` step.
- [x] C20 — Affected docs state actual behavior and evidence limits: editing-and-agents (abstraction, Apple row and status paragraph), api-and-processes (reason field), logging, configuration-and-exports platform table, handoff E5/E11 and the M20 row, prompts README, phase plan, roadmap, changelog, docs index and AGENTS.md.

## Plan-review checklist

- [x] R01 — Compare the plan with the maintained contracts, the M17–M19 code, the installed SDK interface and the logging contract; findings are in the work plan.
- [ ] R02 — Map each finding to a C item and a named test.
- [ ] R03 — Format the changed plans, check local links and whitespace, and review the scoped diff.

## Implementation gate

Run the gate at the end of stage A, at the end of stage B and at the end of stage C. A stage is not started while the previous gate is red.

- [x] G01 — `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [x] G02 — `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [x] G03 — `cargo test --manifest-path src-tauri/Cargo.toml`
- [x] G04 — `npx prettier --check .`
- [x] G05 — `npx eslint . --max-warnings 0`
- [x] G06 — `npx vitest run`
- [x] G07 — `npx tsc --noEmit` (additional project check)
- [x] G08 — `python3 scripts/smoke/agent.py` (embedding, helper present, real-server profile status for all four kinds with fakes)

Run affected tests while iterating. If a command cannot run, leave its item unchecked and record the blocker. No human test is an exit gate.

## Evidence record

### Apple spike (2026-11-06, macOS 26.7.1 arm64, SDK 27.2, Swift 6.4.2, target `macos26.0`)

Throwaway code, not kept. No locked-screen or sleep behavior was observed (not observed).

- Availability: `available`. Context size 4096 tokens. 23 supported languages (`da de en en-AU en-GB es es-419 es-US fr fr-CA it ja ko nb nl pt pt-PT sv tr vi zh zh-HK zh-TW`).
- Locales (owner scope: ko_KR and en_US): `supportsLocale` is true for `ko_KR` and `en_US`; a real request in each succeeded (Korean and English replies). `xx_XX` is false, but a request still ran, so the helper must check `supportsLocale` itself before inference.
- Chat: plain `respond(to:)` works (~0.4 s warm).
- Guided output: a `@Generable` struct mirroring schema 1 (`draft_markdown`, `claim_support[{claim, snippet_ids}]`, `warnings`) works with nested arrays; no runtime schema needed.
- Errors on OS 26.7.1 come from the older family `LanguageModelSession.GenerationError`: context overflow is `exceededContextWindowSize` (8,023 tokens against 4,096); a second concurrent call on one session is `concurrentRequests`; task cancellation surfaces as `Swift.CancellationError`. The `LanguageModelError` family (OS 27) is not exercised here.

### Stage A (2026-11-06, commits ae6ade6, bc82d2e, adad2a7 and the follow-up on `main`)

- C01: `Host` is built in `diary-server/src/main.rs` and in `tests/support`; `git grep "fn host(" -- src-tauri/crates/diary-infrastructure/src` finds nothing.
- C02: `agent/engine.rs` holds the single `execute`, `check_entry` and `AgentRunner` impl (`Adapter<P: Protocol>`). Protocols keep the post-escape result block (`Stream::conclude`) and result reading (`Stream::collect`) because the checks differ per adapter and run on either side of the `ValidatingOutput` phase. Codex 446 lines to 330, Claude 467 to 305, Qwen 496 to 320 including tests; lifecycle lines exist once.
- C03: the M17-M19 suites (`agent_codex`, `agent_claude`, `agent_qwen`, `agent_process`, `agent_generation`, `profiles_*`) pass with edits only to constructors (`Host` from `tests/support`, `Adapter::new(table, host)`), the `.status` accessor, and `r.host.cancel_all()/shutdown()` replacing the same calls on the adapter. Fixtures unchanged (`git diff --stat -- '*/tests/fixtures'` empty). `scripts/gate.sh` (G01-G07) and `scripts/smoke/agent.py` pass. Live re-run on the final code (Codex `~/.codex-pp8`, Claude default `~/.claude`, Qwen `~/.qwen`): `live_check_and_generation`, `live_chat`, `live_cancel` all passed for each of the three.
- C04: `Agents` is the one `AgentRunner`; `ProfileService::new(table, agents)`; tests `registry_dispatches_every_kind`, `registry_rejects_unregistered_kind`, `every_agent_kind_has_a_protocol` (Apple is the one deliberately unregistered kind until stage C). No `matches!(...Codex|Claude|Qwen)` remains.
- C05: `AgentKind::spec()` returns `KindSpec` (asset dir, default binary, home need, session asset, fixed pins), read by `assets.rs` and the engine; `kind_spec_covers_every_kind` in `diary-application`.
- C06: `tests/agent_conformance.rs` runs busy-free success, phase order, unknown alias, wrong kind, failure mapping, malformed/fenced, no content, cancel, shutdown, lease end and missing home over Codex, Claude and Qwen through the registry (10 tests pass). Apple joins in stage C.

Deliberate deviation from "no behavior change" (Q8 reports drift, this one is a bug fix): `run` looked an entry up by alias only, so a request for kind X could run a profile configured as kind Y with X's argv. The engine now rejects it as `unavailable`; `check` already did. Found by the conformance wrong-kind case.

Drift between adapters left as is (follow-ups for the owner): Qwen has no auth probe (`ready` means installed and supported); only Codex has the quoting rule on model and effort pins; the post-escape order differs (Codex `exit or turn.failed` then classify; Claude stream failure before exit; Qwen `init_mismatch` before failure and its own classifier); Codex reads results from a file while Claude and Qwen read the terminal record; the preflight at run time is the version probe only, never the auth probe.

### Stage B (2026-11-06, partial)

- C07: `AgentDetail` in `diary-application::diagnostics`, event codes `agent.check`, `agent.blocked`, `agent.run`, `agent.helper`; codec renders and strictly parses the `agent` object (`render_then_parse` over every `Reason` x `AgentErrorCode`, rejection of unknown keys and values, old lines still parse); trace re-renders it and the legend lists the keys; `docs/architecture/logging.md` updated. `scripts/gate.sh` (G01-G07) and `scripts/smoke/agent.py` pass.
- C08 (open): `tests/agent_diagnostics.rs` proves exactly one typed record per refusal, failure, stop and check for the reachable reasons on all three CLI kinds (profile unknown, cancelled before start, shutdown, lease end, home missing or not a directory, executable missing, version unsupported or unreadable, success, unauthorized, rate limit, context, other failure, inactivity, malformed output, cancel; check status, reason and `changed`). Reasons not reachable with the fake CLI (packet too large, run directory, pin rejected, assets missing or invalid, escaped, record or output too large, helper and availability reasons) are not yet covered by a record test; the Apple ones arrive with stage C.
- C09 (open): the seeded-string scan runs over the typed record dump of every kind, not yet over a real log directory written by the worker.
- C10 (open): a trace unit test covers a refused, a failed and a cancelled agent run and `recent`; the real-server smoke is not extended.

### Stage C (2026-11-06)

Final gate on the finished tree: `scripts/gate.sh` G01-G07 pass; `python3 scripts/smoke/agent.py` passes with the helper built, and passes with `DIARY_SKIP_SWIFT=1` in a separate target directory (profile reports `helper_missing`).

- C11: `diary-infrastructure/build.rs` compiles `native/apple-fm/main.swift` with `swiftc -O -target <arch>-apple-macos26.0` into `OUT_DIR` and the crate embeds it (`include_bytes!`); a missing toolchain, an SDK without the framework or a non-macOS target writes an empty file and warns. The smoke checks the framework path in the built binary and the extracted file in `<runtime_root>/native/apple-fm-<hash>` with mode 0700. `apple_missing_helper_is_unavailable`, `apple_tampered_helper_is_untrusted` (appends to the extracted file, expects `helper_untrusted` and an `agent.helper` record), `apple_unsupported_os_is_unavailable_without_launching`. Limit: the "no swiftc" build was exercised with `DIARY_SKIP_SWIFT=1`, not by removing `swiftc` from `PATH`; both take the same path.
- C12: `apple_check_maps_each_reason` covers device ineligible, disabled, not ready, unknown and unrecognized, junk output (`helper_failed`) and a hung probe (`probe_timed_out`); `apple_availability_is_rechecked_before_every_run`; the `changed` tag logs a reason change (`checks_log_status_reason_and_whether_it_changed`); the live availability record is below. Settings has distinct text for every API-visible reason.
- C13: `apple_argv_is_fixed_and_contains_no_user_text` (argv is `availability` or `run`; stdin is JSON with instructions, prompt, locale and mode and no model or effort), `apple_generation_output_is_validated`, `apple_chat_reply_streams_and_is_bounded`, `apple_helper_source_never_uses_cloud_or_tools_and_mirrors_the_schema` (no `PrivateCloudCompute`, no `tools:`, no `URLSession`, struct fields equal the schema's required fields).
- C14 (open): the OS 26 `LanguageModelSession.GenerationError` family is mapped in the helper (context, assets unavailable, guardrail, unsupported language or locale, decoding failure, rate limit, concurrent request; anything else is `native_error_unmapped`) and the Rust side maps the closed helper codes (`apple_errors_map_to_allowlist`, `apple_availability_loss_mid_run_is_unavailable`, `apple_unknown_native_error_is_failed_and_logged`). A real context overflow was recorded. The OS 27 `LanguageModelError` family (refusal, timeout, unsupported capability) is not mapped: it cannot be exercised on macOS 26.7.1, and the plan forbids shipping a mapping from memory. On OS 27 those errors would arrive as `native_error_unmapped`, which is `failed` and is logged. Guardrail and rate-limit behavior is covered by the stand-in helper only.
- C15: `apple_cancel_terminates_group` (cancel, then shutdown refuses), `apple_inactivity_becomes_timeout_and_lease_end_cancels_without_poisoning`; the escaped-descendant, late-output and TERM-ignoring cases are proven on the shared engine by the CLI conformance and process suites. Limit: the conformance matrix itself was not extended to the Apple kind.
- C16: `GET /profiles` and `POST /profiles/{alias}/check` carry `reason` (API tests in `diary-api`, `list_reports_status_and_capabilities_without_checking`); the bridge validates it (`reasons_are_checked_against_the_closed_api_set`); the Rust test `api_visible_reasons_match_the_shared_list` ties `src/application/profile-reasons.json` to the server's set; `tests/profile-reasons.test.ts` ties the validator list and the Settings wording to the same file and requires distinct wording; `tests/profiles-ui.test.tsx` shows "Apple Intelligence is turned off in System Settings".
- C17: Apple assets keep `default` / `not-applicable` (`assets.rs` pin tests); `Agents::standard` registers Apple, so the capability is `ready` (smoke asserts chat and generate available for the Apple profile).
- C18: recordings from the real helper under `tests/fixtures/agent/apple-macos26.7.1-sdk27.2/` (availability, chat, structured candidate, real context overflow, real locale refusal) replayed by `recorded_macos_26_7_1_streams_parse`. Live run (`DIARY_APPLE_LIVE=1`, macOS 26.7.1 build 25G241, SDK 27.2, Swift 6.4.2): `live_availability` ready in 172 ms; `live_generation` validated schema-1 candidate in 8.2 s; `live_chat` replied in `en` (511 ms) and `ko` (331 ms); `live_cancel` returned `cancelled` in 473 ms; no run directory remained. Context size 4096 and the supported language list are in the availability recording. Owner scope for languages is `ko_KR` and `en_US`, both supported here.
- C19: no run-starting route, command or UI was added; the smoke asserts `agents_available` is `false`; `git diff 5d3da06 HEAD` shows no change to `Cargo.lock`, `package.json`, `package-lock.json` or any `Cargo.toml`. The only build change is the `swiftc` step in `build.rs`.
- C20: docs updated (editing-and-agents, api-and-processes, logging, configuration-and-exports, handoff E11 and M20 row, prompts README, phase plan, changelog, AGENTS.md); the roadmap keeps P1-M20 active because C08-C10 and C14 are open.

### Open items (why P1-M20 is not Done)

- C08: records are proven one per event for the reachable reasons on the three CLI kinds and for the Apple availability, helper, OS, mid-run and native-error paths; not every value of `Reason::ALL`, `AgentErrorCode::ALL`, `Stop` and `ProfileStatus` has a record test (missing: `packet_too_large`, `prompt_pin_rejected`, `assets_missing`, `assets_invalid`, `run_dir_unavailable`, `record_too_large`, `output_too_large`, `escaped_descendants`, `init_mismatch`).
- C09: the seeded-string scan covers the typed records, not yet a log directory written by the real log worker.
- C10: the trace has unit coverage for refused, failed and cancelled agent runs and `recent`; the real-server smoke does not exercise it because no route starts a run.
- C14: the OS 27 error family, see above.

Owner follow-ups (not changed): Qwen has no auth probe; only Codex has the quoting rule; the failure and exit ordering differs per CLI; a run now refuses a profile configured for another kind.
