# P1-M5: Workspace and durable operations overview

Status: Planned

Implement locked workspace initialization, the versioned SQLite authority, durable operation/step identities, resource reservations and startup recovery classification from the [persistence contract](../architecture/domain-and-persistence.md) and [operation identities](../architecture/api-and-processes.md#wire-types-and-preconditions). The work plan is [milestone-05-workspace-durable-operations-architecture.md](milestone-05-workspace-durable-operations-architecture.md).

## Entry conditions

- P1-M4 is Done (closed by the owner on 2026-11-05; its L12 packaged-client smoke rests on the owner's reported run).
- **Owner approval needed: `rusqlite`.** The [dependency table](../architecture/configuration-and-exports.md#dependency-candidates) names it only as a preferred candidate, and [AGENTS.md](../../AGENTS.md) requires approval before adding it. Task 0 of the work plan asks for it with exact version, features and licence check. No other new crate or build tool is planned.
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

- [ ] **W01 — Approval and contracts:** Owner approves `rusqlite` (version, features, licence result) in the changelog; contract text amended where the plan departs from it.
- [ ] **W02 — Initialization:** An empty directory becomes a workspace; `workspace.json` is published last; every interruption point leaves either an uninitialized or a resumable tree, never a marker without a valid DB.
- [ ] **W03 — Refusal:** A tree with private files but no marker, a symlink inside the managed tree, a second server, or a root that collides with another configured root is refused without modification.
- [ ] **W04 — Database:** Pragmas as specified; `user_version` and `schema_migrations` agree; a changed migration checksum or a newer schema is refused; an interrupted migration rolls back whole.
- [ ] **W05 — Integrity:** Missing DB with a marker, or failed authoritative-table checks, enters recovery mode; no empty ledger is created and no file is deleted.
- [ ] **W06 — Operation ledger:** First admit records the operation before any side effect; an identical retry returns the recorded state; changed business input returns `operation_mismatch`; tombstones survive restart.
- [ ] **W07 — Step ledger:** A duplicate `(operation_id, step_id)` returns its recorded outcome; two concurrent steps cannot both advance one checkpoint; an initial step is created atomically with its operation.
- [ ] **W08 — Reservations:** Overlapping key sets serialize without deadlock; paused and uncertain operations keep their reservation across restart; unrelated resources stay available.
- [ ] **W09 — Startup classification:** Each incomplete operation is bound to workspace ID and canonical root identity; a repointed root pauses it with a readable blocker; nothing is reinterpreted against the new root.
- [ ] **W10 — Server integration:** Startup initializes or opens the workspace, reports readiness (`ready`, `recovery`, `inspection_required`) in status, logs allowlisted events, and exposes no path, token or content.
- [ ] **W11 — Native smoke:** The real `diary-server` binary initializes a temporary workspace, restarts, reopens it, and refuses a second instance; logs and trace output contain no seeded secret.
- [ ] **W12 — Handoff:** Update contribution guide, AGENTS.md if commands change, docs index, phase plan, roadmap, handoff rows E2/P1-M5 and changelog.
- [ ] **G01:** Pass `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
- [ ] **G02:** Pass `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [ ] **G03:** Pass `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] **G04:** Pass `npx prettier --check .`.
- [ ] **G05:** Pass `npx eslint . --max-warnings 0`.
- [ ] **G06:** Pass `npx vitest run`.

## Evidence record

Empty until implementation.
