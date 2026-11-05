# P1-M4: Diagnostic logging overview

Status: Planned

Implement the [logging contract](../architecture/logging.md): private, bounded, asynchronous server and client logs with daily rotation, 72-hour retention and content redaction. Add a debugging trace that lets a person or an AI coding agent follow one request across server and client logs to redacted resource details (kind, opaque ID, revision, size, keyed fingerprint, state), without any diary content, credential or path. The work plan is [milestone-04-diagnostic-logging-architecture.md](milestone-04-diagnostic-logging-architecture.md).

## Entry conditions

P1-M3 is Done. Read the logging contract, its [required evidence](../architecture/logging.md#required-evidence), the [`log_client_event` row](../architecture/api-and-processes.md#operations-and-tauri-commands) and handoff row E12. No owner product question blocks this milestone.

The owner asked on 2026-11-05 for an agent-debugging trace to redacted resource detail. That addition (trace IDs, resource descriptors, read-only trace command) extends the logging and API contracts; task 1 of the work plan amends them and records the request in the [changelog](changelog.md) before code.

**No new external crate or build tool.** The plan uses `std` threads and channels, the already-approved `libc`, `serde_json`, `sha2` and `uuid` crates, and the existing `PrivateDir` helpers. The contract names `tracing` as a preferred candidate; it needs a pruner of our own anyway, so this plan does not adopt it. Adopting it later requires owner approval and a license check.

**Owner confirmation needed before Task 9: internal crate edges.** The `diary-client` package currently depends on no `diary-*` crate. Reusing the log worker, `PrivateDir` and record codec in the client adds `diary-application` and `diary-infrastructure` as path dependencies (workspace members, no new external crate or license). Side effect: the client binary also links `serde_yaml_ng` and `unsafe-libyaml`, which the client does not call. The alternative is a separate logging crate, which needs `PrivateDir` moved out of infrastructure and is a broader restructure. Recommended: the two path dependencies. Tasks 1–8 and 10 do not depend on the answer.

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

- [ ] **L01 — Contracts:** Amend [logging](../architecture/logging.md) and [API](../architecture/api-and-processes.md) for trace IDs, resource descriptors, the trace command and diagnostic-only read-back; record the owner request in the changelog.
- [ ] **L02 — Diagnostic model:** `Record` can be built only from allowlisted codes and typed fields; a free-form string cannot be attached; a rendered line over 2 KiB is dropped, not truncated.
- [ ] **L03 — Store:** Role directories are 0700 and files 0600; symlinks and nonregular files are rejected on open, rotation and pruning using opened objects; a second writer fails to lock.
- [ ] **L04 — Rotation:** Daily rollover at local midnight including DST transitions, 16 MiB size rotation, suffix allocation without truncation after restart, and a backward clock change.
- [ ] **L05 — Caps and pruning:** At most eight files/128 MiB per role directory; at the cap, records drop instead of deleting younger files; files are pruned at exactly 72 hours (kept just under, deleted just over) at start, at rotation and hourly while idle; the active file and unrelated files or symlinks are never touched.
- [ ] **L06 — Asynchronous worker:** Queue of 256 records of 2 KiB; overflow drops; stalled disk or stderr never blocks a producer; at most one stderr notice per minute; bounded shutdown drain. A synthetic job admission and status read complete unchanged while diagnostics are stalled; real saves are re-proved in P1-M6/M7.
- [ ] **L07 — Server integration:** The server logs start, bind, discovery, shutdown, session, auth-rejection and fake-job events at `info` or above; no record contains the token, config contents or a request body.
- [ ] **L08 — Client logging:** `log_client_event` rejects unknown codes, extra keys, details over 512 bytes and more than 20 events per second; the 256-line buffer and stderr fallback work before discovery; the WebView never receives a log path. Command manifest, invoke handler and capability inventories agree.
- [ ] **L09 — Trace correlation:** The bridge sends a valid `X-Diary-Trace-Id`; the server accepts only 32 lowercase hex characters, otherwise generates one, and echoes it; the same ID appears in client and server records of one request.
- [ ] **L10 — Redacted resource detail:** Descriptors expose only kind, validated opaque ID, revision, byte and line counts, per-instance keyed fingerprint, state tag and path class; seeded diary text, token and path strings never appear in any log line, buffer or stderr output.
- [ ] **L11 — Trace reader:** `diary-server trace` selects by trace, operation, job, resource or `recent:<n>` (last n warn/error records, the starting point when no trace ID is known), merges server and client logs by time, revalidates every line against the schema, tolerates a truncated last line, bounds output to 200 records (maximum 500) and 256 KiB, never reads the discovery file, and neutralizes injected instruction text planted in a log line.
- [ ] **L12 — Native smoke:** Packaged server and client leave 0700/0600 logs, a seeded-secret scan finds nothing, and the trace command reconstructs one smoke request across both roles.
- [ ] **L13 — Handoff:** Update contribution guide (agent debugging procedure), AGENTS.md commands, docs index, phase plan, roadmap, handoff row E12 and changelog; archive nothing until the milestone is Done.
- [ ] **G01:** Pass `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
- [ ] **G02:** Pass `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [ ] **G03:** Pass `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] **G04:** Pass `npx prettier --check .`.
- [ ] **G05:** Pass `npx eslint . --max-warnings 0`.
- [ ] **G06:** Pass `npx vitest run`.

## Evidence record

No evidence yet; implementation has not started.
