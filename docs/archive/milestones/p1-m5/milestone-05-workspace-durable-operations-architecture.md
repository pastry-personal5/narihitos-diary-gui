# P1-M5: Workspace and durable operations work plan

Status: Done

Implementation plan for the [P1-M5 overview](milestone-05-workspace-durable-operations-overview.md). It follows the [persistence](../../../architecture/domain-and-persistence.md) and [API](../../../architecture/api-and-processes.md) contracts and keeps the inward dependency rule of [architecture](../../../architecture.md). Each task ends with tests that pass and one commit. Run the affected tests while iterating and the full gate once at the end.

## Placement

| Layer             | Crate                  | Adds                                                                                                                                          |
| ----------------- | ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Application ports | `diary-application`    | `OperationId`, `StepId` (extend the existing `OpaqueId` helpers), `OperationLedger` and `Reservations` traits, `OperationState`, `Checkpoint` |
| Infrastructure    | `diary-infrastructure` | `workspace` (init/open/classify), `sqlite` (worker, migrations, integrity), `ledger` (trait implementation), `reservations`                   |
| Process           | `diary-server`         | Startup calls open-or-initialize, maps the outcome to readiness and log codes                                                                 |
| API               | `diary-api`            | `readiness` field in the status DTO only; no new route                                                                                        |

`WorkspaceLock::acquire` stays the single entry; today it requires a pre-made lock file. Task 1 lets it create `.diary/workspace.lock` when the tree is uninitialized.

## Departures made during implementation

Recorded in the [changelog](../../../phase-1/changelog.md); the tasks below describe the intent, this list overrides them.

- **One mutex-guarded connection, not a worker thread.** `Db::with`/`transact` serialize every call, which is what the contract requires; async callers use `spawn_blocking`. Upgrade to a thread only if a measured need appears.
- **Reservations are derived from the ledger (Task 5).** No separate `Reservations` trait or in-memory map: the keys of unfinished operations live in `operation_resources`, are checked in the admitting transaction, fail fast as `Reserved`, and are released by `finish`. This is durable by construction and cannot deadlock because nothing waits.
- **Readiness reuses `storage_ready`.** The status and settings DTOs already carry the boolean to the UI; adding a field would touch the bridge and TypeScript for no consumer. Mode names are in the log codes `workspace.opened`, `workspace.inspection_required`, `workspace.recovery` and `operation.blocked`.
- **Root rule.** A root with files but no `.diary` is refused by `WorkspaceLock::acquire` (`RootNotEmpty`) before anything is created. This is stricter than the plan and protects an existing diary folder.
- **No fault-hook for the database.** Interrupted migrations are tested with a really failing migration; interrupted initialization uses a test-only `Fault` at the two risky points (database created, marker temp written).

## Tasks

### Task 0: Approval and contract amendments

- Ask the owner to approve `rusqlite` pinned exactly, with `bundled` (no system SQLite dependence; SQLite is public domain, `rusqlite`/`libsqlite3-sys` MIT). Record exact version, features, transitive licences from `cargo tree`/lock inspection, and the notices step in the changelog. Stop here without approval.
- Amend the persistence doc: the database file is `.diary/state.sqlite3`; recovery-mode readiness names; the operation hash is SHA-256 over a versioned canonical encoding.
- Commit: docs only.

### Task 1: Workspace initialization

Files: `diary-infrastructure/src/workspace.rs` (+ `workspace/tests.rs`), `lifecycle.rs` (lock creation).

Interface:

```rust
pub enum WorkspaceState { Uninitialized, Ready { workspace_id: String }, InspectionRequired(Reason) }
pub fn inspect(root: &Path) -> Result<WorkspaceState, WorkspaceError>;
pub fn initialize(lock: &WorkspaceLock) -> Result<Workspace, WorkspaceError>; // lock proves exclusivity
```

Order under the lock: create `.diary` 0700, create DB with schema (Task 2), sync, then write `workspace.json` to a temp name, sync, rename, sync parent. Marker last. `inspect` rejects symlinks and non-regular entries in the managed tree, treats "private files without marker" as `InspectionRequired`, and treats an empty directory or one holding only a lock file as `Uninitialized`.

Tests: success path; a fault injected before each step (via a small `Fault` hook enum passed in tests) leaves `Uninitialized` or a resumable tree and a retry succeeds; marker-without-DB is `InspectionRequired`; symlinked `.diary` refused; second `acquire` is `WorkspaceBusy`.

### Task 2: SQLite worker and migrations

Files: `diary-infrastructure/src/sqlite.rs`, `sqlite/migrations/0001_init.sql`, tests.

Interface: `Db::open(path) -> Result<Db, DbError>` returning a handle that sends closures to one worker thread; `Db::call<T>(f: impl FnOnce(&mut Connection) -> T) -> Result<T, DbError>`. On open set `foreign_keys=ON`, `journal_mode=WAL`, `synchronous=FULL`, `busy_timeout`; verify with a read-back. Migrations are embedded `(version, sql)` with a SHA-256 checksum recorded in `schema_migrations`; apply each in one transaction and set `user_version` last inside it. Migration 1 creates only `schema_migrations`, `operations`, `operation_steps`. Later milestones add theirs.

Tests: fresh open at version 1; reopen is a no-op; edited checksum refused (`state_corrupt`); `user_version` above known refused; migration killed mid-way (fault between statements) leaves the previous version; pragma read-back.

### Task 3: Integrity and recovery mode

`Db::check() -> Integrity { Ok, Missing, Damaged }` using `PRAGMA quick_check` plus a required-tables/columns check. `open_workspace` returns `Mode::Recovery` for `Missing` (marker exists, DB absent) and `Damaged`. Recovery mode never creates a DB, never deletes; the ledger traits return `Unavailable` for writes.

Tests: delete DB then open, assert no DB file recreated; truncate DB then open; ledger write refused in recovery; files untouched (byte compare).

### Task 4: Operation and step ledger

Files: `diary-application/src/operations.rs` (types, trait), `diary-infrastructure/src/ledger.rs`.

```rust
pub trait OperationLedger {
    fn admit(&self, op: NewOperation) -> Result<Admission, LedgerError>;   // New | Existing(Record) | Mismatch
    fn get(&self, id: &OperationId) -> Result<Option<Record>, LedgerError>;
    fn advance(&self, id: &OperationId, step: NewStep) -> Result<StepOutcome, LedgerError>;
    fn finish(&self, id: &OperationId, result: Terminal) -> Result<(), LedgerError>; // tombstone, content-free
}
```

`NewOperation` carries kind, hash version, resource keys, `input_hash` (computed by a pure `hash_input(kind, version, fields)` over a length-prefixed canonical encoding that excludes session and trace fields). `admit` is one transaction: insert-or-compare. `advance` compares `expected_checkpoint` inside the transaction and inserts the step row; a duplicate step returns the recorded outcome before checkpoint comparison. No token, raw output or candidate text is stored; a schema-level `CHECK` length bound on `result`.

Tests: new/existing/mismatch; hash ignores session and trace fields, changes with every business field; duplicate step returns recorded result even after checkpoint moved; two threads race one checkpoint, exactly one wins; tombstone persists across reopen; initial step atomic with operation (fault between inserts leaves neither).

### Task 5: Reservations

`diary-application` trait `Reservations { fn acquire(&self, keys: &[ResourceKey], owner: &OperationId) -> Result<Guard, Busy>; }`. Keys are canonical strings sorted before acquiring. In-process map guarded by a mutex; the durable side is the `resource_keys` of non-terminal operations loaded at startup and re-asserted for `paused`/`uncertain` states. A terminal state releases. Reads never take reservations.

Tests: overlapping sets from two threads in opposite order never deadlock (bounded wait, `Busy` otherwise); paused op reservation survives reopen; unrelated key acquires; terminal release frees the key.

### Task 6: Startup classification

`classify(ledger, identity) -> Vec<Classified>` where `identity = {workspace_id, canonical_root, native_id}` is stored at admission and compared at startup. Outcomes: `Resume` (same identity), `Blocked(RootChanged | AdapterChanged)`, `Terminal`. Blocked operations keep their reservation. Per-kind reconciliation is a trait object registry that is empty in this milestone; an unregistered kind is classified `Blocked(NoReconciler)` and quarantined, never marked failed or succeeded.

Tests: same identity resumes; moved root blocks with readable reason; unknown kind quarantined; classification does not mutate rows except an explicit `state` update recorded with a step.

### Task 7: Server wiring, readiness, logging

`diary-server` startup: acquire lock, `inspect`, `initialize` if `Uninitialized`, open DB, classify, then publish discovery. `Mode::Recovery` and `InspectionRequired` still start (reads and status work) but report non-ready mutation. Add allowlisted log codes: `workspace.initialized`, `workspace.opened`, `workspace.refused`, `workspace.recovery`, `operation.blocked`, with typed fields only (count, reason tag, workspace ID is an opaque ID). `diary-api` status DTO gains `readiness`. Update `scripts/smoke/server.py` to start from an empty directory instead of pre-seeding the lock tree. Update the contribution-guide line that says initialization is deferred.

Tests: API test for status readiness in each mode; log-line scan for seeded secret and path; smoke script: initialize, stop, restart, second instance refused.

### Task 8: Handoff and gate

Update docs listed in W12, tick checklist items with evidence, run `scripts/gate.sh`.

## Acceptance matrix

| ID  | Tasks | Evidence                                                  |
| --- | ----- | --------------------------------------------------------- |
| W01 | 0     | Changelog entry with approval and licence result          |
| W02 | 1     | `workspace/tests.rs` fault-point matrix                   |
| W03 | 1, 7  | Refusal tests plus byte-identical tree comparison         |
| W04 | 2     | Migration and pragma tests                                |
| W05 | 3     | Recovery tests asserting no recreated or deleted files    |
| W06 | 4     | Ledger tests, restart tombstone                           |
| W07 | 4     | Duplicate and concurrent step tests                       |
| W08 | 5     | Reservation tests with restart                            |
| W09 | 6     | Classification tests                                      |
| W10 | 7     | API readiness tests, log scan                             |
| W11 | 7     | `python3 scripts/smoke/server.py` against the real binary |
| W12 | 8     | Doc diff and link check                                   |

## Limits and risks

- Crash points are simulated with fault hooks, not by killing the process or pulling power; real `F_FULLFSYNC`/directory durability on macOS belongs to P1-M6's save evidence.
- iCloud, TCC and packaged-app behaviour stay unverified; this milestone uses a temporary local directory only.
- `GET /operations/{id}` and its Tauri command are deliberately absent so no command inventory changes; P1-M6 adds them with its first consumer.
