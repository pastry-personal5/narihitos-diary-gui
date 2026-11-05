# P1-M10: Import hardening follow-up

Status: Done (2026-11-05)

Completed the P1-M10 follow-up to fix scan consistency, misplaced-file coverage, paging cost and missing regression evidence. The original [completion record](milestone-10-civil-date-imports-overview.md) remains historical evidence; this checklist governs the follow-up.

## Completion checklist

- [x] H01: Record the approved snapshot lifetime and refresh/resolution contract; implement stable pagination without repeated whole-tree scans.
- [x] H02: Discover and warn about out-of-window directories containing in-window header timestamps, under shared scan bounds.
- [x] H03: Separate unrelated native-smoke changes into their own local commit without changing final file contents.
- [x] H04: Apply page bounds before JSON conversion, including mixed-category page boundaries.
- [x] H05: Test batch source binding transaction rollback/idempotency, incomplete-only discard and cancellation throttling/latching.
- [x] H06: Pass the full gate and server, imports and Authentic smoke scripts; record results.

## Design and evidence

Owner approved durable snapshots and the explicit new-import refresh boundary on 2026-11-05. No dependencies or build tools changed.

- H02: `source::import::tests::misplaced_header_inside_window_is_discovered_outside_directory_window` passes, along with all eight import tests. Whole-tree discovery shares the existing 10,000-candidate, 100,000-entry and 16 MiB retained-metadata bounds across roots. Unresolved observations outside the date window are warnings; resolved observations are filtered by the captured zone.
- H03: Local history now separates `c20446d` (import pipeline), `1f03433` (native-smoke readiness) and `40740c3` (import cancellation). The final HEAD tree was verified identical before/after; `backup/imports-before-split` retains the prior history. Nothing was pushed.
- H04/H05: All 37 API tests pass, including cancellation polling/latching and mixed-category page boundaries. Repository tests pass for batch idempotency and rollback after ID/tuple collisions and invalid keys, and discard of incomplete rows only.
- Initial broader Rust run under the sandbox: 99 infrastructure tests pass; 22 fail in native coordination, watcher and port-binding fixtures. The subsequent full gate outside the sandbox passed every check.

- H06: `scripts/gate.sh` passes G01–G07 (rustfmt, Clippy, Rust tests, Prettier, ESLint, Vitest and TypeScript). `python3 scripts/smoke/server.py`, `python3 scripts/smoke/imports.py` and `python3 scripts/smoke/authentic.py` all pass with native macOS coordination and loopback access outside the sandbox. `git diff --check` passes. Packaged GUI interaction was not rerun.

## Approved snapshot decision

Persist a bounded, body-less listing snapshot in SQLite for each completed `import_id`, published atomically with the completion marker through a new checksummed migration. Persist ordering, classification, relative identities, observation tokens and incomplete/failure/collision metadata; never source bodies or front matter. Preserve the existing per-import bounds and load only bounded metadata. Do not change existing migrations or add dependencies.

Listing pages read this immutable snapshot, including after restart. Edits, additions, removals and timestamp choices cannot reorder an existing import's pages. A new import is the explicit refresh boundary. Timestamp resolution locates only the snapshot's source identity, validates the configured root and fresh bytes, then records the digest-bound authority; the refreshed import applies the choice. A changed/missing source fails resolution instead of silently scanning for a replacement. Contexts predating snapshots require re-import and return an explicit stale-context error; never silently reconstruct a different listing under the old ID.

This changes the current contract in which listing re-observes files and immediately applies resolutions. The owner approved this contract after review. Implementation and regression evidence pass; all follow-up checklist items are complete.

## Snapshot acceptance evidence

- Migration 4 adds bounded immutable `import_snapshots` without modifying released migrations. Snapshot insertion and context completion share a transaction; repository tests prove rollback when completion fails, repeat-publication refusal, immutable payloads, oversized-publication refusal and legacy-context re-import behavior. The migration fixture upgrades a real v3 context while preserving its provenance and requiring a new import.
- The versioned infrastructure-only codec preserves all listing/classification ordering, tokens, failures, collisions and incomplete/budget metadata, without adding serialization dependencies to inner layers. A bounded writer enforces the 16 MiB encoded limit. Codec tests cover failure/collision/incomplete round trips, invalid data/index rejection and size-limit refusal.
- A 120-source import fixture preserves its observations across resolution, edits, additions, deletion and database restart. Cancellation polling stays below ten calls during a selected-source resolution, proving it no longer traverses the tree. Changed and missing sources fail resolution; a fresh import applies the saved choice and current filesystem state.
- The real-server import smoke verifies unchanged old snapshots after resolution/restart/byte edits, explicit refreshed inclusion and stale-byte rejection, plus absence of seeded source-body and absolute-root values in SQLite snapshot payloads, HTTP responses and logs.
- Final validation after snapshot implementation: full gate G01–G07 PASS; server, imports and Authentic smoke scripts PASS. Final documentation formatting and `git diff --check` PASS. Native/provider/cloud qualification and packaged GUI interaction remain outside this follow-up.
