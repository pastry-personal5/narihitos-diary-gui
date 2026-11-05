# P1-M5: Workspace and durable operations overview

Status: Done

Implement locked workspace initialization, the versioned SQLite authority, durable operation/step identities, resource reservations and startup recovery classification from the [persistence contract](../architecture/domain-and-persistence.md) and [operation identities](../architecture/api-and-processes.md#wire-types-and-preconditions). The work plan is [milestone-05-workspace-durable-operations-architecture.md](milestone-05-workspace-durable-operations-architecture.md).

## Entry conditions

- P1-M4 is Done (closed by the owner on 2026-11-05; its L12 packaged-client smoke rests on the owner's reported run).
- **`rusqlite` approved by the owner on 2026-11-05** (`=0.41.1`, `bundled`); no other new crate or build tool is used. See W01.
- Read handoff row E2 and the P1-M5 row. Nothing in `AGENTS.md` "Undecided" blocks this milestone.

## Scope

| Included in P1-M5                                                                                                                      | Deferred to                                                                        |
| -------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Initialize an empty workspace under the OS lock: `.diary` 0700, DB schema, `workspace.json` published last as the marker               | Authentic Markdown, staged saves, `pending/` payloads, catalogs: P1-M6             |
| Refuse unexpected private files without the marker (inspection required); never overwrite                                              | Interactive repair or backup UI: not planned in Phase 1 unless the owner asks      |
| SQLite worker: foreign keys, WAL, `synchronous=FULL`, busy timeout, `user_version`, checksummed migrations, integrity check            | Catalog, source, import, export and Trash tables (their milestones add migrations) |
| Operation ledger: admit by `(workspace_id, operation_id)` with versioned input hash, mismatch detection, tombstones                    | HTTP `GET /operations/{id}` and `get_operation` command: first consumer is P1-M6   |
| Step ledger: `(operation_id, step_id)` dedup with checkpoint transitions                                                               | Real operation kinds (save, Trash, export) and their business preconditions        |
| Resource reservations: sorted canonical keys, in-process exclusion plus durable retention for paused/uncertain work                    | Job queue limits (32 / 8 reserved): P1-M21+ with jobs                              |
| Startup classification of incomplete operations, bound to workspace ID and root identity; recovery/read-only mode on DB loss or damage | Per-kind reconciliation (staged file, Trash, remote): P1-M6, Trash and export work |
| Server startup wiring, readiness reporting, log event codes, smoke script updated to initialize rather than pre-seed the lock tree     | Settings reload through reservations: P1-M8                                        |

## Completion checklist

Check an item only with a linked result naming the tested revision, command, observed outcome and limitations. [Acceptance matrix](milestone-05-workspace-durable-operations-architecture.md#acceptance-matrix) rows use the same W IDs; G IDs are the unchanged repository gate.

- [x] **W01 — Approval and contracts:** Owner approves `rusqlite` (version, features, licence result) in the changelog; contract text amended where the plan departs from it.
- [x] **W02 — Initialization:** An empty directory becomes a workspace; `workspace.json` is published last; every interruption point leaves either an uninitialized or a resumable tree, never a marker without a valid DB.
- [x] **W03 — Refusal:** A tree with private files but no marker, a symlink inside the managed tree, a second server, or a root that collides with another configured root is refused without modification.
- [x] **W04 — Database:** Pragmas as specified; `user_version` and `schema_migrations` agree; a changed migration checksum or a newer schema is refused; an interrupted migration rolls back whole.
- [x] **W05 — Integrity:** Missing DB with a marker, or failed authoritative-table checks, enters recovery mode; no empty ledger is created and no file is deleted.
- [x] **W06 — Operation ledger:** First admit records the operation before any side effect; an identical retry returns the recorded state; changed business input returns `operation_mismatch`; tombstones survive restart.
- [x] **W07 — Step ledger:** A duplicate `(operation_id, step_id)` returns its recorded outcome; two concurrent steps cannot both advance one checkpoint; an initial step is created atomically with its operation.
- [x] **W08 — Reservations:** Overlapping key sets serialize without deadlock; paused and uncertain operations keep their reservation across restart; unrelated resources stay available.
- [x] **W09 — Startup classification:** Each incomplete operation is bound to workspace ID and canonical root identity; a repointed root pauses it with a readable blocker; nothing is reinterpreted against the new root.
- [x] **W10 — Server integration:** Startup initializes or opens the workspace, reports `storage_ready` (true only when `ready`; the mode is logged as `workspace.opened`, `workspace.inspection_required` or `workspace.recovery`), logs allowlisted events, and exposes no path, token or content.
- [x] **W11 — Native smoke:** The real `diary-server` binary initializes a temporary workspace, restarts, reopens it, and refuses a second instance; logs and trace output contain no seeded secret.
- [x] **W12 — Handoff:** Update contribution guide, AGENTS.md if commands change, docs index, phase plan, roadmap, handoff rows E2/P1-M5 and changelog.
- [x] **G01:** Pass `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
- [x] **G02:** Pass `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [x] **G03:** Pass `cargo test --manifest-path src-tauri/Cargo.toml`.
- [x] **G04:** Pass `npx prettier --check .`.
- [x] **G05:** Pass `npx eslint . --max-warnings 0`.
- [x] **G06:** Pass `npx vitest run`.

## Evidence record

Working tree on top of `e70c17c`, branch `main`, Apple Silicon macOS, 2026-11-05. Test runs went through the test-runner.

W01 | Owner message "Approve dependencies" (this session) and [changelog](changelog.md); `rusqlite =0.41.1` with `bundled`; `cargo tree` and crate metadata show MIT for `rusqlite`/`libsqlite3-sys` and MIT or MIT/Apache-2.0 for the others; bundled SQLite is public domain; `collect-notices.py` regenerated the inventory (+8 Cargo records, clean prettier) | Note: `rsqlite-vfs` 0.2.0 (wasm-only, not compiled here) ships no license file, so its notice is the upstream `LICENSE` fetched from the crate's exact commit by `fetch-notice-supplements.py`.

W02–W03 | `workspace/tests.rs` (15, including an orphan `-wal` without a database requiring inspection, a truncated database with a marker staying 0 bytes in recovery, and an unreadable ledger row refusing `ready`) and `lifecycle/tests.rs`: empty directory to Ready with 0600 files, marker last, both interruption points resumable with the same workspace ID, stale empty DB reused, unexpected files, extra root files, symlinked DB and symlinked `.diary` refused byte-identically, non-empty root untouched, second owner `WorkspaceBusy` | pass | Limit: interruptions are injected, not process kills; no power-loss test.

W04–W05 | `sqlite/tests.rs` (8; WAL and SHM are asserted 0600) and `workspace/tests.rs`: pragmas read back, `user_version`, altered checksum, newer schema, failing migration rolled back whole, garbage/missing/table-dropped DB and a foreign workspace ID all enter recovery with no file created or deleted | pass.

W06–W08 | `ledger/tests.rs` (12): admit new/existing/mismatch and invalid input, tombstones across reopen, idempotent finish, step dedup before checkpoint check, 8-thread race on one checkpoint, atomic first step, overlapping reservations refused, paused/uncertain reservations across reopen | pass | Limit: concurrency is threads in one process; cross-process exclusion is the OS lock.

W09 | `operations/tests.rs` (3) and the workspace move test: moved root and reused inode block as `RootChanged`, unregistered kinds as `NoReconciler`, rows unchanged, reservation kept | pass | Limit: no reconciler exists yet, so nothing resumes.

W10–W11 | `diary-api` test `storage_ready_follows_workspace_readiness_in_status_and_settings` and `python3 scripts/smoke/server.py` against the real binary (including a restart after deleting the database: read-only `storage_ready` false, `workspace.recovery` logged, no database recreated): non-empty folder refused untouched, empty folder initialized, `storage_ready` true, 0700/0600 tree, marker identical across a crash restart, no token in the marker, `workspace.opened` logged, second instance refused | PASS | Limit: ordinary build only; the packaged-client (GUI) half of the smoke was not rerun for P1-M5.

G01–G06 | `scripts/gate.sh` plus `cargo fmt --all --check` on the working tree above, 2026-11-05: all six PASS; the Rust suite passed 175 tests (0 failed); Vitest 30 passed in 7 files | session output, not retained in the repo. An earlier run failed `cargo fmt --all --check` and the notice test (`rsqlite-vfs` had no notice); both were fixed and the gate rerun.
