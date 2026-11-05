# P1-M9: Source identity and reading work plan

Status: Planned

Implementation plan for the [P1-M9 overview](milestone-09-source-reading-overview.md). It follows [sources and dates](../architecture/sources-and-dates.md) and the inward dependency rule of [architecture](../architecture.md). Each task ends with passing tests and one commit. Run affected tests while iterating and the full gate once at the end. Fixtures are synthetic; no private samples.

## Placement

| Layer          | Location                                          | Adds                                                                                                |
| -------------- | ------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Domain         | `diary-domain/src/source/` (new, dependency-free) | `SourcePath` (key + spelling), front-matter grammar, `CreatedAt`/`StemStamp` parsing and comparison |
| Application    | `diary-application/src/source.rs`                 | `SourceView`, `SourceOutcome`, `ObservationToken`, `SourceReader` port, `ScanReport`                |
| Infrastructure | `diary-infrastructure/src/source/` (new)          | Root resolver, volume probe, bounded no-follow reader, Ayumi/Narihito adapters, scanner             |
| Server/API     | none                                              | Exposure is deferred (O2)                                                                           |

Domain code stays free of YAML libraries, as for Authentic ([M6](milestone-06-authentic-persistence-architecture.md)): a closed line grammar rejects duplicate keys, aliases, tags and nesting by construction. Nothing here writes to a source.

## Owner decisions needed

Answer before Task 1; record in the [changelog](changelog.md).

- **O1 — NFC dependency.** Path keys need Unicode NFC. No normalization crate is approved. Recommended: add `unicode-normalization` (MIT/Apache-2.0, compatible) to `diary-domain`, pinned `=` like the other dependencies, and regenerate notices. Alternative: normalize through `objc2-foundation` `NSString` (already approved, macOS only, needs an added `NSString` feature, domain would depend on infrastructure, so the call would sit in infrastructure with a port).
- **O2 — Exposure.** Recommended: no HTTP/bridge surface in P1-M9, because `list_sources` takes an `import_id` that P1-M10 introduces. Adapters are proved by library tests and a gated `diary-server` test hook. Alternative: add a temporary `GET /sources` returning outcomes, which P1-M10 would then reshape.

## Tasks

### Task 1: Path identity

Files: `diary-domain/src/source/{mod.rs,path.rs}`, tests inline in `tests.rs`.

```rust
pub struct SourcePath { pub key: String, pub spelling: String } // key: NFC (+ fold if insensitive); spelling: actual fs name
pub fn normalize(rel: &str, insensitive: bool) -> Result<SourcePath, PathError>;
pub enum PathError { Empty, Absolute, Dot, DotDot, NonUtf8, ControlChar }
```

Tests (R01): NFD input equals NFC key; `A.md` vs `a.md` equal only when `insensitive`; `x//y`, `./x`, `../x`, `/x` rejected; invalid UTF-8 `OsStr` rejected at the caller (infrastructure) with `NonUtf8`.

### Task 2: Root resolver and collisions

Files: `diary-infrastructure/src/source/{mod.rs,root.rs}`, tests.

```rust
pub struct ResolvedRoot { pub canonical: PathBuf, pub insensitive: bool, pub device: u64 }
pub fn resolve_roots(sources: &[Source]) -> Result<Vec<ResolvedRoot>, RootError>; // dedup/nested/alias rejection
pub fn source_id(root: &ResolvedRoot, path: &SourcePath, generation: u64) -> OpaqueId; // hash; stored tuple compared on lookup
```

Probe case behaviour by creating and statting a differently-cased temp name inside a scratch directory it owns, never in the source root. Tests (R01, R02): symlink alias of a root equals the root; nested and duplicate roots rejected; renamed alias keeps `source_id` input stable; changed generation changes the ID; a forced hash collision is detected by the tuple check.

### Task 3: Bounded fresh reader

Files: `diary-infrastructure/src/source/read.rs`, tests.

```rust
pub struct Observation { pub bytes: Vec<u8>, pub sha256: [u8; 32], pub size: u64, pub dev: u64, pub ino: u64, pub mtime_ns: i128 }
pub fn read_fresh(root: &ResolvedRoot, rel: &SourcePath) -> Result<Observation, ReadError>;
pub enum ReadError { Missing, Denied, NotDownloaded, NotRegular, TooLarge, SourceChanging, Io }
```

Open each component with `O_NOFOLLOW` relative to the root descriptor (reuse `diary_logging::private_fs` patterns, but without its 0700/0600 checks, which are for private data); `fstat` before and after; compare; retry once. Detect an iCloud placeholder by the macOS `SF_DATALESS` flag, mapping to `NotDownloaded`. Tests (R03): symlink file and symlinked directory refused; FIFO refused; 8 MiB + 1 refused before allocation; a test hook that mutates the file between stats gives `SourceChanging` after one retry; digest equals an independent `sha256` of the bytes. Limit: dataless detection is unverified against real iCloud (handoff evidence gap).

### Task 4: Front-matter grammar and encoding

Files: `diary-domain/src/source/{front_matter.rs,text.rs}`, tests.

```rust
pub struct Parsed { pub raw_front_matter: String, pub fields: Vec<(String, String)>, pub body: String, pub newline: Newline, pub bom: bool }
pub fn parse(bytes: &[u8]) -> Result<Parsed, ParseError>;
pub enum ParseError { Utf8, StrayCr, MixedNewlines, Unclosed, DuplicateKey, Alias, Tag, TooDeep, FrontMatterTooLarge, TooManyKeys, ScalarTooLarge, BodyTooLarge }
```

Limits: 64 KiB, 64 keys, 8 KiB scalar, 1 MiB body. Accept scalars the research documents (plain, quoted, empty list as `[]` or two-line); anything else is `Alias`/`Tag`/`TooDeep`, never silently coerced. Body is returned byte-for-byte after the closing delimiter (no rewriting). Tests (R04): one test per `ParseError`; BOM and CRLF accepted and recorded; body with attachment syntax unchanged.

### Task 5: Timestamps

Files: `diary-domain/src/source/stamp.rs`, tests.

```rust
pub struct Instant { pub unix_ms: i64, pub precision: Precision } // Precision::{Second, Milli}
pub fn header_created_at(s: &str) -> Result<Instant, StampError>;          // explicit offset required
pub fn stem_stamp(stem: &str, dir_date: (i32, u8, u8)) -> Result<Instant, StampError>; // UTC; dir date must match
pub fn compare(header: Instant, stem: Instant) -> Agreement;                // compares at the coarser precision
```

Own the calendar check (month lengths, leap years); do not use a manual 1–31 check as the prior art does. Tests (R06): `+09:00` header equals the UTC stem; naive header rejected; `2025-02-29` rejected, `2024-02-29` accepted; second-precision stem agrees with a millisecond header in the same second and disagrees in the next; directory/filename date mismatch is `StampError`.

### Task 6: Adapters and outcomes

Files: `diary-application/src/source.rs`, `diary-infrastructure/src/source/{ayumi.rs,narihito.rs,adapter.rs}`, fixtures under `src-tauri/crates/diary-infrastructure/tests/fixtures/sources/`.

```rust
pub struct SourceView { pub id: OpaqueId, pub rel: String, pub body: String, pub raw_front_matter: String, pub fields: Vec<(String,String)>, pub dating: Dating, pub token: ObservationToken }
pub enum Dating { Resolved(Instant), Conflict { header: Instant, filename: Instant }, Undated(UndatedReason) }
pub enum SourceOutcome { Ok(SourceView), Unavailable(ReadError), Malformed(ParseError), Unsupported(UnsupportedReason), Conflict(ConflictKind) }
pub trait SourceAdapter { fn interpret(&self, rel: &SourcePath, obs: &Observation) -> SourceOutcome; }
```

Ayumi: layout `YYYY/MM/DD/YYYY-MM-DD_HH-mm-ss-SSS.md`, ID equals the stem. Narihito: `YYYY/MM/DD/<UTC timestamp>-<uuid>.md`, schema version 1, kind `ndm.diary-snippet`, lowercase UUID v4 matching the stem. Unknown schema/kind is `Unsupported` with body still returned in a separate `viewable_body` field of that variant. Header and filename are validated independently; disagreement is `Dating::Conflict` inside an `Ok` view (viewable, excluded from automatic assignment). Producer IDs duplicating across files warn, never dedupe. Tests (R05, R06): valid fixture per type; unknown schema `Unsupported` with body; missing sibling `.assets` still `Ok`; duplicate producer ID produces a warning on both and two views.

### Task 7: Scanner and isolation

Files: `diary-infrastructure/src/source/scan.rs`, tests.

```rust
pub struct ScanReport { pub views: Vec<SourceView>, pub failures: Vec<(String, SourceOutcome)>, pub collisions: Vec<Vec<String>>, pub incomplete: bool }
pub fn scan(root: &ResolvedRoot, kind: SourceKind, generation: u64) -> ScanReport;
```

Walk with no symlink following, `.md` only, count candidates and stop at 10,000 with `incomplete: true`; sort by relative key for determinism. Tests (R07, R08): mixed directory (valid, malformed, dataless via hook, unsupported, conflict) returns every valid file; 10,001 candidates sets `incomplete`; debug-formatted report contains no absolute root path, body or front matter value.

### Task 8: Logging and docs

Add client-neutral server log codes `source.scanned` (counts, `incomplete`) and `source.read_failed` (code, relative key hash) if the logging contract allows; otherwise record in the departures list that none were added. Update the docs listed in R09 and tick the checklist with evidence. Run `scripts/gate.sh`; record departures below.

## Acceptance matrix

| ID  | Tests                                    |
| --- | ---------------------------------------- |
| R01 | Tasks 1, 2 identity tests                |
| R02 | Task 2 root tests, Task 7 collision test |
| R03 | Task 3 reader tests                      |
| R04 | Task 4 one test per `ParseError`         |
| R05 | Task 6 adapter fixtures                  |
| R06 | Tasks 5, 6 timestamp and dating tests    |
| R07 | Task 7 mixed-directory and limit tests   |
| R08 | Task 7 redaction test, Task 8 log review |
| R09 | Task 8 doc review                        |

## Review focus

- File replaced by a symlink to outside the root between listing and reading: refused, no bytes returned.
- Same name in NFC and NFD on one directory: `path_collision`, neither listed.
- Valid-looking front matter with a YAML alias or `!!tag`: `malformed`, never parsed.
- Header says `+09:00` but filename stem is a different UTC second: viewable `Dating::Conflict`, no date assigned.
- File grows while being read: `source_changing`, not a truncated body.

## Departures made during implementation

None yet. This list overrides the tasks above.
