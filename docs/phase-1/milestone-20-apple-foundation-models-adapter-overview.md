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

- [ ] C01 — `Host` is built once, outside any adapter (server wiring and a test kit); no adapter exposes `host()`. Evidence: `git grep "fn host("` finds nothing in `agent/`; `diary-server/src/main.rs` constructs `Host` and the adapters from it.
- [ ] C02 — The lifecycle (cancel/shutdown gate, kind check, profile lookup, asset and packet rendering, executable and home resolution, run directory, phase events, preflight, supervised spawn, stop/escape/exit mapping, output validation, `Run` finish, active counter, run log) exists once in a generic engine. Evidence: one `execute` template in `agent/`; `git diff --stat` shows the three adapters shrinking to protocol code; no duplicated lifecycle lines.
- [ ] C03 — Codex, Claude and Qwen are `Protocol` implementations and behave exactly as before. Evidence: each stage-A commit passes the M17/M18/M19 suites (`agent_codex`, `agent_claude`, `agent_qwen`, `agent_process`, `agent_generation`, `profiles_*`) with only constructor and `.status` accessor edits, listed in the evidence record; fixtures unchanged; `scripts/smoke/agent.py` passes; the three env-gated live tests are re-run once on the final code.
- [ ] C04 — One `Agents` registry is the single `AgentRunner` the rest of the code holds; it dispatches on `profile.kind`, answers `registered(kind)` and rejects unknown kinds. `ProfileService` takes the registry, not one field per adapter. Evidence: `registry_dispatches_every_kind`, `registry_rejects_unregistered_kind`, `every_agent_kind_has_a_protocol` (iterates `AgentKind::ALL`), no `matches!(…Codex|Claude|Qwen)` left.
- [ ] C05 — Per-kind facts (asset directory, default binary name, home requirement, required assets, allowed pins) live in one table consumed by assets, config mapping and protocols; `assets.rs` has no `if kind == AppleFoundationModels` branches. Evidence: `kind_spec_covers_every_kind`, updated asset tests, grep.
- [ ] C06 — A conformance suite runs the same lifecycle matrix over all four kinds against fake children: busy, shutdown, lease end, cancel, timeout, flood, hang, TERM-ignoring, late output, escaped descendants, unknown alias, wrong kind, missing assets, bad pins, no content in errors, and the exact phase order. Evidence: `agent_conformance.rs` names each case and kind; a new agent needs only a fake and one registry entry.

### Stage B — diagnostics

- [ ] C07 — Agent diagnostics are typed and closed: event codes `agent.check`, `agent.run`, `agent.blocked`, `agent.helper`; tags for agent kind, action, last phase reached, error code (all 11 `AgentErrorCode` values), stop cause, exit class and a single closed `Reason` set; counts for escaped processes, events, input and output bytes, elapsed milliseconds. No field takes text. The codec, parser, trace renderer and `docs/architecture/logging.md` cover them; old lines still parse. Evidence: codec/trace unit tests, schema rejection tests, the updated contract.
- [ ] C08 — Every refusal, failure, stop and blocked state emits exactly one record with the expected tags. Evidence: a table-driven test over `Reason::ALL`, `AgentErrorCode::ALL`, `Stop` and every `ProfileStatus` for all four kinds (host busy, shutdown, lease end, cancelled before start, profile removed by reload, assets missing or invalid, pin rejected, packet too large, run directory unavailable, executable or home missing, version unsupported or unreadable, probe timeout, spawn failure, record or output too large, escaped descendants, nonzero exit, missing result, helper missing or untrusted, availability reasons).
- [ ] C09 — No record contains content. Evidence: a seeded-string scan over the log directory after failing runs of every kind (diary body, prompt, reply, native error text, helper stderr, executable names, paths, environment values) finds none; helper and CLI stderr are classified, never logged.
- [ ] C10 — `diary-server trace` shows a readable timeline for a refused check, a failed run and a cancelled run, and `recent:<n>` lists agent warnings. Evidence: trace fixtures and the real-server smoke.

### Stage C — Apple

- [ ] C11 — The helper is built by `diary-infrastructure/build.rs` with `swiftc` and embedded in the server binary. When `swiftc` or the SDK is missing the build succeeds and the Apple profile reports `unavailable` with reason `helper_missing`. The helper is extracted to the runtime root with mode 0700 and hash-checked before every launch. Evidence: a build with the helper; a build with `swiftc` hidden from `PATH`; `apple_missing_helper_is_unavailable`, `apple_tampered_helper_is_untrusted`.
- [ ] C12 — `check` runs the helper's `availability` command (no session, no inference) and reports `ready` or `unavailable` with one closed reason: `os_unsupported`, `device_ineligible`, `apple_intelligence_disabled`, `model_not_ready`, `model_unavailable_unknown`, `helper_missing`, `helper_untrusted`, `helper_failed`, `probe_timed_out`. Each has user-facing text. Availability is rechecked before every run, and a change of reason between records is logged. Evidence: `apple_check_maps_each_reason` against the fake helper, the live availability record.
- [ ] C13 — `run` uses only `SystemLanguageModel.default`: a fresh session per chat turn or candidate, instructions and prompt on stdin, no tools, no model or effort override, no file access, and no use of `PrivateCloudComputeLanguageModel`. Generation uses guided output matching schema 1 and passes the M16 validator; chat returns a bounded `Reply`. Evidence: `apple_argv_is_fixed_and_contains_no_user_text`, `apple_generation_output_is_validated`, `apple_chat_reply_is_bounded`, a source gate that fails if the helper names `PrivateCloudCompute`, a drift test between the Swift output type and `GENERATION_SCHEMA`.
- [ ] C14 — Failures map to the closed error allowlist from both SDK error families (`LanguageModelSession.GenerationError` on OS 26, `LanguageModelError` on OS 27) with no native message text: context → `context`; unsupported language or locale → `unsupported_language`; guardrail violation or refusal → `refusal`; rate limiting → `rate_limited`; decoding failure or schema mismatch → `invalid_output`; assets unavailable or availability lost mid-run → `unavailable`; concurrent requests → `busy`; native timeout → `timeout`; anything unrecognized → `failed` plus a logged `native_error_unmapped` reason. Evidence: `apple_errors_map_to_allowlist`, `apple_availability_loss_mid_run_is_unavailable`, `apple_unknown_native_error_is_failed_and_logged`.
- [ ] C15 — Cancel, timeout, lease end and shutdown terminate the helper (TERM, grace, KILL, reap, escaped-descendant detection) and drop late output; covered by the conformance suite for the Apple kind plus `apple_cancel_terminates_group`.
- [ ] C16 — `GET /profiles` and `POST /profiles/{alias}/check` carry the closed `reason` for every kind (null when ready); the bridge commands and TypeScript types carry it; Settings shows distinct text for every reason, such as "Apple Intelligence is turned off in System Settings". Evidence: API tests (ready, each reason, unknown alias, reload), a Vitest exhaustiveness test that every API reason has text, native bridge command tests.
- [ ] C17 — Apple assets keep their `default`/`not-applicable` pins; chat and generation capability become `ready` when the adapter is registered. Evidence: updated loader, embedded-table and capability tests; `scripts/smoke/agent.py` finds both assets and the helper in the built binary.
- [ ] C18 — Fixtures are captured from the real helper on this Mac (availability, chat reply, structured candidate, at least one real error) with OS and SDK versions in the path, redacted, and replayed. An env-gated live test (`DIARY_APPLE_LIVE=1`, ignored by default) runs one availability check, one synthetic generation, one chat reply and one mid-run cancellation against the built server and helper; it records OS, SDK, timing and model metadata (context size, supported languages). It is agent-run, not a human exit gate. If Apple Intelligence is unavailable here, C18 stays unchecked and the exact reason is reported.
- [ ] C19 — Nothing starts a run from an API, bridge command or UI; `agents_available` stays `false`; no `Cargo.lock` or `package*.json` change. The only build change is the approved `swiftc` step.
- [ ] C20 — Affected docs state actual behavior and evidence limits: editing-and-agents (abstraction, Apple row and status paragraph), api-and-processes (reason field), logging, configuration-and-exports platform table, handoff E5/E11 and the M20 row, prompts README, phase plan, roadmap, changelog, docs index and AGENTS.md.

## Plan-review checklist

- [x] R01 — Compare the plan with the maintained contracts, the M17–M19 code, the installed SDK interface and the logging contract; findings are in the work plan.
- [ ] R02 — Map each finding to a C item and a named test.
- [ ] R03 — Format the changed plans, check local links and whitespace, and review the scoped diff.

## Implementation gate

Run the gate at the end of stage A, at the end of stage B and at the end of stage C. A stage is not started while the previous gate is red.

- [ ] G01 — `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [ ] G02 — `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] G03 — `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] G04 — `npx prettier --check .`
- [ ] G05 — `npx eslint . --max-warnings 0`
- [ ] G06 — `npx vitest run`
- [ ] G07 — `npx tsc --noEmit` (additional project check)
- [ ] G08 — `python3 scripts/smoke/agent.py` (embedding, helper present, real-server profile status for all four kinds with fakes)

Run affected tests while iterating. If a command cannot run, leave its item unchecked and record the blocker. No human test is an exit gate.

## Evidence record

### Apple spike (2026-11-06, macOS 26.7.1 arm64, SDK 27.2, Swift 6.4.2, target `macos26.0`)

Throwaway code, not kept. No locked-screen or sleep behavior was observed (not observed).

- Availability: `available`. Context size 4096 tokens. 23 supported languages (`da de en en-AU en-GB es es-419 es-US fr fr-CA it ja ko nb nl pt pt-PT sv tr vi zh zh-HK zh-TW`).
- Locales (owner scope: ko_KR and en_US): `supportsLocale` is true for `ko_KR` and `en_US`; a real request in each succeeded (Korean and English replies). `xx_XX` is false, but a request still ran, so the helper must check `supportsLocale` itself before inference.
- Chat: plain `respond(to:)` works (~0.4 s warm).
- Guided output: a `@Generable` struct mirroring schema 1 (`draft_markdown`, `claim_support[{claim, snippet_ids}]`, `warnings`) works with nested arrays; no runtime schema needed.
- Errors on OS 26.7.1 come from the older family `LanguageModelSession.GenerationError`: context overflow is `exceededContextWindowSize` (8,023 tokens against 4,096); a second concurrent call on one session is `concurrentRequests`; task cancellation surfaces as `Swift.CancellationError`. The `LanguageModelError` family (OS 27) is not exercised here.

Not started otherwise.
