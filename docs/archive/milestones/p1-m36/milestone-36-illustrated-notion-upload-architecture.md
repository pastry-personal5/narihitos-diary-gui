# P1-M36: Illustrated diary Notion upload work plan

Status: Archived

Implement the [overview](milestone-36-illustrated-notion-upload-overview.md) through the existing export coordination boundaries, with a separate Illustrated projection and day-page layout. The product interview is complete; owner decisions are binding and proposed internal details require code review before affected implementation.

## Code review findings (2026-11-13)

| Existing location                                                                   | Finding and consequence                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ----------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `diary-application/src/export.rs`                                                   | `Snapshot`, `StartRequest.expected` and `Receipt` assume an Authentic revision; `Target` denotes destination only, and `ExportAdapter::apply` takes one byte slice. Add an explicit content-kind axis instead of treating Illustrated as a third provider. Preserve old Authentic callers and persisted rows.                                                                                                                             |
| `diary-infrastructure/src/export.rs`                                                | The service finds an adapter by `Target` alone, so two Notion adapters would shadow each other; look up by kind and target. The destination key already hashes target, root and workspace, so distinct roots give distinct receipts, reservations and `notion_steps` rows without a new key scheme. This is why equal roots must be rejected.                                                                                             |
| `diary-infrastructure/src/export/{snapshot,store,start,run,recovery}.rs`            | Snapshot loading only reads Authentic; durable intent stores one Markdown blob capped at `MAX_SNAPSHOT_BYTES`, with Authentic revision columns. An Illustrated upload needs its own bounded durable snapshot and kind-aware recovery, independent of current local files.                                                                                                                                                                 |
| `diary-infrastructure/src/sqlite/0006_export.sql`, `0007_notion_steps.sql`          | Tables are `STRICT` with `CHECK` constraints: `export_receipts` is keyed `(destination_key, authentic_id)` and `notion_steps.kind` allows only `create_page`, `create_section`, `append`, `delete`. New step kinds or columns need migration `0008`, and a changed `CHECK` needs a table rebuild that carries existing rows over.                                                                                                         |
| `diary-infrastructure/src/illustrated.rs`, `diary-application/src/illustrated.rs`   | `read` and `photo(date, version, n)` each take the store gate and `photo` rejects a version mismatch with `Stale`. Every regeneration has a new ID, so a sequence of version-bound reads cannot mix two versions; no new multi-photo capture port is needed. Photos read up to 32 MiB each and an entry has up to 99.                                                                                                                     |
| `diary-domain/src/illustrated.rs`                                                   | Validated Markdown references only generated date-relative PNGs, exactly once per photo. Parse this existing format; upload body/image order without exporting front matter or resolving arbitrary paths/URLs.                                                                                                                                                                                                                            |
| `diary-infrastructure/src/config{,/reload}.rs`                                      | Notion requires one `root_page_id` under `deny_unknown_fields`, so the new shape fails today with the generic schema error. Reload fingerprints one root/token pair as a guarded field. Replace with typed pages and fingerprint both roots.                                                                                                                                                                                              |
| `diary-server/src/main.rs`                                                          | `connect_notion` builds one client and adapter at startup; reload never rebuilds adapters and the GUI already tells the user to restart. Build one shared client and up to two adapters here; keep restart-to-apply.                                                                                                                                                                                                                      |
| `diary-infrastructure/src/export/notion/{adapter,client,steps,wire}.rs`             | Current adapter owns year/month child pages, created without a position, and daily heading ranges, reordered by copying blocks. That reorder cannot apply to day pages without changing page IDs. Reuse HTTP/rate/identity/checkpoint mechanics, but keep Illustrated day-page policy separate; do not route its images through the text-only normal projection. The remote fingerprint covers child identity, type, text and order only. |
| `diary-infrastructure/src/export/notion/http.rs`, `diary-server/src/notion_http.rs` | `Request.body` is always sent as `application/json`; the host is fixed with no redirects, a 30-second request timeout and API version `2026-03-11`. Add a bounded multipart body variant for JPEG sending while keeping credentials server-side and avoiding arbitrary upload hosts.                                                                                                                                                      |
| `diary-infrastructure/src/image.rs`                                                 | Existing macOS ImageIO/CoreGraphics FFI validates PNG decoding. A narrow system-framework JPEG encoder/resizer can reuse this platform approach without adding a dependency; qualify its ownership, color/alpha behavior and bounds.                                                                                                                                                                                                      |
| `src/application/export/{controller,contracts}.ts`                                  | The controller flushes and reloads Authentic, requires it open and calls the destination a Notion section; DTOs carry an Authentic `expected_revision` and `revision`. Add explicit kind-aware preparation, an Illustrated version field and wording; an Illustrated upload must not flush unrelated buffers or require an open Authentic tab.                                                                                            |

Paths above are relative to `src-tauri/crates/` unless they begin with `src/`.

## Stage A — identity, configuration and snapshots

1. Before broader implementation (overview Q01), extend `scripts/smoke/notion-live.py` to qualify two behaviors on the authorized dedicated test root using privately configured credentials. First, chronological page insertion: the current create-page schema supports `position` with `page_start`, `page_end` or `after_block`; the historical M27 live run rejected page position. Second, image read-back: upload one fixture JPEG, attach it, then record which read fields (block ID, `last_edited_time`, caption, file URL path) change after the image is replaced, re-captioned or deleted in Notion by hand. Establish actual behavior for API version `2026-03-11`. If either is unsupported, stop dependent work and return with a concrete proposal that preserves the owner's stable-URL and drift-protection requirements. Keep implementation out of this planning change.
2. Introduce an application content-kind contract for Authentic and Illustrated, preserving existing requests as Authentic where compatibility requires it. Provider remains `notion` or `icloud`; reject Illustrated+iCloud server-side. Map config `normal` to Authentic explicitly.
3. Parse the requested `notion.pages` format strictly. Reject `notion.root_page_id` with a safe migration message and document the new example. Do not rewrite the private config. Keep `schema_version: 1` and add one typed config error for the legacy field so the message can name `notion.pages`; preserve unrelated schema behavior.
4. Look up adapters by kind and target, and carry the kind in operation rows and results. Leave the destination-key inputs unchanged: distinct roots already separate receipts, reservations and step rows, and changing the hash would orphan every existing Authentic receipt. Add migration `0008` following the numbered-file convention; it must interpret historical operations as Authentic, rebuild any table whose `CHECK` changes with its rows intact, and cannot discard pending work.
5. Bind Illustrated receipt ownership to the stable date slot (the ISO date fits the existing `authentic_id` receipt column), with the version ID and content manifest as revision evidence. Regeneration creates a new Illustrated ID, so using it alone as the receipt key would incorrectly treat every regenerated day as unowned.
6. Capture a published version with the existing version-bound `read` and `photo` calls, checking the caller's expected version on each; any `Stale` or missing read aborts the whole capture. Read and convert one photo at a time, so at most one 32 MiB PNG is in memory, and persist only the body, manifest and prepared JPEGs. Do not copy source PNGs: 99 photos at the read limit would be about 3 GiB, while the 480-pixel JPEGs are small. Prefer private operation-owned files over SQLite blobs; journal staging/publication and cleanup so restart needs no current local entry. Fix a per-JPEG and a total byte cap in review. Missing/corrupt photos fail preparation without partial capture.
7. Freeze date, root identity, source version, photo hashes, projection version and conversion settings. Config reload must not redirect admitted or paused work; changed destinations require a new operation and cannot borrow old receipts or confirmations.

## Stage B — JPEG preparation and Notion projection

1. Convert each captured PNG to a temporary JPEG with output height exactly 480, width `max(1, round(source_width * 480 / source_height))`, quality 0.85, and white alpha flattening. This includes proportional upscaling when input height is below 480. Define color-space/orientation handling and an overflow-safe output-pixel bound; reject invalid or excessive dimensions before allocating. Do not call external image tools or add packages without approval.
2. Finish and validate every JPEG before remote mutation. Record output dimensions/length/hash and a converter version in the manifest. Keep local originals unchanged; JPEG nondeterminism must not cause repeated uploads of an unchanged published version. No-op decisions compare stable semantic manifests; restart uses retained prepared bytes.
3. Render Markdown text with the existing supported formatting/Unicode rules and counted fallbacks. Project each validated generated photo reference to an image placeholder, keeping its inline position and meaningful alternative text as caption where the API supports it. Do not silently upload Markdown image syntax as text or omit an image. Front matter and metadata never enter the remote projection.
4. Use native Notion file uploads for prepared JPEGs. The small-file protocol creates an upload, sends multipart bytes and attaches the returned upload ID in an image block. Preflight the resized output against current API/workspace limits; if still oversized, fail clearly before remote mutation. Multipart here means HTTP form-data for a single file, not Notion's multi-part large-file protocol. No large-file protocol is needed for this scope unless output qualification proves otherwise and the plan is revised.
5. Extend the existing transport with a typed bounded multipart body and fixed endpoint path built from a validated upload ID. A minimal encoder can use existing byte-body support without enabling reqwest features. If implementation needs a new feature/dependency, ask first under repository policy. Share one credential-scoped `Client` across both Notion adapters, rather than doubling the token's rate budget.
6. Budget the worst case in review: 99 photos need about 200 upload requests plus hierarchy reads and appends of at most 100 blocks each, over a minute at the paced rate. Every upload must be attached within one hour of its creation; a paused or recovered operation whose uploads expired must upload again from retained JPEGs as newly admitted work, never reuse an expired ID. Each JPEG must also fit the 30-second request timeout.

## Stage C — hierarchy, stable day pages and recovery

1. Resolve unique year/month/day child pages with decided titles, paginated reads and recorded IDs. Create missing pages at their ascending chronological position using the qualified create-page position parameter: before all dated siblings at page start, or after the preceding dated sibling's block ID. Apply at all three levels; leave unrelated blocks untouched. Duplicate years/months/days are structural conflicts; serialization prevents duplicate creation within the server. Resolve out-of-process creation races by observation, never guessed ownership. Verify ordering after writes; externally disordered existing pages require a typed conflict unless a separate safe reorder mechanism is qualified. Do not clone/archive pages to sort them or claim an unsupported page-move primitive.
2. Observe the day page and complete normalized child tree. Keep page ID/URL stable. Use the existing decision table: no receipt plus absent creates; unchanged local/remote is a no-op; local changes with unchanged owned remote replace automatically; external drift, missing owned destination or existing unowned content require confirmation. Foreign child pages/databases or unsupported blocks block destructive cleanup.
3. Prepare/upload all photos before deleting old content. Persist each intended create/send/attach request and response identifiers. Insert replacement blocks, verify the complete new projection, then delete only old block IDs covered by the accepted observation. Keep the old body until the new body is verified, acknowledging that temporary duplicate bodies and partial cleanup are possible inside the same day page.
4. Retain operation-owned image block IDs and upload provenance separately from semantic content digests. API image reads return hosted-file data rather than the original `file_upload` input; signed URLs expire and cannot be digest inputs. Verify returned block identity, caption/order and attachment evidence against the checkpoint. Proposed evidence, subject to stage A step 1: the image block IDs returned by this operation's own append responses, each block's `last_edited_time` recorded at verification, and the upload's `uploaded` status from the file-upload read. The existing fingerprint covers identity, type, text and order only, so extend the Illustrated fingerprint with image caption and `last_edited_time`. Never declare success from caption equality alone. If the live check shows an in-place image replacement changes none of these fields, do not enable destructive cleanup; return to the owner.
5. Preserve existing read retry/throttle budgets. Never replay ambiguous page creation, send, append or delete after a lost reply or 5xx. Where a known upload ID permits a status read, reconcile that exact object. A content match without ownership provenance does not authorize deleting remote blocks. Zero matches after a timeout are not proof of non-commit.
6. Recovery reads retained snapshots/checkpoints and observes Notion; it never automatically replays a write. Distinguish verified success, definite failure, external conflict, partial write and uncertain outcome. Cancel stops before the next admitted step and reports any created pages, uploaded files or partial body honestly. Abandoning uncertainty does not claim remote rollback. Block deletion moves blocks to the Notion trash, so removed old content stays restorable by the owner; say so in the partial-cleanup message instead of calling it permanent. Clean local temporary data only when no operation/recovery path needs it.
7. Local Illustrated replacement/deletion after capture does not change the admitted upload. The next upload observes the new version or absence. No local delete propagates a remote delete.

## Stage D — GUI, evidence and maintained docs

- Add **Upload Illustrated Diary to Notion** beside the Illustrated creation/export controls in Main Toolbar Strip, using `ToolbarButton` and a distinct PNG from `scripts/draw-toolbar-icons.py`. Register a palette action and shared guards; no new shortcut is required by this plan.
- Guard on selected date, published Illustrated availability, connected compatible server, writable storage, configured/available Illustrated root and conflicting export reservation. Recheck server-side. Keep upload available from any editor tab; no AI engine/profile readiness requirement.
- Capture date/version once on submission; navigation does not retarget work. Reuse operation status, confirmation, cancel and uncertain-resolution surfaces with kind-specific day-page wording. Global Messages identify date/kind and preparation/upload/photo/verification/cleanup steps; completion never changes focus or auto-opens Notion. Keep export activity out of AI-only activities unless the shared UX contract explicitly accommodates it.
- Add tests at ownership boundaries: config/reload and missing-kind availability; manifest capture vs regeneration/delete; JPEG dimensions/aspect/quality/white alpha and original hashes; no-op without reupload; stable day ID on replace; normal/iCloud nonregression; external drift/foreign blocks; upload expiry and hosted-image reads; limits/throttles; ambiguous writes and crash points; duplicate confirmations and reconnect; native transport/bridge validation and selected-date action behavior.
- Extend isolated fake HTTP/server smoke and packaged native smoke. Test image bodies as JPEG, not PNGs labelled JPEG. The owner authorizes fixture writes under test root `417de2b705cc80239abdd1fc1654154f`; use the existing token privately and verify its access. Validate create/no-op/backdated years, months and days/replace with same page ID/JPEG rendering/restart. Retain created test pages for inspection; do not archive/delete pages. Test-body block replacement is authorized. Never fall back to production roots. Record retained test artifacts and actual API behavior.
- Update the overview checklist with evidence, maintained behavior/config/UX/user docs and M32 release qualification. Run affected tests during implementation and `scripts/gate.sh` at completion. Planning validation is documentation formatting, links and diff review; do not mark the milestone Done on that evidence.

## Revalidated API facts (2026-11-13)

The official [small-file upload guide](https://developers.notion.com/guides/data-apis/uploading-small-files) describes create/send/attach, multipart form-data sending, a 20 MB single-file guide limit, one-hour attachment expiry and temporary hosted download URLs. Upload statuses are `pending`, `uploaded`, `expired` and `failed`, readable by ID at `GET /v1/file_uploads/{id}`. Attached uploads persist; deleting content is not a file-upload revocation mechanism. A free-versus-paid workspace size limit is not stated on these pages. Respect workspace-specific limits as well as the protocol limit.

The [block reference](https://developers.notion.com/reference/block) documents image/file upload attachment objects and hosted-file read representations. A read returns `image.type` `file` with only a one-hour `url` and `expiry_time`; no upload ID or other stable file identity is documented on read. Deleting a block sets `in_trash` and is restorable. Existing JSON-only request handling cannot send JPEG form-data unchanged. Recheck the versioned endpoints and fixture assumptions during implementation; documentation research does not prove live behavior.

The current [create-page schema](https://developers.notion.com/reference/post-page.md) lists a `position` body property with `page_start`, `page_end` and `after_block` shapes, without prose, parent-type limits or caveats. The [move-page reference](https://developers.notion.com/reference/move-page.md) changes parent without a documented position parameter; [update-page](https://developers.notion.com/reference/patch-page.md) does not document one either. Positional creation is the proposed chronological insertion path, subject to the early live check; it is not evidence for arbitrary reordering of existing child pages.

## Qualification execution (2026-11-13)

Application implementation is gated on Q01. The reproducible probe is
[`scripts/smoke/notion-page-qualification.py`](../../../../scripts/smoke/notion-page-qualification.py),
dispatched before server startup by `notion-live.py --qualify-pages`. It uses only
the fixed authorized test root, reads the token privately, refuses redirects,
does not retry writes, and retains all created pages. It never uses the configured
production root. Observation output hashes captions and URL paths and omits signed
URLs, raw responses and config contents. Nine offline safety tests pass via
`python3 -m unittest discover -s scripts/tests -p 'test_notion_qualification.py'`.

Executed with network permission:

```sh
DIARY_NOTION_LIVE=1 python3 scripts/smoke/notion-live.py --qualify-pages --config config/global_config.yaml
DIARY_NOTION_LIVE=1 python3 scripts/smoke/notion-live.py --qualify-pages --config config/global_config.yaml --prepare-images 417de2b7-05cc-81a9-9909-df696928a7c0
```

An initial sandboxed attempt failed on the first read before any write. The
network-enabled run read the authorized root (HTTP 200), created the fixture
container (HTTP 200), and returned these results under API `2026-03-11`:

| Fixture title | Position                            | HTTP | Retained page ID                       |
| ------------- | ----------------------------------- | ---- | -------------------------------------- |
| `2040-02`     | `page_end`                          | 200  | `417de2b7-05cc-8167-9447-f4332e414dda` |
| `2040-01`     | `page_start`                        | 200  | `417de2b7-05cc-8149-b2ab-f3f13dc6cfd7` |
| `2040-04`     | `page_end`                          | 200  | `417de2b7-05cc-813a-9dda-e5bb6adef290` |
| `2040-03`     | `after_block` anchored to `2040-02` | 200  | `417de2b7-05cc-812d-a1ad-c93ed7815c60` |

Paginated read-back matched the expected ascending sequence of the exact returned
IDs. The historical page-position rejection is resolved for these request shapes;
this does not establish application hierarchy/recovery acceptance or insertion at
every hierarchy level. Retained [fixture container](https://www.notion.so/417de2b705cc81a99909df696928a7c0).

A synthetic blue 640 × 480 JPEG was encoded with system ImageIO at quality 0.85,
uploaded natively and attached three times. Create/send/attach all succeeded. Upload
ID: `417de2b7-05cc-810d-820e-00b204ab8408`. This fixture encoder is smoke tooling,
not acceptance of the application's future PNG conversion or transparency handling.

Baseline image read-back at `2026-11-13T10:06:00.000Z`:

| Manual edit requested       | Image block ID                         | Caption SHA-256                                                    |
| --------------------------- | -------------------------------------- | ------------------------------------------------------------------ |
| Replace image, keep caption | `417de2b7-05cc-8172-883a-ff1f4ee41e7d` | `26bdb6f75dd14adfb07fb96ca3c2bd436563e141252deb6542249a37893cbd4f` |
| Change caption only         | `417de2b7-05cc-81ca-97ed-f8bc4ba23ff6` | `0bd0988a88cd4dde8bd354233fbc2159e24c6e9db9aac8f4705ded31a7de900b` |
| Delete image block only     | `417de2b7-05cc-8134-bc41-c92f8e07aec5` | `d26c44f487a20289efa8651bd3293be51cfe87e1092bcec2684d52fe16b4a3b1` |

All three return `image.type = file`, image keys `caption,file,type` and file keys
`expiry_time,url`; no upload ID appears. All share file-path SHA-256
`ab2c5c199ec7cc4cd2e1d7e682f414de6467932979f680963100365c531f808a`.
URL paths are diagnostic evidence only, never proposed semantic digest inputs.
The owner completed the manual edits. The read-only comparison below was run
successfully; the follow-up same-minute counterexample prevents Q01 acceptance.
Read without writes:

```sh
DIARY_NOTION_LIVE=1 python3 scripts/smoke/notion-live.py --qualify-pages --config config/global_config.yaml --observe-images 417de2b7-05cc-81a9-9909-df696928a7c0
```

Do not rerun preparation to observe changes: it creates additional fixtures. No
pages or blocks have been deleted or archived by this qualification tooling.

Validation of this qualification-only change: full `scripts/gate.sh` run
`run.rT4RfY` passed all seven lanes (fmt, clippy, cargo test, prettier, eslint,
vitest and tsc). Logs are in the invocation's temporary `narihito-gate` directory.
The earlier sandboxed run `run.BZg1pP` failed Rust loopback binding with
`PermissionDenied` and one Settings test; the Settings file passed independently
(20 tests), and the full network-enabled gate subsequently passed. The six Python
probe safety tests, local milestone links and `git diff --check` also passed.
At that qualification-only checkpoint, Q01 and application acceptance C01–C15
remained incomplete, and ordinary release artifacts had not yet been refreshed.
Final implementation and artifact acceptance are recorded below.

## Image drift counterexample and proposed resolution

Status: Active — owner approved the byte-hash resolution on 2026-11-13 ("Continue.").

The owner completed all three requested manual edits. Read-back showed:

- Replaced image: same block ID and caption; `last_edited_time` changed from
  `2026-11-13T10:06:00.000Z` to `2026-11-13T10:13:00.000Z`; file-path SHA-256
  changed to `0f029f5d74bb8b99030ec44323ce731f92a1ae9c88397c4d527f0d97b118a0d8`.
- Caption-only edit: same block ID and file-path hash; caption SHA-256 changed to
  `70ece91455bb576815fd55534066010e45b1b7495267d76807e654e58faa5cc9`;
  `last_edited_time` became `2026-11-13T10:13:00.000Z`.
- Deleted image: block `417de2b7-05cc-8134-bc41-c92f8e07aec5` no longer appeared
  in the complete child list.

Those observations initially appeared to support the proposed evidence, but the
minute-resolution timestamps warranted a same-minute check before acceptance.
The owner-edited examples were left intact. This command created a separate
synthetic image block, read it, replaced blue pixels with red pixels in place,
then read that block and the original upload:

```sh
DIARY_NOTION_LIVE=1 python3 scripts/smoke/notion-live.py --qualify-pages --config config/global_config.yaml --probe-rapid-image-edit 417de2b7-05cc-81a9-9909-df696928a7c0
```

The replacement returned HTTP 200. The command exited 1 as intended because the
proposed evidence did not change:

| Field                               | Before replacement                                                 | After replacement                                                  |
| ----------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------ |
| Block ID                            | `417de2b7-05cc-81b5-b3dd-da5e3ac74be8`                             | Same                                                               |
| `last_edited_time`                  | `2026-11-13T10:15:00.000Z`                                         | Same                                                               |
| Caption SHA-256                     | `9fa6fce1007faedb9c02541d7176e59926674eca706c9e16a2ec5f78a4845365` | Same                                                               |
| File-path SHA-256 (diagnostic only) | `e4ef5f9bb51ed856382e8ba3ce1fb9733d6012edb1c3933028c0cbe3c31409fc` | `6f0a541595c4cf3cd168ea90946d4d9e87d8ae62d4d6e2ed08152c3aadf39592` |
| Original blue upload status         | `uploaded`                                                         | `uploaded`                                                         |

Retained blue upload: `417de2b7-05cc-8178-b1fe-00b21d1814c9`; red upload:
`417de2b7-05cc-81e0-b2c8-00b2213cf325`. Image file read-back still exposes only
`url` and `expiry_time`, with no upload ID. Block identity, caption, edit timestamp
and original upload status therefore cannot establish unchanged image content.
At this checkpoint, stage C step 4 and overview Q01 stopped application implementation.
The owner subsequently approved byte hashes and implementation resumed. No requirement for stable URLs, ordering or drift protection
has been weakened.

Proposed resolution: include SHA-256 of the actual hosted image bytes in Illustrated
observations and receipts, alongside block identity, caption and order. Keep
timestamps as supplementary evidence and upload IDs/status as upload provenance,
not proof that an image block still references that upload. Signed URLs, expiry and
URL paths remain excluded from semantic digests. Specifically:

1. At post-append verification, download each returned hosted image and require
   its hash to match the operation's prepared JPEG hash before authorizing any
   old-body cleanup or receipt publication.
2. On every later remote observation, hash the current hosted bytes again. Never
   skip that read based on an unchanged timestamp, block ID or upload status.
   A changed byte hash is external drift and requires the existing observation-bound
   confirmation. Unsupported/external image types and failed/oversized downloads
   make the observation unavailable, not unchanged or absent.
3. Use a separate credential-free HTTPS fetch path, allowlisted to qualified Notion
   attachment hosts, with redirects disabled, bounded streaming reads, cancellation,
   a 30-second per-request timeout and explicit per-file/total byte budgets. Download
   one image at a time. Do not fetch arbitrary Markdown URLs or forward the integration
   token, API cookies or local credentials. A newly observed unqualified host fails
   closed pending qualification.
4. Re-read image content immediately before admitted destructive cleanup as part of
   the existing observation comparison. Keep the documented limitation that Notion
   provides no compare-and-swap transaction; this detects observed drift but cannot
   eliminate an edit racing a write. Recovery remains read-only until safe work is
   explicitly admitted.
5. Budget the added hosted-image downloads for up to 99 photos. An unchanged upload
   remains free of writes/reuploads but now requires image-byte reads. Cache only
   operation-owned prepared bytes; a previous remote hash cannot stand in for a
   fresh observation.

Read-only feasibility qualification was completed with:

```sh
DIARY_NOTION_LIVE=1 python3 scripts/smoke/notion-live.py --qualify-pages --config config/global_config.yaml --qualify-image-bytes 417de2b7-05cc-81a9-9909-df696928a7c0
```

The retained blue and red hosted fixtures each downloaded as 5,763 bytes. Their
SHA-256 values exactly matched independently regenerated local JPEG fixtures:

- Blue: `b17278b0b46163d055e7f79eda8a2171d9f2c8f7486ac4b12da843e122db7a11`.
- Red: `381501b969c79990a661eb62a156d1d1f482db1da20b7e3a416bc48a9c3d07c3`.

This supports the proposed byte-hash comparison for these fixtures. It does not
establish application implementation, all possible attachment hosts, the 99-photo
budget, or a universal no-transcoding service guarantee. Those remain implementation
checks after owner approval. Nine offline tests now cover the original probe
safety guards, the same-minute counterexample, hosted-URL rejection, credential-free
fetching and byte bounds. No further remote writes are required for owner review.

Validation after the counterexample: full gate `run.TvQ5fE` passed all seven lanes;
the nine Python probe tests, local milestone link checks and `git diff --check`
passed. This validates the qualification tooling and documentation, not the
unimplemented application feature. The owner approved the resolution; implementation proceeds with byte hashes as required drift evidence.

## Implementation review

Owner approval adopts hosted-byte hashing for Illustrated remote evidence. Q01's original metadata proposal is rejected; positional insertion and native JPEG upload passed. The replacement evidence is qualified by the downloaded blue/red JPEG hashes above.

The implementation adds an explicit `ContentKind` axis and a separate Illustrated start request carrying a published version ID, while retaining existing Authentic callers. Persist kind/version on export operations with historical rows defaulting to Authentic. Keep destination hashes and receipt keys unchanged; Illustrated uses the date as its stable slot key.

Prepared JPEGs will use bounded private SQLite rows committed atomically with admission rather than operation-owned files. Review rationale: the existing SQLite snapshot lifecycle can atomically delete these rows with every terminal transition and retain them across restart without a second staging journal. Each JPEG is capped at 5 MiB, all JPEGs at 64 MiB, with at most 99 rows; capture decodes/converts one PNG at a time and retains only bounded JPEG output. The Markdown/manifest keeps the existing snapshot cap. No PNG originals or generation metadata enter the durable export snapshot. Output pixels are bounded at 16 million, sRGB with white alpha flattening; malformed PNGs or PNGs with non-default orientation fail before remote mutation. Generation EXIF metadata without rotation is accepted; see the [release regression follow-up](../../../phase-1/milestone-32-personal-release-overview.md#illustrated-upload-exif-regression-follow-up-2026-11-13).

## Implementation acceptance

Implemented without new dependencies, Cargo features or build tools. The review's
private SQLite JPEG rows replace the proposed file staging: admission and terminal
cleanup are atomic with the existing ledger. Migration 0008 adds `content_kind`
and `illustrated_version`, preserves prior rows/hashes and rebuilds `notion_steps`
with upload/verification step kinds. Illustrated snapshots bind the published
version, body/source PNG hashes, conversion settings, ordered text/image manifest
and retained JPEGs. Preparation is capped at 99 photos, 5 MiB each and 64 MiB total;
remote verification permits 128 MiB while the two bounded bodies temporarily coexist.
An already oversized partial/foreign old body is refused before growing another copy.

| Checklist | Evidence                                                                                                                                                                                                                                                                                                                                                                                     |
| --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R03, C01  | `config.rs`, `config/reload.rs` and `config/tests.rs`: independent normalized roots, duplicate/equal/unknown/legacy/mixed-field rejection, secret-free typed migration and restart-to-apply/busy behavior.                                                                                                                                                                                   |
| C02       | `sqlite::tests::illustrated_migration_preserves_authentic_receipts_pending_work_and_notion_steps` retains pre-0008 receipt, snapshot, reservation and pending step rows. Existing 98 export tests passed; final full Rust gate retains Authentic/iCloud coverage and recovery.                                                                                                               |
| C03–C04   | `export/tests/illustrated.rs` covers stale capture, deletion after admission, retained photos through restart and explicit confirmation, and all 99 photos. `export/jpeg.rs` tests JPEG signatures, height 480, 960/240 aspect-correct widths, white transparency, unchanged input and excessive-resize rejection.                                                                           |
| C05–C07   | Paginated fake-service integration tests cover ordered hierarchy, stable day IDs, no writes/reuploads on no-op, automatic local replace, byte-only same-minute drift and foreign/duplicate page refusal. Shared export decision/confirmation tests retain external-change/deletion/unowned/root-change coverage.                                                                             |
| C08–C09   | Per-mutation lost-reply tests cover page creation, upload creation, append and cleanup; exact upload-ID read reconciles a lost send without resend. Expiry and cancellation preserve old body, report Unknown and never delete blindly. Restart preserves snapshots/reservations and never replays writes. TypeScript tests keep tracking through temporary disconnect without resubmission. |
| C10–C11   | Shared action/controller tests capture date/version once, block duplicates, avoid Authentic flushing/loading, admit upload without source/viewer/profile/engine and recheck retained handlers. Normal export/dialog tests remain passing. Packaged proof below verifies actual Tauri bridge behavior.                                                                                        |
| C12–C13   | Isolated persistent fake-server, packaged native and authorized real-service runs below. Existing redacted diagnostics remain in use, including apply-stage logs; snapshots/credentials never enter log records.                                                                                                                                                                             |
| C14–C15   | Maintained product/config/UX/user/action/smoke/handoff/release docs updated together. Final gate, harness and ordinary artifact evidence below; M32 remains Active.                                                                                                                                                                                                                          |

Commands and observed results:

```sh
cargo test --manifest-path src-tauri/Cargo.toml -p diary-infrastructure --lib export --quiet
cargo test --manifest-path src-tauri/Cargo.toml -p diary-infrastructure --lib illustrated --quiet
npx vitest run tests/export.test.ts tests/export-ui.test.tsx tests/app-actions.test.ts tests/illustrated-ui.test.tsx --reporter=dot
python3 scripts/smoke/illustrated-notion.py
npm run tauri build -- --debug --bundles app --features scaffold-smoke
python3 scripts/smoke/illustrated-notion.py --gui-app "src-tauri/target/debug/bundle/macos/Narihito's Diary.app"
DIARY_NOTION_LIVE=1 python3 scripts/smoke/illustrated-notion.py --live
```

The persistent fake-server run passed create/no-op/backdated hierarchy/stable-page
replace/restart/original hashes/terminal JPEG cleanup. The packaged native run
reported `state: passed` for date `2026-11-13`, `packagedOrigin`, `virtualAuthentic`,
`noViewer`, `pendingGuard`, `navigationPreserved` and `missingEntryGuard`. It used
an isolated synthetic destination and private settings, with no saved Authentic,
AI profile or image engine. It does not establish a human visual/IME observation.

The first live integration run retained root
`417de2b7-05cc-812b-858d-e31604450757`, with stable day page
`417de2b7-05cc-817b-9b49-da79dff86f06`. A subsequent expanded run explicitly tested
middle insertion at the year level too, retaining root
`417de2b7-05cc-81ee-ae4b-e926a1170f72` and stable day page
`417de2b7-05cc-8197-bc1f-ca51dcdb8995`. Both roots are direct children of the
owner-authorized test root; no production root was used. All fixture pages remain.

The expanded run uploaded `2036-05-15`, `2032-01-01`, `2038-01-01`, `2034-01-01`,
`2036-01-01`, `2036-03-01`, `2036-12-01`, `2036-05-01`, `2036-05-10` and
`2036-05-30`. The sequences exercise first/middle/last year/month/day insertion.
No-op retained the receipt/page/body fingerprint. Local replacement retained the
same day page ID and changed its body fingerprint; after server restart the
unchanged upload retained the receipt again. Text/photo/text order and absence of
private generation metadata were checked through real read-back. The hosted image
was JPEG (16,125 bytes); an additional read-only frame check confirmed 960 × 480 pixels. Local PNG/Markdown hashes were unchanged and no terminal
JPEG rows remained. Online requests used API `2026-03-11` and the app's privately
configured token. Real pages were never deleted/archived; only owned test body
blocks were replaced.

### Validation and release artifacts

The first passing complete implementation gate was `RUST_TEST_THREADS=1 scripts/gate.sh`,
run `run.8g7FBm`: all seven lanes passed, including 515 frontend tests in 68 files.
An earlier parallel Rust run had one `WorkspaceBusy` failure in the existing
Authentic crash-boundary test; all 358 infrastructure tests passed with Rust tests
serialized (two explicit live tests remained ignored). No test was disabled to
obtain a pass. A final gate after the apply-log/two-body read-budget review is
recorded below.

Python harness validation passed all 28 then-existing tests; two additional
upload-harness readiness/live-gate safety tests passed separately. The latter
ensure a session-admission failure stops its owned child before propagating and
that live mode refuses before build/token access without explicit opt-in.

Ordinary release server and app were rebuilt. A sandboxed Swift macro failure
was detected, so the existing build script was rerun with macOS permission and
the Apple helper was embedded successfully. A fresh private copy outside the
checkout is retained under `diary illustrated release 5dhbk0r2` in the session's
temporary directory. The JSON artifact report records all 423 artifact/resource
hashes, arm64/version/notices/fixture exclusion and the embedded helper marker.
Copied-server lifecycle/lease/restart/stale-discovery/sentinel checks passed; an
initial process-group permission error during cleanup was retried successfully.
Explicit-artifact fake-agent smoke passed without rebuilding or substituting the
server. No production app was installed, published or deployed.

Final verification after the last server corrections: `RUST_TEST_THREADS=1 scripts/gate.sh`
run `run.piIEFc` passed G01 fmt, G02 clippy, G03 cargo test, G04 prettier, G05 eslint,
G06 vitest and G07 tsc. All 515 frontend tests and all 30 Python harness tests passed.
The final ordinary server was recopied, rescanned and passed copied-server and
explicit-artifact fake-agent smokes again. Final SHA-256:

- Server: `6bee55b89fd27c7d9f925d9082721e7fd37baf71dd10c82d58dd4f98ea5175d1`.
- Standalone/bundled client: `5b9752c9f5b8454ff06066f3ddb13af64fd78fac0a7353a9f120435a6e70701a`.

The final fixes affected server logging and the bounded two-body read budget; client
bytes remained identical. No dependency or notice inventory changed. All completion
items are accepted on the evidence above; M32 and overall release closure remain
independent. The private configuration was not rewritten: users must explicitly
migrate legacy Notion roots to `notion.pages` and restart the server.

Final nonregression: `python3 scripts/smoke/export.py` passed after its synthetic Notion config was updated to `notion.pages`, covering the existing server export/recovery workflow. Final changed-document local links, archive formatting and `git diff --check` passed.
