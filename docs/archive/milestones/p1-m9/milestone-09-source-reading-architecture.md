# P1-M9: Source identity and reading work plan

Status: Archived (P1-M9 Done)

Implementation plan for the [P1-M9 overview](milestone-09-source-reading-overview.md). It follows [sources and dates](../../../architecture/sources-and-dates.md) and the inward dependency rule of [architecture](../../../architecture.md). Each task ends with passing affected tests; commit only when the owner asks (`AGENTS.md`). Run the full gate once at the end. Fixtures are synthetic and built in code (see [Fixtures](#fixtures)); no private samples.

## Placement

| Layer          | Location                                         | Adds                                                                                                                          |
| -------------- | ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------- |
| Domain         | `diary-domain/src/source/` (new, framework-free) | `SourcePath` (key + spelling), sibling grouping, front-matter grammar, `Instant` parsing and comparison                       |
| Application    | `diary-application/src/source.rs`                | `SourceListing`, `SourceView`, `SourceOutcome`, `ObservationToken`, `ScanReport`                                              |
| Infrastructure | `diary-infrastructure/src/source/` (new)         | Root resolver, volume probe, bounded no-follow reader, Ayumi/Narihito adapters, scanner; reuses `SourceKind` from `config.rs` |
| Server/API     | none                                             | Exposure is deferred (O2)                                                                                                     |

Domain code stays free of YAML libraries, as for Authentic ([M6](../../../archive/milestones/p1-m6/milestone-06-authentic-persistence-architecture.md)): a closed line grammar rejects duplicate keys, aliases, tags and nesting by construction. Nothing here writes to a source. `diary-infrastructure` already depends on `sha2`, `libc` and `objc2-foundation` (with `NSString`); no other dependency is needed except O1.

## Owner decisions needed

Resolved by the owner on 2026-11-05: approve the reviewed plan, pinned `unicode-normalization` with notice review (O1), and library-only exposure (O2). See the [changelog](../../../phase-1/changelog.md).

- **O1 — NFC dependency.** Path keys need Unicode NFC. Owner approved adding `unicode-normalization` (MIT/Apache-2.0) to `diary-domain`, pinned `=` like the other dependencies; run `python3 scripts/collect-notices.py` to regenerate notices and confirm its transitive crates' licenses as part of implementation. Rejected alternative: `NSString precomposedStringWithCanonicalMapping` through `objc2-foundation` (the `NSString` feature is already enabled, so no feature change). It is macOS-only, ties results to the OS's Unicode version and forces a normalization port, because the call must sit in infrastructure; the domain tests would then need a fake.
- **O2 — Exposure.** Recommended: no HTTP/bridge surface and no test hook in `diary-server`. Adapters are proved by library tests in `diary-infrastructure` (module-local synthetic tests). `list_sources` takes an `import_id` that P1-M10 introduces, and a temporary `GET /sources` would be reshaped there. The M8 precedent is that reload logged no new server codes; M9 likewise adds none (see Task 8).

## Contract decisions made by this plan

These resolve gaps in the first draft. Items 1–2 refine wording in [sources and dates](../../../architecture/sources-and-dates.md), edited together with this plan.

1. **`source_id` does not include the config generation.** `config_generation` (`ServerState`, bumped by every applied reload) would change every ID on each reload, defeating "re-import upserts the same identity" and selection retention. `source_id` = first 32 lowercase hex of SHA-256 over a length-prefixed `(canonical root, normalized key)`; it is a valid `OpaqueId`. The generation is bound by the `ObservationToken` only. A lookup compares the stored tuple, so a hash collision cannot alias two paths.
2. **A scan returns listings, not bodies.** Retaining up to 10,000 bodies of 1 MiB would be 10 GiB; the architecture says cached body text lives in bounded memory. `scan` reads and parses each file sequentially, drops its bytes, and keeps a body-less `SourceListing`. `read_view` re-reads one file for the body. Its fresh token must equal the listing's token or the result is `Stale`.
3. **Outcome order** (first match wins), so every file gets exactly one class:

| Step | Check                                                             | Result                                                                                                                 |
| ---- | ----------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| 1    | Fresh read fails                                                  | `Unavailable(Missing \| Denied \| NotDownloaded)`; `NotRegular`/`TooLarge`/`SourceChanging`/`Io` are `Unavailable` too |
| 2    | `parse` fails                                                     | `Malformed(ParseError)`                                                                                                |
| 3    | Path does not match the adapter layout, or schema/kind unknown    | `Unsupported(Layout \| Schema \| Kind)`, body still viewable                                                           |
| 4    | `id` does not equal the stem identity (Ayumi stem; Narihito UUID) | `Conflict(IdMismatch)`, body still viewable                                                                            |
| 5    | Otherwise                                                         | `Ok`, with one `Dating` state (a timestamp disagreement is `Dating`, never an outcome)                                 |

4. **`Dating` carries raw evidence**, because [sources and dates](../../../architecture/sources-and-dates.md#parsing-and-timestamps) requires an explicit choice even when only one authority is valid:

```rust
pub enum Dating {
    Agreed(Instant),                                             // both valid and equal at the coarser precision
    Disagree { header: Instant, filename: Instant },             // timestamp_conflict
    HeaderOnly { header: Instant, filename: StampError },
    FilenameOnly { filename: Instant, header: StampError },
    Undated { header: StampError, filename: StampError },
}
```

P1-M10 turns these into inclusion and resolution; M9 never picks a winner.

## Fixtures

Build fixtures in code with a `Tree` test helper that writes bytes into a `tempfile` directory. Do not check in `.md` fixtures: `npx prettier --check .` would reformat them, git may rewrite CRLF, and BOM/invalid UTF-8/stray CR cannot be kept reliably in text files. The helper emits valid Ayumi and Narihito files with overridable fields and raw-byte mutators.

## Tasks

### Task 1: Path identity

Files: `diary-domain/src/source/{mod.rs,path.rs}`, tests inline in `tests.rs`.

```rust
pub struct SourcePath { pub key: String, pub spelling: String } // spelling: actual fs relative path; key: per component NFC, then fold if insensitive
pub fn normalize(rel: &str, insensitive: bool) -> Result<SourcePath, PathError>;
pub fn group_siblings(names: &[&str], insensitive: bool) -> Vec<Vec<usize>>; // indices of entries that share a key (groups of 2+)
pub enum PathError { Empty, Absolute, Dot, DotDot, NonUtf8, ControlChar }
```

Fold = NFC(lowercase(NFC(component))); `str::to_lowercase` is not APFS's table, so the key may over-report a collision (visible, never silent) and never under-report ASCII/Latin cases. Document this limit in code. Non-UTF-8 `OsStr` is rejected by infrastructure with `NonUtf8` before calling `normalize`. A `SourcePath` built from stored data is re-validated with `normalize(spelling, insensitive)` and its key compared before use.

Tests (R01): NFD input equals NFC key; `A.md` and `a.md` equal only when `insensitive`; `x//y`, `./x`, `../x`, `/x`, empty and NUL/control components rejected; `group_siblings` finds NFC/NFD and case pairs, and a 3-way group.

### Task 2: Root resolver, volume probe and IDs

Files: `diary-infrastructure/src/source/{mod.rs,root.rs}`, tests.

```rust
pub struct ResolvedRoot { pub canonical: PathBuf, pub insensitive: bool, pub dev: u64, pub ino: u64 }
pub enum RootError { Missing, NotDirectory, Denied, Duplicate, Nested, NotDownloaded }
pub fn resolve_roots(sources: &[Source]) -> Vec<Result<ResolvedRoot, RootError>>; // same order; never all-or-nothing
pub fn source_id(root: &ResolvedRoot, path: &SourcePath) -> OpaqueId;
```

- One bad source must not hide valid ones: results are per source. Both members of a duplicate, nested or aliased pair get `Duplicate`/`Nested`; there is no winner. Compare canonical paths and `(dev, ino)`.
- Probe case behaviour without creating anything: `fpathconf(root_fd, _PC_CASE_SENSITIVE)` (verify `libc` exposes it; otherwise define the Darwin constant with a comment). If the answer is unavailable, assume insensitive: the worst effect is a visible false collision, never a missed one. The first draft's "create a temp name in a scratch directory" is dropped: the scratch directory would have to be on the source's volume, and `runtime_root` may not be.
- `read_fresh` and `scan` re-open the root and compare `(dev, ino)` to the resolved root; a mismatch is `Unavailable(Missing)` (root replaced).

Tests (R01, R02): symlink alias of a root equals the root; nested and duplicate roots both rejected while a third valid root still resolves; a renamed alias gives the same `source_id`; two roots give different IDs for one path; a forced hash collision is rejected by the tuple check; the probe result equals observed behaviour (create `a` and `A` in a tempdir) on whatever volume the tests run on.

### Task 3: Bounded fresh reader

Files: `diary-infrastructure/src/source/read.rs`, tests.

```rust
pub struct Observation { pub bytes: Vec<u8>, pub sha256: [u8; 32], pub size: u64, pub dev: u64, pub ino: u64, pub nlink: u64, pub mtime_ns: i128, pub ctime_ns: i128 }
pub fn read_fresh(root: &ResolvedRoot, rel: &SourcePath) -> Result<Observation, ReadError>;
pub enum ReadError { Missing, Denied, NotDownloaded, NotRegular, TooLarge, SourceChanging, Io }
```

Algorithm, repeated at most twice:

1. Walk components with `openat(O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)` from the root descriptor; open the file with `O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC`. `O_NONBLOCK` is required: opening a FIFO without it blocks forever. Reuse the `diary_logging::private_fs` descriptor patterns but not its 0700/0600 checks, which are for private data.
2. `fstat` before reading: not regular → `NotRegular`; `st_flags & SF_DATALESS` (`0x4000_0000`; define the constant) → `NotDownloaded` **without reading**, because a read materializes the file; size > 8 MiB → `TooLarge` before any allocation.
3. Read through `take(8 MiB + 1)`. More bytes than `st_size` or than the limit means the file grew: `SourceChanging` (second attempt) or retry (first).
4. `fstat` again and compare `(size, mtime, ctime, ino, dev)` to the first; also `fstatat(parent, name, AT_SYMLINK_NOFOLLOW)` and compare `(dev, ino)` to the descriptor, so a file replaced by rename or symlink during the read is caught.
5. Digest the complete bytes with `sha2`.

Map errno: `ENOENT` → `Missing`, `EACCES`/`EPERM` → `Denied`, `ELOOP`/`ENOTDIR` on a no-follow open → `NotRegular`, `EINTR` retried, others → `Io`. `ReadError` never carries `io::Error` or a path, so `Debug` cannot leak one. Provide an inner `read_with(.., between: impl FnMut())` that the tests use to mutate the file between steps; no environment fault hook is added to production code.

Tests (R03): symlinked file and symlinked directory component refused; FIFO refused without hanging; 8 MiB + 1 refused before allocation (assert via a sparse file); a `between` closure that appends once gives success on retry, always gives `SourceChanging`; a rename-replace during the read is caught; digest equals an independent `sha256` of the bytes; `nlink` is 2 for a hard link. Limit: `SF_DATALESS` and real iCloud behaviour stay unverified against a real provider (handoff evidence gap); a unit test covers only the flag decision on a synthetic `stat`.

### Task 4: Front-matter grammar and encoding

Files: `diary-domain/src/source/{front_matter.rs,text.rs}`, tests.

```rust
pub enum Newline { Lf, CrLf }
pub enum Value { Scalar(String), List(Vec<String>) }
pub struct Parsed { pub raw_front_matter: String, pub fields: Vec<(String, Value)>, pub body: String, pub newline: Newline, pub bom: bool }
pub fn parse(bytes: &[u8]) -> Result<Parsed, ParseError>;
pub enum ParseError { FileTooLarge, Utf8, StrayCr, MixedNewlines, MissingFrontMatter, Unclosed, DuplicateKey, Alias, Tag, TooDeep, FrontMatterTooLarge, TooManyKeys, ScalarTooLarge, BodyTooLarge, BadLine }
```

Rules to fix in code and tests:

- Input ≤ 8 MiB (checked again here as defence). Optional UTF-8 BOM. First line is exactly `---`; a missing opener is `MissingFrontMatter`. Closing line is exactly `---`; stop looking for it after 64 KiB so a huge unclosed block is `FrontMatterTooLarge`, not a scan of the whole file.
- Newlines apply to the whole file, body included: `Lf` has no CR; `CrLf` has every LF preceded by CR and no other CR. A CR anywhere else is `StrayCr`; a mix is `MixedNewlines`. A file with no newline is `Lf`.
- `body` is the original text after the closing line, **unchanged, CRLF retained**; `newline` and `bom` are recorded. Converting to LF for the editor, and restoring on save, belong to P1-M11 and the source-save milestone. `BodyTooLarge` counts original bytes (> 1 MiB).
- Line grammar: `key: value`, key `[A-Za-z0-9_]+`, case-sensitive. Value is a plain, single-quoted or double-quoted scalar, `[]`, or a flat list of scalars (inline `[a, b]` or block `- a`). A plain scalar starting with `&` or `*` is `Alias`; with `!` is `Tag`; a nested mapping, nested list or indented continuation is `TooDeep`; anything else unparsable is `BadLine`. Confirm the accepted quoting and empty-list forms against the reference `LegacyDiaryReader.swift` and `DiarySnippetMarkdownCodec.swift` while writing the tests (see the [research](../../../research/source-formats-and-prior-art.md#r11-ayumi)). The Ayumi `tags` key is an ordinary list field and is unrelated to YAML tags.
- Limits: 64 KiB front matter, 64 keys, 8 KiB per scalar.

Tests (R04): one test per `ParseError`; BOM and CRLF accepted and recorded; body with attachment syntax byte-identical; `tags: []` in both forms; a table-driven fuzz test feeding truncations and single-byte mutations of valid fixtures must never panic and never exceed a limit.

### Task 5: Timestamps

Files: `diary-domain/src/source/stamp.rs`, tests.

```rust
pub struct Instant { pub unix_ms: i64, pub precision: Precision } // Precision::{Second, Milli}
pub enum StampError { Format, Offset, Calendar, DirMismatch }
pub fn header_created_at(s: &str) -> Result<Instant, StampError>;                     // RFC 3339, explicit Z or ±hh:mm
pub fn ayumi_stem(stem: &str, dir: (i32, u8, u8)) -> Result<Instant, StampError>;     // YYYY-MM-DD_HH-mm-ss-SSS, UTC
pub fn narihito_stem(stem: &str, dir: (i32, u8, u8)) -> Result<(Instant, String), StampError>; // YYYY-MM-DD_HH-mm-ss-<uuid>
pub fn compare(header: Instant, stem: Instant) -> bool;                                // equal at the coarser precision
```

Rules: four-digit years 1000–9999; `Z` or `±hh:mm` with hh ≤ 23, mm ≤ 59 (`+0900` and a missing offset are `Offset`/`Format`); fractional seconds 1–3 digits, more is `Format`; seconds 0–59 (leap second `:60` is `Calendar`). Own the calendar check (month lengths, leap years); the prior art's manual 1–31 check is not reused. The Narihito stem is `YYYY-MM-DD_HH-mm-ss-<lowercase uuid v4>` (from `DiarySnippet.fileTimestamp`); the directory `YYYY/MM/DD` must equal the stem's UTC date.

Tests (R06): `+09:00` header equals the UTC stem; naive header, `+0900`, 4-digit fraction rejected; `2025-02-29` rejected, `2024-02-29` accepted, `2100-02-29` rejected; second-precision stem agrees with a millisecond header in the same second and disagrees in the next; directory/filename date mismatch is `DirMismatch`; uppercase or non-v4 UUID in a Narihito stem is `Format`.

### Task 6: Adapters and outcomes

Files: `diary-application/src/source.rs`, `diary-infrastructure/src/source/{ayumi.rs,narihito.rs,adapter.rs}`.

```rust
pub struct SourceListing { pub id: OpaqueId, pub rel: String, pub size: u64, pub dating: Dating, pub token: ObservationToken, pub warnings: Vec<Warning> }
pub struct SourceView { pub listing: SourceListing, pub body: String, pub raw_front_matter: String, pub fields: Vec<(String, Value)>, pub newline: Newline, pub bom: bool }
pub enum SourceOutcome { Ok(SourceListing), Unavailable(ReadError), Malformed(ParseError), Unsupported { reason: UnsupportedReason, viewable: bool }, Conflict(ConflictKind) }
pub enum Warning { DuplicateProducerId }
pub struct ObservationToken { /* id tuple, sha256, dev, ino, size, config_generation */ }
pub trait SourceAdapter { fn interpret(&self, rel: &SourcePath, obs: &Observation, generation: u64) -> SourceOutcome; }
pub fn read_view(root: &ResolvedRoot, kind: SourceKind, rel: &SourcePath, generation: u64) -> Result<SourceView, ViewError>; // ViewError::{Outcome(SourceOutcome), Stale}
```

Ayumi: layout `YYYY/MM/DD/YYYY-MM-DD_HH-mm-ss-SSS.md`; `id` equals the stem; no schema marker, so `Schema`/`Kind` apply only to Narihito. Narihito: `YYYY/MM/DD/<stem>.md`, `schema_version` 1, `kind` `ndm.diary-snippet`, lowercase UUID v4 `id` matching the stem's UUID. `Unsupported` and `Conflict` outcomes keep the body retrievable through `read_view`. Both validate header and filename independently into `Dating`. A missing sibling `.assets` never changes the outcome. Duplicate producer IDs are detected across the scan (Task 7), warn on every involved listing and never merge paths. `Debug` for `SourceView`, `Observation` and `ObservationToken` is implemented by hand and prints lengths and short digest prefixes only.

Tests (R05, R06): valid fixture per kind; unknown schema and unknown kind give `Unsupported` with `viewable`; layout mismatch is `Unsupported(Layout)`; `id` ≠ stem is `Conflict(IdMismatch)`; missing `.assets` still `Ok`; `read_view` after a byte change is `Stale`; the outcome-order table is covered by one test per row.

### Task 7: Scanner and isolation

Files: `diary-infrastructure/src/source/scan.rs`, tests.

```rust
pub struct ScanReport { pub listings: Vec<SourceListing>, pub failures: Vec<(String, SourceOutcome)>, pub collisions: Vec<Vec<String>>, pub incomplete: bool }
pub fn scan(root: &ResolvedRoot, kind: SourceKind, generation: u64) -> ScanReport;
```

- Walk with `openat` descriptors, no symlink following, never crossing a mount point (`st_dev` change). In each directory list the names, run `group_siblings`, record each collision group in `collisions` and skip **every** member (file or directory; the architecture says neither is offered). Sort the remaining names by key before descending, so truncation yields a deterministic prefix.
- Candidates are `.md` regular files. Ignore dot-files (`.DS_Store`, AppleDouble `._x.md`) except `.<name>.md.icloud` placeholders, which become `Unavailable(NotDownloaded)` for the real relative name (legacy iCloud stub shape, unverified on modern File Provider). Do not descend into `*.assets` directories. Depth limit 8.
- Stop after 10,000 candidates, and also after visiting 100,000 directory entries, setting `incomplete: true`. The walk runs on the calling thread and is not cancellable in M9; the P1-M10 import job owns cancellation and the deadline.
- Read each candidate with `read_fresh`, interpret, drop the bytes. Then apply the duplicate-producer-ID warning across listings.
- `listings` and `failures` are sorted by key.

Tests (R02, R07, R08): mixed directory (valid, malformed, dataless via the `between` seam, unsupported, conflict) returns every valid listing and one outcome per failure; 10,001 candidates set `incomplete` and the first 10,000 by key are returned; a symlinked subdirectory and a file symlink are not followed; collisions use the pure `group_siblings` seam (APFS on macOS cannot hold NFC/NFD twins, and a case twin only on a case-sensitive volume, so a real-filesystem collision test runs only when the volume allows it); `Debug` of `ScanReport` contains no absolute root, body or front-matter value, with seeded marker strings.

### Task 8: Docs

No new server log codes in M9: nothing calls the scanner yet, and the logging allowlist must change with its first caller (P1-M10). Record this as a departure. Update the docs listed in R09, tick the checklist with evidence, run `scripts/gate.sh`, and record departures below.

## Acceptance matrix

| ID  | Tests                                              |
| --- | -------------------------------------------------- |
| R01 | Tasks 1, 2 identity tests                          |
| R02 | Task 2 root tests, Task 7 collision and link tests |
| R03 | Task 3 reader tests                                |
| R04 | Task 4 one test per `ParseError`, fuzz test        |
| R05 | Task 6 adapter and outcome-order tests             |
| R06 | Tasks 5, 6 timestamp and dating tests              |
| R07 | Task 7 mixed-directory, bound and order tests      |
| R08 | Task 7 redaction test, Task 6 `Debug` impls        |
| R09 | Task 8 doc review                                  |

## Review focus

- File replaced by a symlink to outside the root between listing and reading: refused, no bytes returned.
- Same key from two directory entries: `path_collision`, neither listed (pure-function test; not reproducible on APFS).
- Valid-looking front matter with a YAML alias or `!!tag`: `malformed`, never parsed.
- Header says `+09:00` but the filename stem is a different UTC second: viewable `Dating::Disagree`, no date assigned.
- File grows while being read: `source_changing`, not a truncated body.
- A FIFO or dataless placeholder is never opened for reading.
- Config reload with unchanged roots: `source_id` unchanged, token generation changed.

## Departures made during implementation

- `ReadError` is the application-owned `Unavailable` type, re-exported by infrastructure, preserving inward dependencies. `SourceOutcome::Ok` boxes its listing to bound enum size. Public IDs are validated opaque hex strings; no domain `OpaqueId` type existed.
- Producer adapters implement separate `SourceAdapter` policies in `ayumi.rs` and `narihito.rs`; the shared adapter constructs observations and enforces outcome order. `observe` exposes a token for unsupported/conflicting viewable files. `read_view` explicitly takes the expected token, checking it before parsing changed bytes.
- Root namespace in a token is the complete SHA-256 of the canonical path so no absolute path leaks. The scanner's private identity index compares full canonical-root/path tuples independently of shortened IDs; import-context binding remains P1-M10.
- The reader checks the original descriptor and reopens the complete path from the validated root, which also catches swapped directory ancestors. This replaces the narrower parent-only `fstatat` check.
- Synthetic adapter tests live beside the modules, so private read-race seams remain inaccessible to production callers. No server test hook or new log code is added.
- Non-UTF-8 filenames cannot be created on this test APFS volume (`EPERM`); the `OsStr` conversion boundary is tested in memory. The entry-budget test uses the same walker with a reduced internal limit; the production limit is 100,000. The 10,001-candidate test exercises the production candidate limit.
- Real iCloud materialization/dataless flags and a mounted-volume crossing remain runtime evidence limits. Synthetic flag decisions, device guards and descriptor traversal do not claim provider validation.
