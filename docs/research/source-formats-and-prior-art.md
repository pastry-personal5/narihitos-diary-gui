# Source formats and prior-art findings

Status: Active

Research date: 2026-11-05. Covers P1-M1 R1.1–R1.6 and R2.1–R2.4; also informs R3, R6, R8, and source write-back questions. These are observed contracts and recommendations, not an approved GUI schema or permission to modify sources.

## Evidence and limits

Inspected reference revisions: `contrib/narihitos-diary` at `8d4563851ee5f7f7d96b241138f526df6c83d1c9` and `contrib/narihitos-diary-mobile` at `71e564f8f5e4a23483d3a91b4bdf4d5cfb663793`. Both worktrees were clean. Source paths below are relative to those revisions; neither reference becomes a dependency.

A read-only Python inventory inspected every local sample Markdown file, split front matter at delimiters, counted field names/schema markers, parsed `created_at` with `datetime.fromisoformat`, and compared UTC dates/stems with directories/filenames. It emitted counts only, with no diary bodies, IDs, paths, locations, or timestamps from the samples. This inventories the supplied examples; it is not a production YAML parser or proof of all historical source variants.

| Check                                                  | Ayumi               | Narihito mobile                           |
| ------------------------------------------------------ | ------------------- | ----------------------------------------- |
| Markdown samples inspected                             | 2                   | 20                                        |
| `created_at` offset                                    | Both `+09:00`       | All `Z`                                   |
| Parent directory matches UTC creation date             | 2/2                 | 20/20                                     |
| Filename starts with UTC creation timestamp to seconds | 2/2                 | 20/20                                     |
| ID matches expected filename identity                  | 2/2, stem equals ID | 20/20, stem ends with UUID                |
| Matching sibling `.assets` directory exists            | 2/2                 | 20/20                                     |
| Duplicate IDs within sample set                        | 0                   | 0                                         |
| Schema markers                                         | None                | All version `1`, kind `ndm.diary-snippet` |

Audio bytes were not opened. No live iCloud operation, source mutation, build, or prior-art test suite was run. Test source is evidence of intended coverage, not a passing test claim.

## R1.1 Ayumi

Observed layout is `YYYY/MM/DD/YYYY-MM-DD_HH-mm-ss-SSS.md` with a sibling `<stem>.assets/`. Header keys present in both examples are `id`, `created_at`, `updated_at`, `tags`, `favorite`, weather summary/temperature, location latitude/longitude/place name, and transcription model/prompt. The ID is the timestamp stem; it is not the mobile format's UUID. Directory/stem timestamps are UTC even when the header serializes a local offset.

The mobile reference's [LegacyDiaryReader.swift](../../contrib/narihitos-diary-mobile/NarihitosDiary/Adapters/LegacyDiaryReader.swift) independently documents and checks the legacy timestamp grammar, ID/stem equality, and creation-instant agreement. It recognizes empty tags in both inline and two-line forms, and has a deliberately restricted scalar grammar. This is useful evidence of compatibility expectations, not a complete specification of Ayumi's exporter: its producer source is not present here, and nonempty tag variants are not established by two samples.

The desktop [ayumi.rs](../../contrib/narihitos-diary/crates/diary-connectors/src/ayumi.rs) handles BOM/CRLF normalization, rejects stray carriage returns, bounds input at 8 MiB, strips front matter and recognized audio attachment lines, and checks path containment and read freshness. It is not a general YAML schema adapter. In particular, its source-writing/deletion behavior must not be inherited merely by reusing read logic.

## R1.2 Narihito mobile

The schema owner in the inspected revision is the mobile producer's [DiarySnippet.swift](../../contrib/narihitos-diary-mobile/NarihitosDiary/Domain/DiarySnippet.swift), [DiarySnippetMarkdownCodec.swift](../../contrib/narihitos-diary-mobile/NarihitosDiary/Adapters/DiarySnippetMarkdownCodec.swift), and [DiarySnippetLayout.swift](../../contrib/narihitos-diary-mobile/NarihitosDiary/Adapters/DiarySnippetLayout.swift).

- Identity is a canonical lowercase UUID v4. `created_at`/`updated_at` use whole-second UTC `YYYY-MM-DDTHH:mm:ssZ` in the producer schema.
- Layout is `YYYY/MM/DD/<UTC timestamp>-<uuid>.md`, sibling `<same stem>.assets/`, with `<uuid>.m4a` as the audio filename. The producer rejects audio references containing traversal/path/URI-like characters.
- Required fields cover schema/kind, identity/timestamps, title/title origin, favorite/language, audio properties, and transcription method/language hint/settings revision. Audio hash, transcription model/runtime/conversion provenance, title provenance, and location/weather context have conditional or optional rules. All examples contain title and audio/transcription metadata; absence of location/weather in these 20 samples does not mean the schema lacks them.
- The codec distinguishes supported, unsupported, and malformed data. It preserves unknown bounded scalars and uninterpreted context fields. Current producer policy accepts only its recognized transcription methods/title origins; a GUI importing text should compare that strict producer policy against a more forward-compatible text-import policy.
- Producer limits are 64 KiB front matter, 64 keys, 8 KiB per scalar, and 1 MiB body. These are reference constraints, not automatically the GUI's chosen limits. The GUI must bound the whole read before parsing.

The [codec tests](../../contrib/narihitos-diary-mobile/NarihitosDiaryTests/DiarySnippetMarkdownCodecTests.swift) and [domain tests](../../contrib/narihitos-diary-mobile/NarihitosDiaryTests/DiarySnippetDomainTests.swift) are candidates to translate into synthetic Rust fixtures. The Swift implementation should be rewritten against the eventual Rust parser rather than translated line-for-line with all producer-specific validation.

## R1.3–R1.6 Import implications and alternatives

Recommended separation: retain original Markdown bytes as provenance, extract a normalized snippet view, and build an explicitly filtered agent packet. Preserving provenance does not require displaying every field or sending metadata to a provider. Audio is outside v1 import scope; do not require the audio file to exist to accept otherwise valid text. Whether to strip attachment-only lines from the editor view or keep broken links visible is a product question; retain the original text either way.

Compare identity strategies:

| Strategy                                                      | Advantage                         | Problem                                                                        |
| ------------------------------------------------------------- | --------------------------------- | ------------------------------------------------------------------------------ |
| Source path only                                              | Simple catalog key                | Rename looks like a new snippet                                                |
| Content digest only                                           | Detects identical bytes           | An edit looks like a new asset; unrelated identical notes collapse             |
| Stable source identifier + source-native ID, plus byte digest | Separates identity from revisions | Requires rules for source rename/reconfiguration and conflicting duplicate IDs |

Recommend the third for P1-M2 evaluation. A digest records a revision, not identity. Neither the source display name nor absolute folder path is necessarily a stable namespace. Do not silently collapse an ID shared across two sources; preserve provenance and report same-namespace ID conflicts. Repeated same-ID/same-byte imports can be no-ops while preserving selection state; same-ID/changed-byte imports need an explicit conflict/refresh policy, especially if the imported copy has edits.

Use `created_at` as the candidate authoritative instant after schema validation, and compare it with path/stem evidence. Filename fallback for damaged metadata is an alternative, but must be visibly uncertain rather than silently overriding a contradictory header. No sample exercises disagreement, so owner confirmation remains necessary. See [date findings](date-and-time.md) for the UTC scan/filter algorithm.

Proposed synthetic fixture matrix for implementation: valid examples of each source with invented bodies/IDs; CRLF/BOM vs. strict LF; duplicate keys; missing/unclosed delimiters; malformed UTF-8; oversized input; impossible/leap dates; offset and filename disagreement; unknown schema/kind; unknown scalars; missing audio; traversal-like attachment; unchanged re-import; changed bytes under one ID; duplicate IDs within/across sources; source rename; symlink escape; source changed while being read. Expected outcomes must distinguish unavailable, malformed, unsupported, conflicting, and successfully imported records. Preserve valid neighbors when one file fails. No fixture or parser implementation is introduced by this research.

## R2.1–R2.2 Reuse map

The desktop prior art is already Rust (edition 2024), with a Swift Apple helper, rather than a codebase requiring wholesale porting to Rust. Its workspace contains `diary-core` (domain/workflow), `diary-connectors` (Ayumi, SQLite, local output, Notion, Apple bridge), `diary-control`/`diary-control-contract` (HTTP/control jobs and DTOs), `diary-macos` (native helpers), `diary-tui`, and `apps/nd` (assembly).

| Reference                                                                      | Reusable knowledge                                                                                            | GUI adaptation needed                                                                         |
| ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `diary-connectors/src/local_output.rs`                                         | `YYYY/MM/YYYY-MM-DD.md`, no-clobber writes, digest/read-back, placeholder guards, receipt-guarded replacement | Add draft/Entry metadata and history contract; keep optional export independent of local save |
| `diary-connectors/src/notion/{projection,hierarchy,output,transport,retry}.rs` | Markdown projection, page discovery, insertion anchors, chunking, reconciliation                              | Verify API behavior/version; choose GUI export UX and stable receipts                         |
| `diary-connectors/src/ayumi.rs`                                                | Bounded/fresh reads and containment checks                                                                    | Add mobile format and explicit source namespaces; separate import from edit/delete            |
| `diary-core/src/source_time.rs`                                                | UTC stem interpretation and boundary cases                                                                    | Replace fixed-offset/manual calendar handling with timezone-aware validation                  |
| `diary-control/src/lifecycle.rs`                                               | Config validation, secret redaction, explicit executable paths                                                | Replace launch-directory/server assumptions; no need to transplant HTTP/TUI layers            |
| Mobile codec/layout/domain and pair-store code                                 | Exact producer grammar, schema evolution, coordinated writes/recovery concepts                                | Read Markdown independently of audio; investigate write-back interoperability separately      |

## R2.3 Bugs, limitations, and contradictory assumptions

- **Notion ordering is already addressed in the inspected prior-art revision.** [hierarchy.rs](../../contrib/narihitos-diary/crates/diary-connectors/src/notion/hierarchy.rs) contains `plan_insertion`; [tests.rs](../../contrib/narihitos-diary/crates/diary-connectors/src/notion/tests.rs) includes first/middle/last insertion, >100-block chunks, anchor drift, and lost-reply adoption tests. The [design](../../contrib/narihitos-diary/docs/superpowers/specs/2026-10-30-notion-ascending-order-design.md) explicitly leaves existing disorder untouched. Therefore, "fix sort order" becomes verify/reuse ascending insertion and decide whether historical disorder needs repair; do not promise a full re-sort from this implementation.
- The desktop filename-time parser checks day 1–31 without month-specific validity and discards fractional seconds. It expects a numeric suffix; the mobile UUID suffix will not parse. Its fixed-offset conversion cannot itself supply historical DST rules. Do not reuse it as a universal snippet parser.
- Automatic Ayumi discovery scans neighboring UTC directories around the selected date. That is not proof that it covers the GUI's full **three local dates** under every offset. Compute bounds explicitly (R8).
- The old config's `server.port_fallback_count` is used by its control server; its YAML parser is hand-written and tied to the old schema. Neither the server nor parser is required by the GUI ideas.
- The source adapter includes body overwrite and audio-linked deletion, and the old mirror can publish automatically. The GUI owner now requires explicit source edit/save and confirmed snippet-plus-related-audio deletion. This supports reusing safety concepts, but does not inherit the old deletion algorithm or automatic mirror behavior.
- The owner's general description "buggy workflow" is not a reproducible defect list. These are source-inspected risks and mismatches, not a claim that the whole prior-art workflow has been diagnosed.

## R2.4 Licensing

Both reference repositories have Apache-2.0 root licenses; the desktop workspace also declares `license = "Apache-2.0"`. This supports evaluating first-party source reuse within this Apache-2.0 project. Keep required attribution/license notices and document modifications when copying; a rewrite does not justify copying unrelated vendor assets without their terms. This is an inventory, not blanket clearance of every transitive dependency.

The mobile repository separately includes TranscribeCpp/ggml/miniz and model licenses/NOTICE files. None is needed for Markdown-only import. Do not pull its vendored frameworks/models into the GUI. R10 must review the actual selected dependency/assets set before adoption.

## P1-M2 implications and unresolved decisions

R1 implies separate source adapters with normalized identity/instant/body plus preserved provenance and per-file outcomes; choose the duplicate/revision/metadata policy before defining import storage. R2 implies selective reuse of Rust algorithms, not importing the old client-server/TUI architecture. The owner requires explicit source edit/save and confirmed deletion of the snippet plus related audio. Freshness guards, metadata updates, related-file ownership, and interaction with mobile pair storage remain unresolved implementation contracts. Ayumi producer coverage beyond the supplied examples is an evidence gap; expanding compatibility may require more synthetic contracts or producer documentation.
