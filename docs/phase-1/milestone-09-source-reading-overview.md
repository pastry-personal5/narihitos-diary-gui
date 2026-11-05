# P1-M9: Source identity and reading overview

Status: Planned

Read configured Ayumi and Narihito source trees safely: canonical path identities, bounded fresh reads, body/metadata separation and independent per-file outcomes. This milestone is read-only and has no HTTP, bridge or UI surface. The work plan is [milestone-09-source-reading-architecture.md](milestone-09-source-reading-architecture.md). Contracts: [path identity and observations](../architecture/sources-and-dates.md#path-identity-and-observations), [parsing and timestamps](../architecture/sources-and-dates.md#parsing-and-timestamps), [source research](../research/source-formats-and-prior-art.md).

## Entry conditions

- P1-M8 is Done. Read handoff rows E3 and P1-M9.
- [Owner decisions](milestone-09-source-reading-architecture.md#owner-decisions-needed) O1 (NFC dependency) and O2 (API exposure) are answered before Task 1. No dependency or feature flag is approved yet; stop and ask.

## Scope

| Included in P1-M9                                                                                                                                                           | Deferred to                                                       |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Root canonicalization, case-sensitivity probe, NFC/case-folded path key, `path_collision`, opaque `source_id` bound to root, path and config generation                     | Import contexts and `list_sources?import_id`: P1-M10              |
| Bounded no-follow read (8 MiB), before/after metadata check, one retry, `source_changing`, SHA-256 of complete bytes, observation token                                     | Timestamp resolution, zone arithmetic, date inclusion: P1-M10     |
| Ayumi and Narihito adapters: BOM, consistent CRLF, stray CR, duplicate keys, aliases/tags, nesting and 64 KiB/64 keys/8 KiB limits, body ≤ 1 MiB, raw front matter retained | Source Save, Trash, capabilities `write_body`/`trash_pair`: later |
| Per-file outcomes: `ok`, `unavailable` (not downloaded/denied/missing), `malformed`, `unsupported`, `conflict`; one failure never hides valid neighbours                    | Explorer, calendar, tabs: P1-M11                                  |
| Header `created_at` and filename/directory UTC stamp validated independently at filename precision; `timestamp_conflict` reported, no resolution                            | Agent packet attachment-line stripping: AI milestones             |
| Bounded scan (10,000 candidates) reporting `incomplete`; synthetic fixtures only                                                                                            |                                                                   |
| Docs, handoff, changelog                                                                                                                                                    |                                                                   |

## Completion checklist

Check an item only with a linked result naming the tested revision, command, observed outcome and limitations. Rows use the IDs of the [acceptance matrix](milestone-09-source-reading-architecture.md#acceptance-matrix).

- [ ] **R01 — Identity:** NFC and case-fold key (insensitive volumes only), `/` separators, no empty/`.`/`..`/absolute components, non-UTF-8 rejected, display-alias change keeps identity, root move does not.
- [ ] **R02 — Collisions and roots:** two entries with one key give `path_collision` and neither is offered; duplicate/nested/aliased roots rejected; symlinks below the root are not followed; hard-linked files remain distinct paths.
- [ ] **R03 — Fresh read:** regular file only, no symlink traversal, size bounded before allocation, metadata compared before/after, one retry then `source_changing`, digest of complete bytes, token binds tuple, digest, file identity, size and generation.
- [ ] **R04 — Encoding and limits:** BOM accepted, consistent CRLF accepted, mixed or stray CR, malformed UTF-8, over-limit file/front matter/keys/scalar/body, duplicate keys, aliases/tags and deep nesting each give a distinct `malformed` or limit outcome.
- [ ] **R05 — Adapters:** valid Ayumi and Narihito fixtures yield body, raw front matter, unknown scalars preserved and no body rewriting; unknown schema/kind is `unsupported` but body stays viewable; missing audio does not fail a valid text file.
- [ ] **R06 — Timestamps:** header offset must be explicit; naive timestamp, impossible date, filename/directory mismatch and header/filename disagreement at filename precision (second vs. millisecond) are reported; `timestamp_conflict` lists both instants and carries no automatic date.
- [ ] **R07 — Isolation:** a directory with valid, malformed, unavailable, unsupported and conflicting files returns every valid file plus one outcome per failure; 10,001 candidates return `incomplete`.
- [ ] **R08 — Secrecy:** outcomes, tokens and log records carry no absolute path, body or front matter value; only relative path, codes and counts.
- [ ] **R09 — Handoff:** update handoff rows E3/P1-M9, phase plan, roadmap, docs index, changelog and sources document status; `AGENTS.md` only if commands change.
- [ ] **G01:** Pass `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
- [ ] **G02:** Pass `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [ ] **G03:** Pass `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] **G04:** Pass `npx prettier --check .`.
- [ ] **G05:** Pass `npx eslint . --max-warnings 0`.
- [ ] **G06:** Pass `npx vitest run`.

## Evidence record

Not started.
