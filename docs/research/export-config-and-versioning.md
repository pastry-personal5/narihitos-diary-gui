# Export, configuration, and versioning findings

Status: Active

Research date: 2026-11-05. Covers R6.1–R6.5, R9.1–R9.5, and R11.1–R11.2. The [prior-art report](source-formats-and-prior-art.md) identifies inspected source revisions; external sources below were read on this research date. No real Notion workspace, token, user configuration, or iCloud output was accessed or changed.

## R6.1 iCloud export

The prior art uses `LocalOutput` for both local and iCloud files, with byte-identical Markdown and `YYYY/MM/YYYY-MM-DD.md` layout. Its mirror preflight confines the root under Mobile Documents, rejects an unavailable root, detects `.<filename>.icloud` placeholders, and verifies target bytes. New files use owner-private temporary files and no-clobber finalization; replacement requires the expected digest of an owned file. Its automatic mirror UX is not implied by the GUI's optional export requirement.

Recommendation: local Entry save succeeds independently of optional exports. Track iCloud export status/revision separately; retry the same immutable local revision rather than the editor's latest mutable buffer. Do not interpret an evicted or unreadable file as absent, and do not infer upload completion from a successful local write. The `.icloud` convention alone is not proven complete for modern File Provider availability states. Compare ordinary file writes with a coordinated native adapter before claiming conflict safety; synthetic packaged tests must cover download-needed, concurrent edits, root removal, and ambiguous interruption.

## R6.2–R6.3 Notion contract

[Notion authorization](https://developers.notion.com/guides/authorization/overview) currently documents PATs, internal connection tokens, and OAuth. The owner's "PAT" wording is therefore valid; do not silently replace it with an internal-integration-only requirement. An internal connection needs the target page shared with it. Compare a user-scoped PAT against a narrowly shared internal connection; either can fit a personal app, but access scope must be understood before choosing. Read/insert/update needs should match the selected export/update operations.

The inspected prior art uses API `2026-03-11`, year child pages, month child pages, and dated daily sections within a month. This is a strong reuse candidate, not owner approval of the exact page hierarchy. [Notion versioning](https://developers.notion.com/reference/versioning) requires `Notion-Version`, tolerating additive fields, and treating pagination cursors as opaque. Use stable record IDs rather than parsing display URLs.

[Append block children](https://developers.notion.com/reference/patch-block-children) supports start/end/after-block insertion, at most 100 children per request, and up to two levels of nesting per request; it does not move existing blocks. This matches the prior-art ascending insertion strategy: locate the prior dated section, insert after its last block, then chain later chunks after verified newly created blocks. That preserves order on an already sorted page. Historical disorder and ambiguous same-day headings need a separate owner policy; insertion alone does not repair them.

[Current request limits](https://developers.notion.com/reference/request-limits) include 2,000 characters per rich-text content item, 100-element arrays, and overall 500 KB/1,000-block payload bounds. Per-connection budgets vary by plan, with additional workspace/endpoint limits. Therefore, split rich text and block batches by all limits, paginate reads, and honor `Retry-After` rather than hard-code one universal requests-per-second assumption. Long entries need deterministic chunking, including non-ASCII text and nested lists.

## R6.5 Recovery and remote edits

Compare page-per-Entry export (simpler stable per-entry identity, more pages) with the prior-art month sections (matches the initial ascending-in-page intent, more block reconciliation). Recommend evaluating month sections first because the owner explicitly wants ordered entries inside a page, while leaving hierarchy confirmation open.

Persist a receipt keyed by destination/root ID, local asset ID, exported revision digest, renderer/API version, and remote page/block IDs. A timeout after a write is ambiguous: read back before retrying, and adopt matching results when uniquely identifiable. Never blindly replay page creation or the whole block list. Partial writes require a durable intent/checkpoint and deterministic reconciliation; a receipt only in a rebuildable cache is insufficient.

For an existing remote section, compare overwrite-with-expected-digest, explicit append, and create-a-new-version. Recommend conflict detection before any replacement of user-edited remote content. Remote deletion, duplicate headings, changed anchors, changed root/config, renderer upgrades, and backdated local Entries need explicit outcomes. Existing connector tests cover several of these cases, but were inspected, not run. A synthetic-account smoke test remains needed before claiming production idempotence; no network write is authorized merely by research.

## R6.4/R9 Secrets and configuration

The initial project-root YAML path is an owner requirement for v1 configuration. An installed `.app` has no stable repository working directory; compare an explicitly chosen config path, a command-line/launch override, and a remembered path under app support. Recommend retaining an explicit path override plus the project-root development default, with an installed-app selection mechanism to be approved. Never write config inside a signed app bundle. No config discovery or migration policy is decided here.

The previous app's `LifecycleConfig` loads a private, bounded file and redacts secrets. Its hand-written YAML subset and launch-root-relative resolution are tied to its server app; reuse validation requirements rather than that parser wholesale. `server.port_fallback_count` controls old loopback-server discovery, not a required Tauri feature. Recommend omitting an app HTTP server unless another requirement demands it; ask before removing a user-facing config field.

[serde_yaml](https://docs.rs/serde_yaml/latest/serde_yaml/) marks itself unmaintained/deprecated. [serde_yaml_ng 0.10.0](https://docs.rs/serde_yaml_ng/0.10.0/serde_yaml_ng/) is an alternative Serde parser but documents YAML 1.1 semantics. Compare maintained parser choices, duplicate-key behavior, recursion/alias limits, and string coercions using synthetic config fixtures before adoption. Quote timestamp-like IDs and paths in examples; domain validation remains separate from YAML decoding.

Proposed validated config: version marker; unique stable source identity plus display name/type/location; workspace/Entry/export paths; Notion credential reference/root; agent profile alias/type/home/executable. Exact shape is P1-M2 work. Reject unknown required type/version, duplicate aliases/namespaces, invalid directories/executables, overlap that creates recursive import/export, and incompatible source/root permissions. Resolve relative paths against an explicit documented base, never an incidental Finder working directory. Validate changes as one snapshot; retain last valid config on reload failure and avoid changing an in-flight job's destination midway.

Compare literal token in private ignored YAML with a Keychain reference. The former preserves the initial example but places secrets in backups and process-readable files; the latter reduces accidental exposure but adds credential access and migration UX. [Keyring 4.2.0](https://docs.rs/keyring/4.2.0/keyring/) now distinguishes convenience APIs from app-specific credential-store integration, so do not copy an old feature recipe without checking the chosen crate/backend. No Keychain dependency or token migration is authorized here. Existing `.gitignore` excludes `config/global_config.yaml`; a tracked example must use placeholders. Rust should retrieve secrets only when needed, omit them from frontend state/logs, and never inherit them into agent processes.

`sources[].type` dispatches source parsers (`ayumi`, `narihito`); `agents.profiles[].type` dispatches agent adapters (`codex`, `claude`, `qwen`). Keep these namespaces distinct from asset type (`diary`). Adding a new source type means implementing/validating an adapter, not allowing arbitrary executable code through YAML. Profile homes belong to the CLI; the GUI must not read/copy login files just to report configuration status.

## R11 Version consistency

The owner requires display text `Version v1.0.0`; package/bundle versions should remain ordinary semantic versions such as `1.0.0`. [Tauri config](https://v2.tauri.app/reference/config/) defines application version metadata. Compare an explicit shared version input with a checked set of Cargo/package/Tauri versions; have the UI read app metadata rather than carry another literal. P1-M3 can add a lightweight consistency check using existing tooling after the source of truth is selected. No package manifests exist yet in the GUI, so there is no present mismatch to repair or version bump to perform.

P1-M2 implications: choose export hierarchy and expected-revision replacement policy, durable receipt/intent storage, installed config discovery, YAML/schema compatibility, credential strategy, and a version source of truth. Remaining owner questions are hierarchy, source write-back/config behavior, and installed/distribution UX; remaining execution evidence is packaged iCloud handling and a synthetic Notion round trip.
