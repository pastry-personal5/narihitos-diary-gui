# P1-M36: Illustrated diary Notion upload

Status: Done

Upload the selected date's published Illustrated diary to Notion, with one entry per day page under a separate year/month hierarchy. Completed 2026-11-13 with fake/server/native/live acceptance, the seven-lane gate and refreshed ordinary release artifacts. The [work plan](milestone-36-illustrated-notion-upload-architecture.md) defines boundaries, sequencing and verification.

## Entry conditions

[P1-M34 creation](../p1-m34/milestone-34-illustrated-diary-overview.md) and [P1-M35 GUI](../p1-m35/milestone-35-illustrated-diary-gui-overview.md) are Done. This owner-requested follow-up is planned before P1-M32 release closure; M32 remains Active. Read the existing [export contract](../../../architecture/configuration-and-exports.md), [Notion delivery](../../../phase-1/milestone-27-notion-export-overview.md) and [contribution rules](../../../contribution-guide.md) before implementation.

## Owner decisions (2026-11-13)

- Add a dedicated Main Toolbar Strip button to upload the selected date's Illustrated diary. Notion is the only supported destination for this kind.
- Use separate configured roots for `normal` and `illustrated`, sharing the Notion token. Reject legacy `notion.root_page_id`; do not silently migrate configuration.
- Titles: year `YYYY`, month `YYYY-MM`, day `YYYY-MM-DD (WED)` using the date's uppercase English weekday. Each Illustrated day is a child page, rather than an Authentic-style heading section.
- An unchanged upload is a no-op; automatically replace changed local Illustrated content. Retain existing protections for external Notion changes, deleted destinations and unowned pages; automatic replacement does not silently authorize overwriting external changes.
- Keep the same day page and URL across replacements. Multi-request content replacement is not atomic; partial and ambiguous outcomes need honest recovery.
- Upload rendered body text and photos in their original order. Exclude YAML front matter, prompts, seeds and other generation metadata.
- Convert upload copies to JPEG, exactly 480 pixels high, with proportional integer-rounded width and quality 85. Use a white background for transparency. Preserve original local PNGs and Markdown.
- Require ascending chronological order for year, month and day child pages, including backdated uploads. Preserve page identities; positional insertion needs early live qualification.
- Allow independently configured, distinct roots, at most one per type.
- Include a real online test with fixture text/photos. The owner authorizes writes under the dedicated [test page](https://app.notion.com/p/test-page-417de2b705cc80239abdd1fc1654154f?source=copy_link), root ID `417de2b705cc80239abdd1fc1654154f`. Leave created test pages for inspection; no page deletion/archival is authorized. Replacing test body blocks is part of the upload test.

## Configuration

The requested replacement shape is:

```yaml
notion:
  token: "ntn_..."
  pages:
    - type: normal
      root_page_id: "<normal-root-page-id>"
    - type: illustrated
      root_page_id: "<illustrated-root-page-id>"
```

`normal` maps to the existing Authentic Notion export; it does not rename the local diary kind. Allow either type independently, at most one root per type, require a nonempty list, normalize/validate page IDs, reject unknown types, duplicate types, equal normalized roots and mixed legacy/new fields. Omitting `notion` disables both Notion destinations. The owner confirmed independent, distinct roots during the interview. Keep `schema_version: 1`; the legacy field gets a typed, secret-free migration error naming `notion.pages`, where today every schema failure is the same generic error. Real root IDs stay in the private config; maintained docs use placeholders. A Notion config change still takes effect on server restart, as today; M36 adds no live adapter re-registration.

## Scope

- Strict config parsing, per-kind registration/availability and reload handling.
- A version-bound snapshot of published Illustrated text and all generated photos.
- Bounded temporary JPEG preparation, native file upload, page hierarchy creation, stable-page content replacement, receipts and recovery.
- A distinct accessible toolbar bitmap/action and command-palette entry, shared action guards, confirmation/cancel/recovery UI and Global Messages.
- Fake HTTP, application, bridge, UI and isolated server/native smoke coverage; documentation and the full gate.

Out of scope: other upload destinations, batch/date-range upload, automatic upload after generation, editing in Notion and syncing back, publishing the Authentic diary as a day page, metadata attachments, modifying original photos, remote deletion when a local Illustrated diary is deleted, new dependencies or build tools without approval.

## Interview completion and implementation prerequisites

The product interview is complete. Before any other implementation (checklist Q01), qualify two service behaviors against the real API on the test root:

1. **Chronological insertion.** The M27 live test rejected page positioning, while the current create-page schema lists `position` without prose or caveats.
2. **Image read-back.** A read of an image block returns only a temporary URL and expiry, with no documented stable file identity, so replacement detection must rest on other evidence (work plan, stage C step 4).

Do not weaken ordering, stable URLs or drift protection if either fails; stop, and return with concrete findings and a proposed resolution.

The test root above is supplied and fixture writes are authorized. Use the existing privately configured integration token without printing it; verify that integration can read/write the test root before testing. Connected-app access is not proof of access through the app's token. The connected Notion reader was unavailable during planning; the page link alone does not establish permissions. Never substitute either production root as a fallback.

## Completion checklist

Check only with linked evidence. Planning checks do not establish implementation acceptance.

- [x] Q00 — Run a reproducible, explicitly gated service probe with the private integration token and fixed authorized test root, retaining created pages and recording redacted results. Stop broader implementation if Q01 cannot pass. Evidence: [qualification execution](milestone-36-illustrated-notion-upload-architecture.md#qualification-execution-2026-11-13); page positioning passed; manual image edits were observed, but the same-minute replacement counterexample fails the proposed drift evidence. Q01 includes the subsequent owner-approved byte-hash resolution.

- [x] R01 — Inspect existing Illustrated storage, export snapshot/ledger, Notion transport, configuration/reload and GUI export controller. Evidence: inspected paths and findings in the work plan.
- [x] R02 — Resolve ordering, live acceptance and config edge rules; record owner answers and reconcile the work plan. Evidence: owner interview on 2026-11-13, decisions above and supplied test root.
- [x] Q01 — Gate for all later items: a recorded live run on the test root shows (a) positional child-page creation at start, middle and end under API version `2026-03-11`, and (b) which image-block read fields change when an image is replaced or edited in Notion. Evidence: script output and observations in the work plan. On failure, stop and return to the owner. Owner approved the qualified hosted-byte-hash resolution after metadata-only evidence failed; see the [counterexample and proposed resolution](milestone-36-illustrated-notion-upload-architecture.md#image-drift-counterexample-and-proposed-resolution).
- [x] R03 — Review migration, receipt namespaces, snapshot limits, JPEG conversion and image read-back design against actual code; record exact interface/schema changes. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C01 — New config accepts independently configured kinds and rejects legacy/mixed fields, invalid roots, unknown/duplicate types and ambiguous roots. The legacy shape yields the typed migration error. Credentials remain server-only. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C02 — Existing normal Notion and iCloud exports preserve their behavior and receipts; Illustrated operations never collide with their identities, reservations or recovery. A database with pre-M36 receipts and an unresolved Authentic export migrates and recovers unchanged. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C03 — Capture one published Illustrated version and all photos before remote writes; regeneration/deletion during capture produces a typed stale outcome without mixed content. A completed snapshot survives source replacement and restart. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C04 — JPEGs are height 480, aspect-correct, quality 85, with white transparency flattening; originals remain byte-identical. Conversion and size failures occur before remote mutation; preparation and cleanup are bounded, including the 99-photo maximum. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C05 — Unique year/month/day pages are created on demand with decided titles in ascending chronological order, including first/middle/last backdated insertion at every level. Duplicate or unrepairable disordered hierarchy is a typed conflict, including after pagination and concurrent submissions; stable page IDs survive replacement. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C06 — Native JPEG uploads and text/image projection preserve order, alt text/captions and Unicode. Metadata never appears in uploaded content. Upload lifecycle, size/rate limits and expired uploads are handled explicitly. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C07 — Unchanged uploads do no writes; local changes update the same day page automatically. External drift, deletion and unowned content use confirmed, observation-bound decisions. Unsupported foreign blocks prevent destructive cleanup. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C08 — Durable per-step checkpoints cover photo upload, page creation, append, verification and old-block cleanup. Ambiguous writes are never replayed blindly; recovery is read-only until explicitly admitted safe work. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C09 — Cancel, disconnect, restart, lost replies and root changes preserve reservations and accurate partial/uncertain outcomes. Cancellation never claims rollback or deletion of uploaded files. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C10 — Toolbar and palette use the selected date regardless of active editor kind; explain missing Illustrated entry, unavailable root/storage/connection and conflicting operation. No saved Authentic entry, open viewer, AI profile or ready image engine is required to upload an existing Illustrated entry. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C11 — Submission captures date/version once, blocks duplicates, tracks progress and handles confirmation/uncertainty without changing selected date or focus on completion. Normal export dialogs retain correct destination wording. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C12 — Fake HTTP and isolated server smoke cover create/no-op/replace, JPEG properties, paginated hierarchy, external edits, per-request failures, cancellation, crash/restart and secret/content-free logs. Packaged native smoke verifies the new button, guards and status flow. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C13 — Authorized live test against the dedicated test root passes create/no-op/backdated insertion at each hierarchy level/stable-page replace/JPEG rendering/restart. Record actual service observations, fixture ownership and retained test pages without claiming unrun proof. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C14 — Update README, product behavior, configuration/export architecture, user guide, UX/action/shortcut docs, smoke docs, handoff, phase plan, roadmap, decisions and docs index together with implementation. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).
- [x] C15 — `scripts/gate.sh` passes; record all seven lanes. Refresh affected ordinary release artifacts and M32 evidence before release closure. Evidence: [implementation acceptance](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance).

## Completion record

Completed 2026-11-13. The owner-approved hosted-byte correction preserves external-drift safeguards despite minute-resolution Notion timestamps. All seven gate lanes passed (`run.piIEFc`), with 515 frontend tests and 30 Python harness tests, authorized live fixtures, packaged native proof and refreshed ordinary release scans/process/agent checks. See the [acceptance record](milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance) for retained pages, hashes and limitations. M32 remains Active.
