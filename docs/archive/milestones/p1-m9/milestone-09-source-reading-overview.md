# P1-M9: Source identity and reading overview

Status: Archived (P1-M9 Done)

Read configured Ayumi and Narihito source trees safely: canonical path identities, bounded fresh reads, body/metadata separation and independent per-file outcomes. This milestone is read-only and has no HTTP, bridge or UI surface. The work plan is [milestone-09-source-reading-architecture.md](milestone-09-source-reading-architecture.md). Contracts: [path identity and observations](../../../architecture/sources-and-dates.md#path-identity-and-observations), [parsing and timestamps](../../../architecture/sources-and-dates.md#parsing-and-timestamps), [source research](../../../research/source-formats-and-prior-art.md).

## Entry conditions

- P1-M8 is Done. Read handoff rows E3 and P1-M9.
- Owner approved the reviewed plan and O1/O2 on 2026-11-05: pinned `unicode-normalization` with license-notice review; library-only exposure until P1-M10.

## Scope

| Included in P1-M9                                                                                                                                                                                           | Deferred to                                                       |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Root canonicalization, case-sensitivity probe, NFC/case-folded path key, `path_collision`, opaque `source_id` bound to root and path (stable across reloads; the config generation is bound by the token)   | Import contexts and `list_sources?import_id`: P1-M10              |
| Bounded no-follow read (8 MiB, `O_NONBLOCK`, dataless check before reading), before/after metadata and path-vs-descriptor check, one retry, `source_changing`, SHA-256 of complete bytes, observation token | Timestamp resolution, zone arithmetic, date inclusion: P1-M10     |
| Ayumi and Narihito adapters: BOM, consistent CRLF, stray CR, duplicate keys, YAML aliases/`!` tags, nesting and 64 KiB/64 keys/8 KiB limits, body ≤ 1 MiB kept unchanged, raw front matter retained         | Source Save, Trash, capabilities `write_body`/`trash_pair`: later |
| Per-file outcomes: `ok`, `unavailable` (not downloaded/denied/missing), `malformed`, `unsupported`, `conflict`; one failure never hides valid neighbours                                                    | Explorer, calendar, tabs: P1-M11                                  |
| Header `created_at` and filename/directory UTC stamp validated independently at filename precision; `Dating` carries both pieces of evidence, `timestamp_conflict` reported, no resolution                  | Agent packet attachment-line stripping: AI milestones             |
| Bounded deterministic scan (10,000 candidates, 100,000 entries) returning body-less listings and `incomplete`; body through `read_view`; fixtures built in code                                             |                                                                   |
| Docs, handoff, changelog                                                                                                                                                                                    |                                                                   |

## Completion checklist

Check an item only with a linked result naming the tested revision, command, observed outcome and limitations. Rows use the IDs of the [acceptance matrix](milestone-09-source-reading-architecture.md#acceptance-matrix).

- [x] **R01 — Identity:** NFC and case-fold key (insensitive volumes only), `/` separators, no empty/`.`/`..`/absolute components, non-UTF-8 rejected, display-alias change keeps identity, root move does not. [Evidence](#evidence-record).
- [x] **R02 — Collisions and roots:** two sibling entries with one key give `path_collision` and neither is offered (pure-function test: APFS cannot hold such twins); duplicate/nested/aliased roots rejected; symlinks below the root are not followed; hard-linked files remain distinct paths; a mount point is not crossed. [Evidence](#evidence-record).
- [x] **R03 — Fresh read:** regular file only, no symlink traversal, FIFO not opened for reading, dataless file not read, size bounded before allocation, metadata and path identity compared before/after, one retry then `source_changing`, digest of complete bytes, token binds tuple, digest, file identity, size and generation. [Evidence](#evidence-record).
- [x] **R04 — Encoding and limits:** BOM accepted, consistent CRLF accepted, mixed or stray CR, malformed UTF-8, over-limit file/front matter/keys/scalar/body, duplicate keys, YAML aliases, `!` tags, missing/unclosed front matter and deep nesting each give a distinct `malformed` or limit outcome; arbitrary mutated input never panics. [Evidence](#evidence-record).
- [x] **R05 — Adapters:** valid Ayumi and Narihito fixtures yield body, raw front matter, unknown scalars preserved and no body rewriting; unknown schema/kind is `unsupported` but body stays viewable; missing audio does not fail a valid text file; layout mismatch is `unsupported`, id/stem mismatch is `conflict`; duplicate producer IDs warn on both listings and never merge. [Evidence](#evidence-record).
- [x] **R06 — Timestamps:** header offset must be explicit; naive timestamp, impossible date, filename/directory mismatch and header/filename disagreement at filename precision (second vs. millisecond) are reported; `timestamp_conflict` lists both instants and carries no automatic date. [Evidence](#evidence-record).
- [x] **R07 — Isolation:** a directory with valid, malformed, unavailable, unsupported and conflicting files returns every valid listing plus exactly one outcome per failure, in key order; 10,001 candidates return `incomplete` with a deterministic first 10,000; `read_view` after a byte change is `Stale`. [Evidence](#evidence-record).
- [x] **R08 — Secrecy:** outcomes, tokens and log records carry no absolute path, body or front matter value (`Debug` included); only relative path, codes and counts. No server log code is added in M9. [Evidence](#evidence-record).
- [x] **R09 — Handoff:** update handoff rows E3/P1-M9, phase plan, roadmap, docs index, changelog and sources document status; `AGENTS.md` only if commands change. [Evidence](#evidence-record).
- [x] **G01:** Pass `cargo fmt --manifest-path src-tauri/Cargo.toml --check`. [Evidence](#evidence-record).
- [x] **G02:** Pass `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`. [Evidence](#evidence-record).
- [x] **G03:** Pass `cargo test --manifest-path src-tauri/Cargo.toml`. [Evidence](#evidence-record).
- [x] **G04:** Pass `npx prettier --check .`. [Evidence](#evidence-record).
- [x] **G05:** Pass `npx eslint . --max-warnings 0`. [Evidence](#evidence-record).
- [x] **G06:** Pass `npx vitest run`. [Evidence](#evidence-record).

## Evidence record

Implementation is under validation on base revision `195afb2` plus the P1-M9 working tree (2026-11-05). No commit was requested.

- R01–R08: `cargo test --manifest-path src-tauri/Cargo.toml -p diary-domain -p diary-infrastructure source:: --offline --quiet`: 6 domain source tests and 15 infrastructure source tests pass. Test locations: `diary-domain/src/source/{path,front_matter,stamp}.rs` and `diary-infrastructure/src/source/{tests,read,root,scan}.rs`.
- R01/R02: normalized path groups; canonical/aliased/nested/independent roots; case probe against actual test-volume behavior; forced public-ID collision rejected by full tuple; hard links retain separate IDs. Real mount changes are not injected; no-follow traversal checks every opened device against the root.
- R03: sparse oversized file, FIFO, file/directory symlinks, original-descriptor metadata, one-shot/continuous growth, rename replacement, symlink swap, replaced root, complete digest and hard-link count. Dataless flag decision is synthetic; real File Provider/iCloud behavior remains unverified.
- R04: each parse-error variant, raw body/metadata preservation, BOM, CRLF, attachment syntax, producer quoting and both empty-tags forms; truncation and all single-byte mutations of a synthetic UTF-8 fixture.
- R05/R06: both producer adapters, unknown schema/kind/layout, identity conflict, missing audio, unknown metadata, explicit offsets, precision/calendar validation and independent dating evidence. Read-only parsing does not enable Save or Trash.
- R07/R08: valid/malformed/placeholder/unsupported/conflict isolation, denied neighbour, duplicate IDs, key order, 10,001 candidates returning the first 10,000, entry/depth bounds, placeholder identity collision and stale views including generation changes; redacted report/view/observation/token formatting. Invalid UTF-8 names use the production conversion boundary because APFS refuses such filenames.
- Dependency review: `unicode-normalization` 0.1.25 declares MIT OR Apache-2.0; existing transitive `tinyvec` 1.13.3 declares compatible alternatives. MIT is selected. `python3 scripts/collect-notices.py` regenerated 737 records/404 distinct notice texts, with package copyright and both license texts preserved; no missing notice assets. The plan's `scripts/notices` name was corrected to the existing collector.

The tested revision is base `195afb2` plus the P1-M9 working tree. Its source fingerprint is `6d5493e0e8683d0d685b02d2262aa108524590210304492f991ce652baafacef` (SHA-256 of sorted source paths and bytes, each separated by NUL: source modules in all three layers, architecture test, domain manifest and Cargo lockfile).

R09: handoff E3/P1-M9, phase status, roadmap, index, source contract, README, changelog and notices updated together; completed plans archived with links repaired. Owner approvals recorded in `AGENTS.md`. No server command, API, Tauri permission, UI or log code was added.

Final verification on 2026-11-05: `scripts/gate.sh` passed G01–G07 (the six required commands plus `npx tsc --noEmit`). The Rust tests ran with loopback access because the sandbox rejects localhost binds used by existing bridge tests. Full Rust suites and all 65 Vitest tests passed. `cargo fmt --manifest-path src-tauri/Cargo.toml --all --check` additionally covers every workspace crate; `git diff --check` and local-link checks pass.

The first gate exposed a permission-literal lint and the scaffold's dependency-free domain allowlist, both corrected. The approved normalization crate is the only allowed external domain dependency. Prettier also normalized whitespace in the independently added, untracked `.rust-analyer.json`; its settings were preserved. A final audit added a nested-list regression and a root-move test before the passing gate recorded above.
