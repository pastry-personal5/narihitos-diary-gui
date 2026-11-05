# P1-M6: Authentic persistence and recovery overview

Status: Done

Implement canonical Authentic Markdown, guarded durable saves, server-side Create Empty, the saved-date catalog, external-change detection and interrupted-save recovery from the [persistence contract](../architecture/domain-and-persistence.md#autosave-protocol) on top of the P1-M5 ledger. The work plan is [milestone-06-authentic-persistence-architecture.md](milestone-06-authentic-persistence-architecture.md). The editor, autosave queue and any visible UI are P1-M7.

## Entry conditions

- P1-M5 is Done. Nothing in `AGENTS.md` "Undecided" blocks this milestone.
- **Owner decisions, 2026-11-05:** approved `notify` (default macOS FSEvents backend) and `objc2` + `objc2-foundation`, each pinned exactly; no other new crate. Licence results and notices are recorded in Task 0. Native watching, `NSFileCoordinator` access (required for every Authentic read-before-write and publish) and the Tauri-commands-in-P1-M7 split are decided; the `timezone_database` source is the macOS tzdata file, to be verified (stop and ask if none is reliable).
- Read handoff rows E2 and P1-M6.

## Scope

| Included in P1-M6                                                                                                                                                                            | Deferred to                                                                        |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Pure domain: `DiaryDate`, `AuthenticRevision`, header model, canonical serializer and strict parser (schema 1)                                                                               | Candidate, review and paragraph-accept saves: P1-M2x                               |
| Guarded save protocol: stage in `.diary/pending`, `prepared` intent, no-clobber create or atomic replace, `F_FULLFSYNC`, verify, commit receipt                                              | Autosave queue, debounce, IME flush, generation echo handling in the editor: P1-M7 |
| Create Empty: expected absence, revision 1, empty body, same protocol, existing file is a conflict                                                                                           | Create Empty button and Explorer listing: P1-M7 / P1-M11                           |
| Migration 2: `catalog_authentic`; rebuild from Markdown under the lock; list with cursor (page ≤ 200), load, virtual empty buffer                                                            | Source, import, Trash and export tables                                            |
| Operation kinds `authentic_save` and `authentic_create` with reconcilers; startup recovery of `prepared`/`published` intents                                                                 | Reconcilers for other kinds                                                        |
| HTTP routes `GET /authentic`, `GET /authentic/{date}`, `PUT /authentic/{date}`, `POST /authentic/{date}/empty`, `GET /operations/{id}`                                                       | Tauri commands and TypeScript DTOs for these routes: P1-M7 (first UI consumer)     |
| External-change detection: rescan (startup, `POST /authentic/refresh`, before every write) plus a debounced (250 ms) `notify` FSEvents watcher on the Authentic tree as an invalidation hint | Explorer/editor reaction to invalidation events: P1-M7 / P1-M11                    |
| Coordinated access through `NSFileCoordinator` (`objc2`) for Authentic reads-before-write and publication; log event codes; smoke script; macOS durability evidence on the local filesystem  | iCloud, packaged-app and TCC behaviour: not verified                               |

## Completion checklist

Check an item only with a linked result naming the tested revision, command, observed outcome and limitations. [Acceptance matrix](milestone-06-authentic-persistence-architecture.md#acceptance-matrix) rows use the same A IDs; G IDs are the unchanged repository gate.

- [x] **A01 — Approvals:** Owner approval of `notify` and `objc2`/`objc2-foundation` (versions, features, licence result, notices) is recorded in the changelog; contract text records the coordination requirement and the P1-M7 command split.
- [x] **A02 — Canonical format:** A file round-trips byte-identically; header order, LF, no BOM, one LF after the closing delimiter are fixed; scalar extension fields survive; duplicate keys, aliases, tags, compound extensions, unknown schema, invalid UTF-8, bad dates, and metadata/path/body disagreement are rejected with a readable diagnostic and never repaired.
- [x] **A03 — Revision:** Creation is revision 1; every save increments; a clock regression never lowers it; the digest covers the whole file; a body of exactly 1 MiB saves and 1 MiB + 1 is refused.
- [x] **A04 — Guarded save:** Missing or stale expected revision, external edit, and ABA (same bytes, different sequence) are refused without writing; a valid save publishes, verifies and only then acknowledges `{revision, operation_id, buffer_generation}`.
- [x] **A05 — Create Empty:** Absence creates revision 1 with an empty body and no snippet or agent; an existing file or concurrent create returns a conflict and leaves the file byte-identical; a repeated operation ID returns the recorded outcome; emptying a body saves an empty body and never deletes.
- [x] **A06 — Interruption matrix:** A fault before and after each of: stage write, stage sync, intent commit, publish, file sync, directory sync, verify, receipt commit, acknowledge. Each case reconciles to exactly one of committed, safely retried, or conflict, and a retry with the original ID returns the observed outcome.
- [x] **A07 — Recovery rules:** Equal to new bytes finishes the receipt; equal to old bytes (or absence) with a matching stage retries publication; equal to neither preserves both and marks conflict; a missing or corrupt stage with an unchanged file is a failed save; an unreferenced app-owned temporary file is removed and a user Markdown file never is.
- [x] **A08 — Catalog:** Rebuild from Markdown restores `catalog_authentic` and leaves operations and steps untouched; duplicate IDs or dates and unparseable files appear as parse outcomes and block saving that date; list pages are bounded and cursor-stable.
- [x] **A09 — External changes:** Rescan detects added, edited, deleted and replaced files; a read compares identity, size and bytes, not mtime alone; the watcher coalesces events within 250 ms into one invalidation, drops nothing silently on overflow (it forces a rescan), and never proves race freedom; the load response lets the client tell clean reload from conflict.
- [x] **A10 — Failure modes:** Injected disk-full and sync failures return failure, keep the old file, and leave a recoverable or failed (not succeeded) operation; recovery mode refuses writes while loads still work.
- [x] **A11 — Routes and operations:** The five routes enforce session and workspace authorization, body limits and `not_found` for unknown operations; `GET /operations/{id}` answers after server restart and from a new session; a lost-reply retry gets the recorded result before any precondition is rechecked.
- [x] **A12 — Native smoke:** The real `diary-server` creates, saves, restarts, and reloads a file; kills the process between publication and receipt and recovers it; logs contain no body text or path.
- [x] **A13 — macOS durability and coordination:** `F_FULLFSYNC` on file and directory, `renamex_np(RENAME_EXCL)` no-clobber and atomic replace are exercised on the local APFS volume; every Authentic read-before-write and publish runs inside an `NSFileCoordinator` access and a coordination failure fails the save visibly with the old file intact; a second process writing through its own coordinator is serialized in a test. Results and unverified limits (iCloud, other volumes) are recorded.
- [x] **A14 — Handoff:** Update contribution guide, `AGENTS.md` if commands change, docs index, phase plan, roadmap, handoff rows E2/P1-M6, architecture API table notes and changelog.
- [x] **G01:** Pass `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
- [x] **G02:** Pass `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [x] **G03:** Pass `cargo test --manifest-path src-tauri/Cargo.toml`.
- [x] **G04:** Pass `npx prettier --check .`.
- [x] **G05:** Pass `npx eslint . --max-warnings 0`.
- [x] **G06:** Pass `npx vitest run`.

## Evidence record

Working tree on top of `2d0ba9f`, branch `main`, Apple Silicon macOS (APFS temp volume), 2026-11-05. Test runs went through the test-runner. Departures from the plan are in the [work plan](milestone-06-authentic-persistence-architecture.md#departures-made-during-implementation).

A01 | Owner interview answers (this session) and [changelog](changelog.md); `notify =8.2.0` (CC0-1.0, FSEvents only), `objc2 =0.6.5`, `objc2-foundation =0.3.2` (MIT) and `block2 =0.6.2` (MIT, already a transitive dependency of `objc2-foundation`, now direct because the accessor block is built here); `fetch` pulled the all-platform crates so `collect-notices.py` could regenerate the inventory (+7 notice files) | `block2` as a direct dependency was not named in the interview.

A02–A03 | `diary-domain` `authentic/tests.rs` (9): golden round trip, unicode, scalar extensions, 13 unsupported-YAML shapes, BOM/CR/UTF-8/schema/date/kind/revision rejections, field order, 1 MiB boundary, Gregorian dates | pass | Limit: the header grammar is hand-written and deliberately closed (see departures); files written by other tools with different quoting are rejected, not repaired.

A04–A05 | `authentic/tests.rs`: stale, ABA, wrong-date, repeated-ID, mismatched-ID, `Exists` byte comparison, emptying a body, clock regression, 1 MiB boundary with no side effects | pass.

A06 | `every_crash_boundary_reconciles_to_one_complete_file`: crash before and after each of the nine boundaries (stage write, stage sync, intent, publish, file sync, directory sync, verify, receipt, acknowledge) for both replace and Create Empty (36 cases), restart, retry of the original ID, then file, ledger, stage and temp-file checks; `failing_system_calls_fail_cleanly_before_publication_and_are_uncertain_after`: 18 injected I/O failures | pass | Limit: crashes are returned errors with no cleanup, plus one real `SIGKILL` (A12); disk-full is an injected error, not a full volume.

A07 | `a_missing_or_corrupt_stage…`, `an_external_edit_during_a_prepared_save_is_preserved…`, `startup_removes_only_app_owned_orphans`, `a_quarantined_operation_blocks_only_its_own_date` | pass.

A08–A09 | `rebuild_restores_the_catalog…`, `listing_is_bounded_and_cursor_stable`, `rescans_see_added_edited_and_deleted_files`; `watch/tests.rs` (burst coalesces to one path-free signal, quiet tree sends none, errors and changes invalidate but plain reads do not) | pass | Limit: the watcher is a tested module; no server component consumes its signal yet, because no invalidation event exists until the Explorer (P1-M11).

A10 | the I/O-failure matrix above and `recovery_mode_keeps_files_readable_and_refuses_every_mutation` (no ledger recreated) | pass.

A11 | `diary-api` `authentic_routes` (6, with a fake store): session on every route, no store, round trip with request mapping, malformed input rejected before the store, 13 store errors mapped to stable codes without echoing body text; the lost-reply and new-session cases ran in A12 | pass | Limit: Tauri commands and TypeScript DTOs are P1-M7.

A12 | `python3 scripts/smoke/authentic.py` against the real `diary-server` built with `--features fault-injection`: create, conflict, save, restart, reload, list and `GET /operations/{id}` from a new session; `SIGKILL` between publication and receipt; the next start settles the operation, the original ID returns the recorded revision, no stage remains; logs contain no body text or workspace path and carry `authentic.created`, `.saved`, `.recovered`, `.rebuilt` | PASS.

A13 | `authentic_fs/tests.rs` (`F_FULLFSYNC` on file and directory, `RENAME_EXCL` no-clobber, atomic replace, private modes, symlink refusal, APFS check) and `coordination/tests.rs` (closure runs once and returns its value, absent files, error propagation, two threads serialized, a second process holding a coordinated write blocks ours for at least one second) | pass | Limits: local APFS only; not iCloud, other volumes or power loss; `F_FULLFSYNC` is requested, not independently proven to reach the platter.

A14 | Persistence and API contracts, contribution guide, `AGENTS.md` smoke command, docs index, phase plan, roadmap, handoff and changelog updated in this change.

G01–G06 | `scripts/gate.sh` on the working tree above, 2026-11-05: all six PASS; Rust suite 243 passed, 0 failed; Vitest 30 tests in 7 files; additionally `cargo clippy -p diary-server -p diary-infrastructure --all-targets --features diary-server/fault-injection -- -D warnings` passes | session output, not retained in the repo. Earlier runs failed clippy (`large_enum_variant`, `result_large_err`, `collapsible_if`), the architecture test (new `diary-api` → `diary-domain` edge) and `cargo fmt --all --check`; all were fixed and the gate rerun.
