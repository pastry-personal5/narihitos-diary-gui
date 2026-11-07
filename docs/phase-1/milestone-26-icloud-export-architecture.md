# P1-M26: iCloud export work plan

Status: Delivered (2026-11-07)

Stages and scenarios for the [overview](milestone-26-icloud-export-overview.md). The behavioral contract is [iCloud projection](../architecture/configuration-and-exports.md#icloud-projection); this plan only sequences it.

## Existing code to reuse

- `diary-application::export`: `ExportAdapter` (`observe`, `apply`, `read_back`), `Observation{present, available, fingerprint, digest, ids}`, `Write{Create, Replace}`, `Applied{Done, Failed, Unknown}`, the decision table, confirmation tokens and recovery. The adapter is injected in `diary-server/src/main.rs` (currently `None` outside `fault-injection`).
- `diary-infrastructure/src/export/{store,fake}.rs`: the store and the directory-backed fake are the behavioral reference. The real adapter must pass the same scenarios.
- Native pattern: `source/trash/native.rs` and the pinned `objc2`/`objc2-foundation` dependency (features `NSFileCoordinator`, `NSFileManager`, `NSURL`, `NSError` already enabled).
- Settings: `paths.icloud_entries_root`, root validation and aliasing rules in [configuration](../architecture/configuration-and-exports.md#configuration-schema-and-validation).

## Port changes

Keep the port minimal. Expected changes, each recorded here if made:

0. `apply` must receive the expected fingerprint (see corrections C1); `Write::Replace` carries it or `apply` takes the expected `Observation`.
1. `Observation` needs a reason code for unavailability (`download_needed`, `unreadable`, `coordination_failed`) so the UI can show a typed message; add an allowlisted enum, not free text.
2. `cancel` and `abandon` are service operations, not adapter methods; the adapter needs none.

## Module plan

| Layer          | File                                         | Responsibility                                                                      |
| -------------- | -------------------------------------------- | ----------------------------------------------------------------------------------- |
| infrastructure | `export/icloud.rs` (macOS), `export/icloud/` | Coordinated observe/apply/read-back, staging, fingerprint, availability             |
| application    | `export.rs`                                  | `cancel`, `abandon_uncertain`, reason codes                                         |
| api            | `exports.rs`                                 | Cancel dispatch for exports; `POST /exports/{id}/abandon`                           |
| bridge         | `bridge/export.rs`                           | `abandon_export` command and capability entry                                       |
| ui             | `src/components/`                            | Export action, preview/confirm dialog, status; reuse `ModalDialog`, Global Messages |
| server         | `diary-server/src/main.rs`                   | Register adapter from config                                                        |
| scripts        | `scripts/smoke/export.py`                    | Add real-adapter-on-temp-dir legs                                                   |

## Adapter design

- **Path:** `<root>/YYYY/MM/YYYY-MM-DD.md`; create the year and month directories on first write. Reject symlinked components and any path escaping the canonical root.
- **Observe:** coordinated read (`NSFileCoordinator` read intent) with a bounded timeout. Fingerprint is the SHA-256 of contents plus size; digest is the normalized content digest. A `.icloud` placeholder name or ubiquity download status other than current yields `available = false`, never `present = false`.
- **Create:** write to a staged temp file in the same directory, then publish with a no-clobber rename inside a coordinated write; failure because the target exists becomes divergence (`Unknown` then read-back).
- **Replace:** re-observe inside the coordinated write; if the fingerprint differs from the expected one, return `Failed` without writing (a changed fingerprint needs a new preview). Otherwise atomic replace from the staged file.
- **Read-back:** same as observe; success requires digest equal to the snapshot.
- **Unknown outcomes:** timeouts and ambiguous errors return `Unknown`; the service reads back and never replays.
- Staged input stays until the operation is terminal so a retry exports the fixed revision.
- Native coordination is cooperative: document that concurrent external writes are detected, not prevented.

## Recovery design

- **Cancel:** an export in `running` stops before its next step; `paused` becomes `cancelled` and releases the reservation; a step already past its side effect is finished by read-back. No remote rollback.
- **Abandon uncertain:** requires a fresh read-back preview, then a one-use confirmation (the M25 token model). Outcome records "abandoned" on the operation, deletes the snapshot, releases `export:<destination key>` and the reload block. The remote file is not touched.

## Stages and tasks

Each task is test-first.

### Stage A - adapter (T01-T05)

1. T01 path and bytes: exact Markdown, year/month creation, symlink and escape rejection.
2. T02 no-clobber and replace: create refuses an existing file; replace refuses a changed fingerprint; read-back verifies.
3. T03 availability: injected placeholder and download-needed states give `available = false`; absent after receipt needs confirmation.
4. T04 concurrent write: an external change between observe and publish yields divergence, staged bytes kept, never `Succeeded`.
5. T05 wiring: valid root registers the adapter, missing or invalid root keeps targets `disabled`.

### Stage B - recovery (T06-T07)

6. T06 cancel at each state, no rollback, reservation released.
7. T07 abandon uncertain: read-back preview, one-use confirmation, restart safe, reload block released.

### Stage C - UI (T08)

8. T08 component tests for the dialog and status, keyboard path, accessible names, `cloud_unavailable` and uncertain messages, SF Symbols through the existing host.

### Stage D - proof and docs

9. Extend `scripts/smoke/export.py` with the real adapter on a temp directory (create, no-op, external edit confirmation, deletion, kill-and-recover). Run the full gate once. Update docs per C11.

## Plan review corrections (2026-11-07)

Checked against the P1-M25 code.

- **C1 TOCTOU:** the port's `apply(date, bytes, write)` has no expected fingerprint, so "replace requires the observed fingerprint" cannot be enforced inside the coordinated write. Service `run` re-observes first, but an external write can land between that and `apply`. Fix: pass the expected observation into `apply`; the adapter re-reads and returns `Failed` on mismatch.
- **C2 No-clobber:** `std::fs::rename` overwrites. Create: stage in the same directory, then `renamex_np(..., RENAME_EXCL)` (the pinned `libc` is already a dependency); EEXIST is divergence. Replace: fingerprint recheck then `rename` inside the `ForReplacing` accessor. Never create the final name with `O_EXCL` and write into it (a partial file could sync).
- **C3 Timeouts:** `coordination.rs` has no timeout. Run each accessor on a worker thread, wait with a deadline, call the coordinator's `cancel` on expiry, and have the closure check an "abandoned" flag so a late accessor never writes. A timed-out apply returns `Unknown`.
- **C4 Test seam:** T03 cannot drive real provider states. Add a small injectable probe (`fn(&Path) -> Provider`) with the real one reading `NSURLUbiquitousItemDownloadingStatusKey`; `nil` means local and available.
- **C5 States:** `ExportState` has no `cancelled` and `result()` maps `OperationState::Cancelled` to `Failed`. Add the variant, update the UI/API types and `is_terminal`. Cancel sets a flag read by `run` before `apply`; `Paused` cancels directly; the reservation and snapshot are released at terminal.
- **C6 Abandon:** C07 mixed two paths. Read-back match resolves to Succeeded with a receipt; a mismatch needs the preview plus one-use confirmation, records "abandoned" and releases the reservation. Abandon needs its own route and token reuse of the M25 confirmation table.
- **C7 Permissions:** a shell-launched server may lack macOS Files and Folders access to iCloud Drive. Map EPERM/EACCES to `permission_denied`.
- **C8 Staging leftovers:** staged files use a dot-prefixed name and are removed on every outcome; a crash leftover is removed at the next observe of that date.
- **C9 Smoke:** `scripts/smoke/export.py` runs the fault-injection binary. Real-adapter legs need a build without the fake plus a config with `icloud_entries_root`; state which binary each leg uses.
- **C10 UI:** the Export action flushes first and sends the saved revision it received; a dirty or failed save blocks with the save error, never exporting stale bytes.

## Risks and notes

- iCloud Drive behavior (download-needed, eviction, upload timing) cannot be forced in tests; injected provider states cover logic only. State this in the completion record and carry live proof to P1-M32.
- Coordination can block; every call needs a timeout and a typed failure.
- The macOS adapter must compile out cleanly on other targets, like the Trash native module.

## Delivery notes (2026-11-07)

- Added a `list_exports` bridge command (GET unfinished exports) so the UI can find an `uncertain` export after a restart.
- `square.and.arrow.up` joined the allowlisted SF Symbols.
- The abandon preview uses `Reason::Uncertain` and a placeholder `Write::Replace`; its token is accepted only by the abandon route.
- A crash between the abandon step and its terminal write leaves the export `running`; recovery re-proves or re-parks it as `uncertain`, and the user can abandon again.
- Non-iCloud folders have no provider state and are always available (O5). Real iCloud Drive proof (download-needed, eviction, staging-file sync, TCC) is carried to P1-M32.
