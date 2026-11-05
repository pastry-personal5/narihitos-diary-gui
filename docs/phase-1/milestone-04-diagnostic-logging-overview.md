# P1-M4: Diagnostic logging overview

Status: Active

Implement the [logging contract](../architecture/logging.md): private, bounded, asynchronous server and client logs with daily rotation, 72-hour retention and content redaction. Add a debugging trace that lets a person or an AI coding agent follow one request across server and client logs to redacted resource details (kind, opaque ID, revision, size, keyed fingerprint, state), without any diary content, credential or path. The work plan is [milestone-04-diagnostic-logging-architecture.md](milestone-04-diagnostic-logging-architecture.md).

## Entry conditions

P1-M3 is Done. Read the logging contract, its [required evidence](../architecture/logging.md#required-evidence), the [`log_client_event` row](../architecture/api-and-processes.md#operations-and-tauri-commands) and handoff row E12. No owner product question blocks this milestone.

The owner asked on 2026-11-05 for an agent-debugging trace to redacted resource detail. That addition (trace IDs, resource descriptors, read-only trace command) extends the logging and API contracts; task 1 of the work plan amends them and records the request in the [changelog](changelog.md) before code.

**No new external crate or build tool.** The plan uses `std` threads and channels, the already-approved `libc`, `serde_json`, `sha2` and `uuid` crates, and the existing `PrivateDir` helpers. The contract names `tracing` as a preferred candidate; it needs a pruner of our own anyway, so this plan does not adopt it. Adopting it later requires owner approval and a license check.

**Crate layout (owner decision, 2026-11-05).** Logging lives in a new `diary-logging` crate that also takes `PrivateDir` from `diary-infrastructure`. `diary-infrastructure`, `diary-server` and the `diary-client` Tauri package depend on it, so the client does not link the YAML adapter. No new external crate or license; the architecture test lists the new edges.

## Scope

| Included in P1-M4                                                                                                                                      | Deferred to                                                                                      |
| ------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------ |
| Typed diagnostic model: allowlisted event codes, typed fields, trace IDs, redacted resource descriptors                                                | Event codes for later features are added by their milestones (save P1-M6/M7, jobs P1-M21+)       |
| Role-owned log store: 0700/0600, one writer lock, symlink rejection, daily and size rotation, caps, 72-hour pruning                                    | Real save/job isolation evidence against actual producers: P1-M6/M7 and P1-M21+ (synthetic here) |
| Bounded asynchronous worker, drop policy, rate-limited stderr notice, bounded shutdown drain                                                           | Settings log-level control: P1-M8 (level is fixed at `info` here)                                |
| Server wiring for existing lifecycle/session/auth/job events                                                                                           | Packaged iCloud/TCC behavior: P1-M13/P1-M28                                                      |
| Client `log_client_event` command, 256-line pre-discovery buffer, 20 events/s limit, permission and capability update                                  | Exporting, syncing or displaying logs in the GUI (not planned)                                   |
| `X-Diary-Trace-Id` correlation header and echo                                                                                                         | Redacted resource descriptors for sources, exports and agents: attached by their milestones      |
| Read-only `diary-server trace` command (selectors trace/op/job/resource/recent) that prints a bounded, revalidated, redacted timeline for one selector | Any model-facing UI or automatic upload of traces (not planned)                                  |
| Packaged synthetic smoke for log modes, secret scan and trace output                                                                                   | Release qualification: P1-M28                                                                    |

## Completion checklist

Check an item only with a linked result identifying the tested revision, command or manual procedure, observed outcome and limitations. [Acceptance matrix](milestone-04-diagnostic-logging-architecture.md#acceptance-matrix) rows use the same L IDs; G IDs are the unchanged repository gate.

- [x] **L01 — Contracts:** Amend [logging](../architecture/logging.md) and [API](../architecture/api-and-processes.md) for trace IDs, resource descriptors, the trace command and diagnostic-only read-back; record the owner request in the changelog.
- [x] **L02 — Diagnostic model:** `Record` can be built only from allowlisted codes and typed fields; a free-form string cannot be attached; a rendered line over 2 KiB is dropped, not truncated.
- [x] **L03 — Store:** Role directories are 0700 and files 0600; symlinks and nonregular files are rejected on open, rotation and pruning using opened objects; a second writer fails to lock.
- [x] **L04 — Rotation:** Daily rollover at local midnight including DST transitions, 16 MiB size rotation, suffix allocation without truncation after restart, and a backward clock change.
- [x] **L05 — Caps and pruning:** At most eight files/128 MiB per role directory; at the cap, records drop instead of deleting younger files; files are pruned at exactly 72 hours (kept just under, deleted just over) at start, at rotation and hourly while idle; the active file and unrelated files or symlinks are never touched.
- [x] **L06 — Asynchronous worker:** Queue of 256 records of 2 KiB; overflow drops; stalled disk or stderr never blocks a producer; at most one stderr notice per minute; bounded shutdown drain. A synthetic job admission and status read complete unchanged while diagnostics are stalled; real saves are re-proved in P1-M6/M7.
- [x] **L07 — Server integration:** The server logs start, bind, discovery, shutdown, session, auth-rejection and fake-job events at `info` or above; no record contains the token, config contents or a request body.
- [x] **L08 — Client logging:** `log_client_event` rejects unknown codes, extra keys, details over 512 bytes and more than 20 events per second; the 256-line buffer and stderr fallback work before discovery; the WebView never receives a log path. Command manifest, invoke handler and capability inventories agree.
- [x] **L09 — Trace correlation:** The bridge sends a valid `X-Diary-Trace-Id`; the server accepts only 32 lowercase hex characters, otherwise generates one, and echoes it; the same ID appears in client and server records of one request.
- [x] **L10 — Redacted resource detail:** Descriptors expose only kind, validated opaque ID, revision, byte and line counts, per-instance keyed fingerprint, state tag and path class; seeded diary text, token and path strings never appear in any log line, buffer or stderr output.
- [x] **L11 — Trace reader:** `diary-server trace` selects by trace, operation, job, resource or `recent:<n>` (last n warn/error records, the starting point when no trace ID is known), merges server and client logs by time, revalidates every line against the schema, tolerates a truncated last line, bounds output to 200 records (maximum 500) and 256 KiB, never reads the discovery file, and neutralizes injected instruction text planted in a log line.
- [ ] **L12 — Native smoke:** Packaged server and client leave 0700/0600 logs, a seeded-secret scan finds nothing, and the trace command reconstructs one smoke request across both roles.
- [x] **L13 — Handoff:** Update contribution guide (agent debugging procedure), AGENTS.md commands, docs index, phase plan, roadmap, handoff row E12 and changelog; archive nothing until the milestone is Done.
- [x] **G01:** Pass `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
- [x] **G02:** Pass `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [x] **G03:** Pass `cargo test --manifest-path src-tauri/Cargo.toml`.
- [x] **G04:** Pass `npx prettier --check .`.
- [x] **G05:** Pass `npx eslint . --max-warnings 0`.
- [x] **G06:** Pass `npx vitest run`.

## Evidence record

Revision `158bb89` plus the review fix pass described at the end of this record, on branch `p1-m4-diagnostic-logging`, Apple Silicon macOS, 2026-11-05. Ordinary (non-fixture) build unless stated. Runs went through the test-runner; counts below are verbatim per-binary results.

L01 | `docs/architecture/logging.md`, `api-and-processes.md`, changelog | prettier and relative-link check clean | amendments for trace IDs, resource detail, `comp`, selectors, trace command | Limit: owner approval is recorded in the changelog entry of the same day.

L02 | `diary-application` diagnostics tests (4) and `diary-logging` codec tests (7): ID/tag/code parsers, round trip, key order, strict parse, 2 KiB drop | pass | no free-form string can be attached to a `Record` by construction | Limit: the over-2 KiB path is tested on the helper `fit` because a typed record cannot exceed the cap.

L03–L05 | `diary-logging` private_fs (5) and store (10) tests: 0700/0600, second open `Locked`, hostile symlink/fifo, midnight and DST rollover, size suffixes after restart, backward clock, file and total caps, exact 72 h boundary, prune on open and rotation, active/unrelated/other-role/symlink names untouched | pass | Limit: modification times are set explicitly in tests; there is no test of real wall-clock DST on this machine.

L06 | worker tests (7) and `diary-server/tests/stalled_diagnostics.rs`: 10,000 emits in under 1 s with a blocked writer, 300 emits drop 43 or 44, one stderr notice per minute, hourly idle prune, bounded shutdown, and HTTP status/session/rejection requests under a blocked writer | pass | Limit: proves job-admission-era HTTP only; real save isolation is re-proved in P1-M6/M7.

L07 | diary-api diagnostics tests (5) and the server smoke: lifecycle, session, busy, cancel and auth-rejection events with no token or body | pass | Limit: `job.admitted` is not emitted (admission exists only in the fixture build); fixture job IDs are not opaque IDs, so the `job` field is omitted for them.

L08 | `src-tauri/src/diagnostics/tests.rs` (4) and Vitest adapter/architecture tests: allowlist, extra keys, 512-byte detail, 20 events/s, 256-line buffer keeping the earliest, ordered flush, locked-log fallback to stderr, command inventory | pass | Limit: "no log path in any response" is structural (the command returns `Result<(), &'static str>`). Plumbing only: no UI code calls `logClientEvent` yet, so no real WebView event is reported.

L09 | diary-api trace tests and `bridge/tests.rs::every_request_carries_a_fresh_lowercase_hex_trace_id` | pass.

L10 | codec fingerprint test, diary-api seeded body/token test, server smoke grep of every token and a sentinel across the log | pass | Limit: stderr output of the smoke server is not scanned. `Fingerprinter` has no production caller yet; later milestones attach resource detail when they add saves, sources and exports, so L10 is proved on synthetic data only.

L11 | trace tests (9) and `python3 scripts/smoke/server.py` against the real binary: all selectors, epoch ordering across a DST offset change, truncated and oversize lines, injected instruction text, output bounds, no discovery read | pass | Limit: the CLI argument parser has no unit test; it is exercised only by the smoke script.

L12 | Server half run: building `diary-server` and running `python3 scripts/smoke/server.py` printed PASS (0700/0600 logs, secret scan, `trace:` and `recent:` output). **Not complete:** the packaged-client half (`make smoke`, which needs a GUI session) has not been run, so client log modes and the competitor trace correlation are unverified. The new script lines for it are untested.

L13 | contribution guide (debugging procedure), AGENTS.md command, docs index, phase plan, changelog | prettier clean | Limit: the handoff E12 row keeps its text; P1-M12/P1-M13 own shortcut and native key evidence.

G01–G06 | revision `06eae45` (docs-only changes since) | fmt, clippy, prettier, eslint exit 0; the Rust suite passed 123 tests (api 18, application 12, client 1+23, domain 1, infrastructure 24, logging 43, stalled_diagnostics 1); Vitest 29 passed | session output, not retained in the repo.

Review fix pass | whole-branch review of `d25affd..158bb89` found no Critical issues and four Important ones, all fixed with tests: per-code 60/minute producer cap (`a_flooding_code_cannot_silence_other_events`), newline before appending to a truncated tail (`restart_onto_a_truncated_tail_starts_a_fresh_line`), 60-second capped retry gate (`capped_state_does_not_rescan_the_directory_for_every_record`), and a bounded trace collection (`recent_counts_every_match_but_returns_only_the_last_n`, a regression guard: the memory bound itself is not measured) | afterwards the Rust suite passed 127 tests twice with identical results (api 18, application 12, client 1+23, domain 1, infrastructure 24, logging 47, stalled_diagnostics 1), clippy passed with and without `scaffold-smoke`, prettier, eslint and Vitest (29) passed, and the server smoke printed PASS | Ten Minor findings are deferred, not fixed (descriptor `dup` without close-on-exec, planted canonical-name entries occupying cap slots, `ClientLog::attach` holding its mutex while opening, shutdown not waking an idle worker, `ui.bridge_error` for expected 409 responses, duplicate `job.finished` on repeated cancel, unused event codes, `trace` panicking on a closed pipe, loose RFC 3339 parsing, early-exit paths skipping shutdown).
