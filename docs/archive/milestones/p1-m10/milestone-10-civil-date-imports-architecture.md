# P1-M10: Civil-date imports and timestamp resolution work plan

Status: Archived — P1-M10 completed 2026-11-05

Implementation plan for the [P1-M10 overview](milestone-10-civil-date-imports-overview.md). It follows [sources and dates](../../../architecture/sources-and-dates.md) and the inward dependency rule of [architecture](../../../architecture.md). Each task ends with passing affected tests; commit only when the owner asks (`AGENTS.md`). Run the full gate once at the end. Fixtures are synthetic and built in code (reuse the M9 `Tree` helper); no private samples.

## Placement

| Layer          | Location                                                       | Adds                                                                                                                                    |
| -------------- | -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| Domain         | `diary-domain/src/source/window.rs` (new)                      | `ImportZone`, `Zone` trait (`local_date`, `earliest_start`), `import_window`, `include`, ordering; pure, tested with a fake zone        |
| Application    | `diary-application/src/source.rs`, `imports.rs` (new)          | `ImportContext`, `ImportListing`, `Resolution`, store ports; `import_id` in `ObservationToken`                                          |
| Infrastructure | `diary-infrastructure/src/zone.rs` (new), `sqlite/`, `source/` | Zone adapter (jiff, O1), system zone and tzdb version capture (O3), `import_contexts`/`time_resolutions` migration, import scan wrapper |
| Server/API     | `diary-api/src/imports.rs` (new)                               | `POST /imports`, `GET /sources?import_id`, `POST /sources/{id}/time-resolution`; session admission as in `authentic.rs`                 |
| Client/bridge  | `src-tauri/src/bridge/`                                        | Typed commands `import_date`, `list_sources`, `resolve_source_time` (per O2), individual permissions; no UI                             |

Domain stays free of a calendar library: it receives a `Zone` and does only the set logic, so every boundary case is a table test with a fake zone, and jiff is exercised separately against real tzdb data.

## Owner decisions

Resolved by the owner on 2026-11-05: O1 pinned `jiff`, O2 server routes plus bridge commands without UI, O3 system tzdb with its `+VERSION`. Recommendations below are the approved choices.

- **O1 — Date dependency.** Architecture selects Jiff ([configuration contract](../../../architecture/configuration-and-exports.md#version-and-platform-strategy)). `jiff 0.2.39` is already in `Cargo.lock` transitively, but a direct dependency in `diary-infrastructure` needs approval: pinned `=`, features limited to what zone lookup needs, license review and `python3 scripts/collect-notices.py`. Alternative: Chrono + Chrono-TZ (two crates, default gap/fold handling must be overridden). Recommended: Jiff, with the explicit disambiguation the contract requires.
- **O2 — Exposure.** Recommended: server routes plus typed bridge commands now, no UI (the Explorer in P1-M11 consumes them). Alternative: server routes only, bridge in P1-M11. M9 promised exposure "until P1-M10" without fixing this.
- **O3 — tzdb version source.** The contract requires a recorded database version. Jiff can use the system database (`/var/db/timezone/zoneinfo/+VERSION` reads `2026e` on the research Mac) or its bundled copy. Recommended: read the system database and its `+VERSION`; if the version file is unreadable, fail the import rather than store an unknown version. Unverified: whether this path is stable across macOS releases.

## Contract decisions made by this plan

1. **Window set.** `import_window(D, zone)` returns the three civil dates, the half-open instant interval `[earliest_start(D-1 or next valid), earliest_start(D+2 or next valid))`, and the UTC directory dates it intersects. `D` itself with no valid instant gives `invalid_diary_date`.
2. **Inclusion** uses `zone.local_date(resolved_instant)` ∈ {D-1, D, D+1}. Source offset is provenance only.
3. **Resolved instant** is the `Agreed` instant, or the stored resolution's chosen authority when its `(source_key, digest)` matches the current observation. `Disagree`, `HeaderOnly`, `FilenameOnly` and `Undated` without a matching resolution are warnings, not inclusions (a single valid authority still needs an explicit choice, per the contract).
4. **Filename choice** is refused while the dating state carries a `DirMismatch` filename error; the response names the external fix.
5. **`import_id`** is server-issued, opaque, and bound into tokens. The M9 `source_id` stays unchanged, so re-import upserts the same identity.
6. **Context lifetime.** The row is written when the zone is captured, `completed_at` when the scan finishes. A lease expiry cancels the scan job and leaves an incomplete row, which is ignored by listings.

## Tasks

### Task 1: Window and inclusion (domain)

Files: `diary-domain/src/source/{window.rs,mod.rs}`. Pure functions over a `Zone` trait; tests use a table-driven fake zone (fixed offsets, a 23-hour day, a 25-hour day, folded midnight, gap midnight, a skipped date). Covers D02, D03, D05 ordering.

### Task 2: Jiff zone adapter (infrastructure)

Files: `diary-infrastructure/src/zone.rs`, `Cargo.toml` (after O1/O3). `SystemZone::capture() -> Result<(ImportZone, impl Zone), ZoneError>`. Tests against real tzdb names: `Asia/Seoul`, `America/New_York` DST transitions (23/25 h), `Asia/Kolkata` (+05:30), `Australia/Lord_Howe` (30-minute DST), a skipped date (`Pacific/Apia`, 2011-12-30), leap day and year end. Unnamed zone gives `ZoneError::Unnamed`. Covers D01, D02, D03.

### Task 3: Persistence

Files: `diary-infrastructure/src/sqlite/` migration for `import_contexts` and `time_resolutions` as specified in [persistence](../../../architecture/domain-and-persistence.md#sqlite-responsibilities). Tests: round trip, restart, resolution lookup misses on a different digest, foreign-key and bounded-size checks. Covers D06 storage, D07.

### Task 4: Import scan

Files: `diary-infrastructure/src/source/import.rs`, `diary-application/src/imports.rs`. `import_date(date, config) -> ImportReport` captures the zone, builds the window, runs `scan` over the intersecting UTC directories (reusing the M9 candidate and entry bounds), classifies each listing as included, warning (conflict/undated/unresolved) or misplaced, and orders the included list. Tests: mixed tree with conflict, undated, misplaced and neighbour-date files; `incomplete` propagation; second zone on re-import. Covers D04, D05, D07.

### Task 5: Resolution

Files: `diary-application/src/imports.rs`. `resolve_time(source_id, digest, authority)` re-observes the file, rejects a stale digest, rejects `filename` when its stamp is invalid, stores the choice. Tests: each dating state, byte change invalidates, source bytes unchanged. Covers D06.

### Task 6: API and bridge

Files: `diary-api/src/imports.rs`, `src-tauri/src/bridge/`, capability permissions. Reuse the session admission, body bounds, redaction and job patterns of `authentic.rs`. Tests: unadmitted session rejected, absolute path in a request rejected, error bodies carry no path/body, manifest/invoke-handler/permission inventories agree. Covers D08.

### Task 7: Docs

Update handoff E3/P1-M10, phase plan, roadmap, `docs/README.md`, sources document implementation status, changelog; archive nothing until Done.

## Acceptance matrix

| ID  | Tests                                    |
| --- | ---------------------------------------- |
| D01 | Task 2 capture tests, Task 4 context row |
| D02 | Task 1 table tests, Task 2 tzdb tests    |
| D03 | Tasks 1, 2 skipped-date tests            |
| D04 | Task 4 interval and misplaced tests      |
| D05 | Tasks 1, 4 ordering and warning tests    |
| D06 | Tasks 3, 5 resolution tests              |
| D07 | Tasks 3, 4 zone-change test              |
| D08 | Task 6 API and inventory tests           |
| D09 | Task 7 doc review                        |

## Review focus

- Date 2011-12-30 in `Pacific/Apia` as D: `invalid_diary_date`, no instants invented.
- Neighbour D+1 skipped: D+2 start is the next valid start; no 72-hour addition.
- File at 23:30 local on D-1 whose UTC date is D-1 or D: included either way (inclusion uses the converted date, not the directory).
- A resolution made, then one byte appended: back to unresolved, warning again.
- System zone changes between two imports of the same date: two contexts with their own zone; Authentic dates unchanged.
- tzdb version unreadable: import fails with a visible code, nothing stored.
- Filename directory disagrees with the filename: `filename` choice refused.

## Departures made during implementation

- System-zone capture reads macOS's `/etc/localtime` target on every import rather than using `TimeZone::try_system()`: Jiff caches that detection for five minutes. Named rules are still loaded through pinned Jiff from the canonical system database directory, paired with `+VERSION`. Unnamed copied zone files fail. Shell `TZ` overrides do not replace the Mac's configured system zone.
- Gap disambiguation uses the actual following transition, including a gap starting before midnight; simply shifting midnight by the gap duration can miss the first valid instant. A synthetic POSIX-rule test covers that case alongside the real-tzdb matrix.
- Ordering stays in the infrastructure import wrapper, where the private canonical roots are available for tie breaking; the domain contains only calendar/window/inclusion logic.
- The scanner shares its 10,000-candidate/100,000-entry budgets across roots, checks cancellation during traversal, and bounds retained metadata to 16 MiB. Two worker slots bound concurrent source operations. A failed/unavailable root is visible and marks the scan incomplete.
- `POST /imports` accepts `{date, request_id}` and returns HTTP 202 with `{import_id, job}`. The server-generated import ID is also its job ID; identical session requests reuse admission. Date/config generation have an injective fixed-width encoding in the coordinator's 64-hex request-key field. No extra hashing dependency is needed.
- `GET /sources?import_id=…&cursor=…` rescans current source observations against the immutable context and returns pages of at most 100 included/warning/failure/collision records. It retains no source body or editable copy. If the installed tzdb version changes, old-context listings fail explicitly rather than silently reinterpret provenance; re-import captures the new version.
- Resolution takes `{import_id, digest, authority}` and uses a fresh observation, current config generation and digest. A stale import after config reload requires re-import. Read-only/recovery workspaces do not offer imports. The three commands are registered in Rust with individual main-window permissions; the Explorer and its TypeScript/UI integration remain P1-M11.
- Import job failures expose allowlisted `error_code` through `get_job`; existing job SSE notifications and cancellation are reused. Import rows have a one-way completion marker and source identities are collision-checked before resolution FK insertion.
- Migration tests now derive the next schema version instead of assuming version 2 is latest. A separate v2→v3 fixture proves existing workspace rows survive.
- Pinned Jiff 0.2.39 uses direct `std`/`tzdb-zoneinfo` features, with no direct bundled database or system-detection feature. Its existing MIT/Unlicense notice assets were reviewed (MIT selected); `python3 scripts/collect-notices.py` regenerated the unchanged 737-package/404-notice inventory.
