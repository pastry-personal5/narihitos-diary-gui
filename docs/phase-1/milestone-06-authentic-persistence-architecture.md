# P1-M6: Authentic persistence and recovery work plan

Status: Done

Implementation plan for the [P1-M6 overview](milestone-06-authentic-persistence-overview.md). It follows the [persistence](../architecture/domain-and-persistence.md) and [API](../architecture/api-and-processes.md) contracts and the inward dependency rule of [architecture](../architecture.md). Each task ends with passing tests and one commit. Run affected tests while iterating and the full gate once at the end.

## Placement

| Layer          | Crate                  | Adds                                                                                                                                                   |
| -------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Domain         | `diary-domain`         | `DiaryDate`, `AuthenticRevision`, `AuthenticHeader`, `render`/`parse` for schema 1, `FormatError`. Pure: no clock, zone or I/O                         |
| Application    | `diary-application`    | `AuthenticStore` port, `SaveRequest`/`CreateRequest`/`SaveOutcome`, `Clock` and `ZoneSource` ports, save use case, `authentic:YYYY-MM-DD` resource key |
| Infrastructure | `diary-infrastructure` | `authentic_fs` (durable file primitives and fault hooks), `authentic` (store, catalog, reconcilers), migration 2, system zone capture                  |
| API            | `diary-api`            | Four Authentic routes, `GET /operations/{id}`, `POST /authentic/refresh`, DTOs, error mapping                                                          |
| Process        | `diary-server`         | Startup rebuild and recovery after `Workspace::classify`; log codes                                                                                    |

The current `Workspace` owns the ledger; the store borrows it, so a save and its operation record share one authority.

## Owner decisions (A01)

Answered 2026-11-05 and to be copied to the [changelog](changelog.md).

| #   | Decision                                                                                                                         |
| --- | -------------------------------------------------------------------------------------------------------------------------------- |
| Q1  | Approve `notify` now: default FSEvents backend, watching only the Authentic tree; a hint, with rescan authoritative.             |
| Q2  | Authentic Tauri commands and TypeScript DTOs, including `get_operation`, land in P1-M7 (departs from the P1-M5 note).            |
| Q3  | `timezone_database` comes from the macOS tzdata version file; verify at implementation time and stop to ask if it is unreliable. |
| Q4  | Require `NSFileCoordinator` via `objc2` + `objc2-foundation`; required for every Authentic save, failing the save visibly.       |

## Departures made during implementation

This list overrides the tasks below.

- **Closed header grammar, no YAML library.** `parse` is a strict line grammar in `diary-domain`, so duplicate keys, aliases, tags and compound values cannot be expressed and the domain crate stays dependency-free; `serde_yaml_ng` is not used for Authentic files.
- **Revision digest lives in infrastructure.** `diary-domain` holds `AuthenticRevision` but not SHA-256; `sha_hex` and `revision_of` are in `diary-infrastructure`.
- **Prepared intent is the operation's first step.** Its outcome text carries date, old and new digests and sequence; a new `OperationLedger::step_outcome` reads it back. No `published` step is stored: recovery judges by current bytes.
- **Rejected preconditions are not recorded.** A stale, existing or blocked request returns before admission, so it leaves no operation row. Failures after admission end `failed` with a tag (`exists`, `conflict`, `publish_failed`, `stage_missing`).
- **A recovery conflict finishes the operation as `failed`/`conflict`** and keeps the staged file; it does not hold a reservation on the date (see the persistence doc).
- **One process-wide write lock** serializes Authentic mutations in the server; the OS workspace lock keeps other processes out.
- **Read-only recovery store.** In recovery mode `ReadOnlyStore` serves load and list from the files and refuses everything else, which keeps "files remain readable".
- **Catalog freshness.** The catalog row for a date is refreshed from every observation made before a save; `POST /authentic/refresh` and startup rebuild all of it.
- **Watcher not wired.** `watch` is implemented and tested; the server does not start it until an invalidation event exists.
- **Dependencies.** `notify =8.2.0` (stable, not the 9.0 release candidate), `objc2`, `objc2-foundation` and a direct `block2`, all pinned.
- **Fault injection.** `Store::inject` exists in tests and under the `fault-injection` feature (also read from `DIARY_FAULT`); release builds compile the hooks out.

## Tasks

### Task 0: Questions and contract amendments

Record the Q1–Q4 decisions in the [changelog](changelog.md). Pin `notify`, `objc2` and `objc2-foundation` exactly, record versions, features and licences from `cargo tree`/lock inspection, regenerate the notices as in P1-M5, and amend the persistence doc where the plan departs (`diary_timezone` capture, the refresh route, mandatory coordinated access). Docs and manifests only; stop here if a licence is incompatible with Apache-2.0.

### Task 1: Canonical format (domain)

Files: `diary-domain/src/authentic.rs` (+ `authentic/tests.rs`).

```rust
pub struct DiaryDate(/* y, m, d */); // parse("2026-11-05"), Display, from_ymd; rejects invalid Gregorian dates
pub struct AuthenticRevision { pub sequence: u64, pub sha256: String }
pub struct AuthenticHeader { id, date, timezone, timezone_database, created_at, updated_at, revision: u64, extensions: Vec<(String, Scalar)> }
pub fn render(h: &AuthenticHeader, body: &str) -> Result<Vec<u8>, FormatError>;
pub fn parse(bytes: &[u8], path_date: &DiaryDate) -> Result<(AuthenticHeader, String), FormatError>;
pub fn revision_of(bytes: &[u8]) -> AuthenticRevision; // sha256 of whole file, sequence from header
```

`parse` uses `serde_yaml_ng` into a strict struct for known keys and rejects duplicate keys, aliases, tags and compound extension values before deserializing (scan the YAML events; `unsafe-libyaml` is already present, so use whatever event access `serde_yaml_ng` exposes and fall back to rejecting on a pre-scan of the header text). The body cap is 1 MiB of UTF-8 (`FormatError::BodyTooLarge`). `parse` fails if header `diary_date` differs from the path date.

Tests: golden file round-trip is byte-identical; each rejection class above; a scalar extension survives parse then render; `2026-02-29` and `2026-13-01` rejected, `2028-02-29` accepted; CRLF input is rejected, not normalized on a write path; 1 MiB accepted, +1 refused; emoji and combining text round-trip.

### Task 2: Durable file primitives

Files: `diary-infrastructure/src/authentic_fs.rs` (+ tests), using `libc` only.

```rust
pub struct Fs { fault: Option<FaultPoint> /* test-only */ }
impl Fs {
    pub fn write_private(&self, dir: &Path, name: &str, bytes: &[u8]) -> io::Result<()>; // O_EXCL|O_NOFOLLOW, 0600, F_FULLFSYNC, fsync dir
    pub fn publish_new(&self, tmp: &Path, target: &Path) -> io::Result<()>;      // renamex_np RENAME_EXCL; AlreadyExists on collision
    pub fn publish_replace(&self, tmp: &Path, target: &Path) -> io::Result<()>;  // rename, then F_FULLFSYNC target and parent
    pub fn ensure_dir(&self, path: &Path) -> io::Result<()>;                    // 0700 chain, sync each new parent
    pub fn read_bounded(&self, path: &Path, max: usize) -> io::Result<Observed>; // identity, size, bytes; rejects symlinks
}
pub enum FaultPoint { StageWrite, StageSync, TmpWrite, Publish, FileSync, DirSync }
```

Tests: collision never clobbers; a symlink target or parent is refused; `F_FULLFSYNC` is called (assert through the `Fault` hook counters); permissions 0600 and 0700 on created entries; fault at each point returns an error and leaves the target unchanged.

Coordination: `diary-infrastructure/src/coordination.rs` (+ tests) is the only module with `objc2` FFI. `Coordinator::read<T>(path, f)` and `Coordinator::write<T>(path, f)` wrap `NSFileCoordinator` `coordinateReadingItemAtURL`/`coordinateWritingItemAtURL` with the closure run inside the accessor block; a coordination error maps to `StoreError::Coordination` and nothing is written. `publish_*` and `read_bounded` in Task 4 call through it. Tests: the closure runs once and its result is returned; an error inside the closure propagates; a second process holding a coordinated write blocks the first until it ends; a coordination error leaves the target unchanged.

### Task 3: Migration 2, catalog and rebuild

Files: `sqlite/0002_catalog.sql`, `authentic/catalog.rs` (+ tests).

`catalog_authentic(id PK, date UNIQUE, path UNIQUE, revision, sha256, timezone, size, mtime_ns, inode, parse_outcome)`, STRICT. `rebuild(ws) -> Result<RebuildReport>` walks `assets/diary/authentic/YYYY/MM/*.md` under the lock, refuses symlinks, records unparseable, duplicate-ID and duplicate-date files as parse outcomes (those dates are blocked for writes), deletes only stale catalog rows, never touches `operations` or `operation_steps`. `list(cursor, limit<=200)` is keyset-paged by date; `load(date)` returns `Loaded { bytes_revision, header, body }` or `Virtual` for an absent date.

Tests: rebuild restores a dropped catalog; operations and steps byte-identical after rebuild; duplicate ID and duplicate date flagged; cursor is stable across an insert; page size 200 enforced; absent date is `Virtual`, not a file.

### Task 4: Guarded save and Create Empty

Files: `diary-application/src/authentic.rs`, `diary-infrastructure/src/authentic/store.rs` (+ tests).

```rust
pub trait AuthenticStore {
    fn load(&self, date: &DiaryDate) -> Result<Loaded, StoreError>;
    fn save(&self, req: SaveRequest) -> Result<SaveOutcome, StoreError>;     // { operation_id, date, expected: Option<AuthenticRevision>, body, buffer_generation }
    fn create_empty(&self, req: CreateRequest) -> Result<SaveOutcome, StoreError>;
}
pub enum StoreError { Conflict(ConflictKind), Reserved, Blocked(String), TooLarge, Unavailable, Storage, NotFound }
```

Sequence, matching the contract: `ledger.admit` (kind `authentic_save`/`authentic_create`, key `authentic:<date>`, input hash over date, expected sequence and digest or `absent`, body SHA-256; `buffer_generation` and session fields are excluded) and return the recorded state for an existing matching ID before any precondition. For a new action: read and verify the current file against `expected`; render with the next revision (clock regression never lowers it); write the stage under `.diary/pending/<operation-id>.md` and the target-directory temp; `advance` to `prepared`; publish (`publish_new` for create, `publish_replace` otherwise); verify the digest by read-back; in one SQLite transaction update the catalog and `advance` to `committed`, then `finish(succeeded)`; remove stage and temp; acknowledge. Any failure before publication ends `failed` with the old file intact; any failure after it leaves the operation `uncertain` for reconciliation. `diary_timezone` for creation comes from `ZoneSource` (system zone) and is preserved on later saves.

Tests: success, stale revision, ABA, external edit between read and publish, concurrent create (one wins), existing file refused by Create Empty and left byte-identical, same operation ID twice returns the recorded result, mismatched body under the same ID is `operation_mismatch`, clock regression keeps sequence rising, emptying a body keeps the file.

### Task 5: Interruption matrix and recovery

Files: `authentic/reconcile.rs`, startup hook (+ tests).

Register reconcilers for both kinds so `classify` returns `Resume` rather than `NoReconciler`. Implement the contract's rules in this order: digest equals new bytes → finish the receipt; equals old bytes or absence with a verified stage → retry publication under the same identity; equals neither → `uncertain`/conflict with both copies preserved; stage missing or corrupt with an unchanged file → `failed`. Startup removes an unreferenced app-owned temp (name pattern plus no ledger row) and never a `.md` file.

Tests: a table-driven matrix that injects a fault before and after every durable step of Task 4 (the nine overview points), restarts, and asserts both the final file and the ledger state; external edit during `prepared` is preserved; stage corrupt; stage missing; orphan temp removed; a user `.md` beside it kept; unrelated dates stay editable while one operation is quarantined.

### Task 6: Routes

Files: `diary-api/src/authentic.rs`, `lib.rs` router additions (+ tests).

`GET /authentic?cursor=`, `GET /authentic/{date}`, `PUT /authentic/{date}` (`{operation_id, expected_revision, body, buffer_generation}`), `POST /authentic/{date}/empty` (`{operation_id}`), `GET /operations/{operation_id}`, `POST /authentic/refresh`. All sit behind the existing session admission and workspace authorization, bound bodies to the contract limits, return `409` with a conflict kind, `404 not_found` for an unknown operation, and refuse writes with a readable code while readiness is not `ready`. The store call runs on `spawn_blocking`. Responses and errors carry no body text or path.

Tests: auth required on each route; limit and malformed-date cases; 409 mapping; operation lookup after a rebuilt `ServerState` and from a second session; recovery mode allows load and refuses save; log scan for seeded body text.

### Task 7: Rescan, watcher, logging and smoke

`watch.rs` wraps `notify` (FSEvents) on `assets/diary/authentic`, debounces 250 ms into one `Invalidated` signal, and on error or overflow forces a rescan. Tests: a burst of writes yields one signal; a drop of events yields a rescan; no event content is trusted (the signal carries no path). Rescan runs at startup after recovery, on `POST /authentic/refresh`, and inside every save before the expected-revision check; it compares identity, size and bytes, never mtime alone. Add allowlisted log codes `authentic.saved`, `authentic.created`, `authentic.conflict`, `authentic.recovered`, `authentic.rebuilt`, `authentic.blocked` with typed fields only (counts, reason tag, opaque operation ID). Extend `scripts/smoke/server.py`: create, save, restart, load; `SIGKILL` between publication and receipt (via a test-only environment fault that is compiled out of release) and verify recovery; scan logs for the seeded body and path.

### Task 8: macOS durability evidence, handoff and gate

Run a local-APFS test that exercises `F_FULLFSYNC`, `RENAME_EXCL` and replace, and record the observed result and limits in the overview. Update the docs in A14, tick the checklist with evidence and run `scripts/gate.sh`.

## Acceptance matrix

| ID  | Tasks   | Evidence                                                  |
| --- | ------- | --------------------------------------------------------- |
| A01 | 0       | Changelog entry with Q1–Q4 answers                        |
| A02 | 1       | `authentic/tests.rs` golden and rejection cases           |
| A03 | 1, 4    | Revision and limit tests                                  |
| A04 | 4       | Stale, ABA and external-edit tests                        |
| A05 | 4       | Create Empty tests, byte comparison                       |
| A06 | 2, 4, 5 | Fault matrix over nine boundaries                         |
| A07 | 5       | Reconciler tests per rule                                 |
| A08 | 3       | Rebuild and list tests                                    |
| A09 | 7       | Rescan tests                                              |
| A10 | 2, 4, 6 | Injected sync and disk-full failures; recovery-mode test  |
| A11 | 6       | API tests                                                 |
| A12 | 7       | `python3 scripts/smoke/server.py` against the real binary |
| A13 | 8       | APFS test output and recorded limits                      |
| A14 | 8       | Doc diff and link check                                   |

## Limits and risks

- Crashes are simulated with fault points and one `SIGKILL` smoke, not power loss. `F_FULLFSYNC` guarantees depend on the drive; claim only what Task 8 observes.
- `renamex_np` is macOS-specific. Gate it behind `cfg(target_os = "macos")` with a compile error elsewhere, since the product targets macOS only.
- No iCloud, TCC or packaged-app evidence. Coordination covers cooperating writers only; uncoordinated external writers during the publish window stay outside the write contract.
- Block-based `NSFileCoordinator` callbacks through `objc2` are the likeliest FFI friction; build the Task 2 coordinator spike first and, if it cannot meet A13, stop and ask rather than weaken the requirement.
- FSEvents can drop or coalesce events, so the watcher never replaces rescans.
- The strict YAML pre-scan (duplicate keys, aliases, tags) is the likeliest source of surprise; Task 1 should confirm what `serde_yaml_ng` exposes before writing code.
- Task 4 is the largest task. If it exceeds one reviewable change, split Create Empty from replace-save at the `publish_new`/`publish_replace` seam.
