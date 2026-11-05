# Configuration and exports

Status: Active

This contract defines private configuration, optional saved-revision exports, version ownership and platform integration. It complements the [architecture overview](../architecture.md); capability acceptance is separate from design selection.

## Configuration schema and validation

The server requires `--config PATH`; the conventional private file is `config/global_config.yaml`. No cwd search, environment token substitution or Keychain migration is implied. `paths.entries_root` and `paths.runtime_root` must be absolute paths; reject relative values for these fields. Resolve other relative path values against the config file's canonical parent. CLI executable overrides are optional; executable selection is defined below. The client gets only discovery path via `--connect`. Do not read private config during documentation or package a real credential in the app.

Illustrative schema 1, with invented paths/credential placeholders only:

```yaml
schema_version: 1
paths:
  entries_root: "/absolute/workspace"
  runtime_root: "/absolute/runtime"
  icloud_entries_root: "/absolute/export/root" # omit to disable iCloud export
server:
  port: 8781
  port_fallback_count: 8
sources:
  - alias: "mobile"
    type: "narihito" # ayumi | narihito
    root: "/absolute/source/root"
agents:
  profiles:
    - alias: "personal"
      type: "codex" # codex | claude | qwen | apple-foundation-models
      profile_home: "/absolute/profile/home"
      # executable: "/absolute/bin/codex" # optional override; otherwise codex from PATH
    - alias: "on-device"
      type: "apple-foundation-models" # no profile_home or executable
notion: # omit to disable Notion export
  root_page_id: "<page-id>"
  token: "<literal-private-credential>"
```

Optional source/profile lists default empty; no configured agents is valid. Source aliases/profile aliases are unique case-sensitive ASCII identifiers (`[a-zA-Z0-9_-]{1,64}`) within their own namespaces. Source type is distinct from asset/agent type. Parse YAML with limits of 256 KiB, depth 16 and bounded arrays (64 sources/profiles), reject duplicate/unknown keys, aliases/tags, implicit type coercion and unsupported versions. Require strings where strings are specified. `server.port` defaults to 8781 and `server.port_fallback_count` defaults to 8, covering 8781–8789 in ascending order. Ports are 1024–65535, fallback count 0–8, and the starting port plus fallback count must not exceed 65535. Credentials are nonempty bounded strings; never echo invalid input values.

Profiles form a type-tagged union. Each CLI profile (`codex`, `claude`, `qwen`) requires `profile_home`, mapped by adapter type to `CODEX_HOME`, `CLAUDE_CONFIG_DIR` or `QWEN_HOME`; the former `home` key is not accepted. `executable` is optional: when omitted, look up `codex`, `claude` or `qwen`, respectively, in the server's launch `PATH`. An explicit override takes precedence; resolve a relative override against the config file's canonical parent. Resolve the selected executable to an absolute path before invoking it directly with fixed argv, without a shell. See the [agent contract](editing-and-agents.md#agent-port-and-profiles) for adapter checks.

An `apple-foundation-models` profile requires only `alias` and `type`; reject `profile_home`, `executable`, API credentials and model/effort overrides for this type. It selects the native system model rather than searching `PATH` or creating a CLI home. A well-formed profile on an unsupported OS/device or with unavailable Apple Intelligence remains configured but unavailable; editing still works. See the [native adapter contract](editing-and-agents.md#apple-foundation-models-adapter). Agent-type names also select their [prompt asset directory](../../assets/prompts/README.md); paths are fixed by the adapter, not supplied by the WebView.

Require config owned by the current user, regular non-symlink 0600, with a private parent directory; validate before parsing. Runtime/private workspace state must also be owner-only. Reject root equality/nesting/physical aliases across workspace, sources, iCloud output and runtime; reject config/profile homes inside those data roots. Validate canonical existing parents for a proposed new workspace; create only after explicit server launch, then revalidate. A source missing/denied at startup is listed unavailable; it never becomes an empty successful import. An unavailable executable or missing/inaccessible `profile_home` directory disables only that profile without blocking editing; omitting `executable` alone does not disable it. Invalid schema, root overlap or unsafe credential permissions prevents startup.

The schema is fixed at 1; no silent migration. `reload_settings` is explicit: decode and validate a complete new snapshot first. Keep the last valid config on failure and report redacted field paths/codes. Changes to workspace, runtime, port or source type/root require restart, not hot-swapping identity beneath a buffer/job. Profile/export changes require no in-flight operations using them; otherwise return busy. Paused/uncertain operations count as in-flight for affected roots, credentials and destinations. Alias-only display changes do not change source identity. Successful reload increments config generation and invalidates previews; admitted jobs retain their exact nonsecret settings and secret reference until terminal. On restart, changed/missing settings needed by an intent leave it blocked under its original identity, never silently rebound. Rotate a credential only with no affected export in progress. Never return the token, token length, private home paths or arbitrary raw YAML to the WebView.

The explicit development origin, when used, is a server command-line development option limited to `http://localhost:1420`; it is absent in packaged use. The packaged allowed origin is the exact origin observed for the selected Tauri configuration (`tauri://localhost` target); validate that in the native smoke test rather than broadening to `*` if it differs.

## Runtime directory

`paths.runtime_root` is the absolute local directory for transient server connection state. The server publishes a discovery JSON file under `<runtime_root>/<workspace-key>/`; the client receives that file's path through `--connect`. It contains the running server's loopback address/port, process and instance identifiers, workspace key, protocol version and authentication token. The [launch contract](api-and-processes.md#launch-and-discovery) defines its exact fields and lifecycle.

Keep this directory separate from the workspace, sources and exports, with owner-only directories (0700) and discovery files (0600) because discovery contains a credential. The server removes its own discovery record on normal exit and publishes a new record with a fresh token on restart; stale records after a crash do not establish a live server. `entries_root` holds durable Markdown, SQLite, workspace locking and recovery payloads as defined in the [workspace schema](domain-and-persistence.md#workspace-and-file-schema). Those durable files do not belong in `runtime_root`.

## Export input and shared reconciliation

Exports are explicit and optional. Flush the Authentic buffer, capture its acknowledged revision and immutable bytes, and persist an operation intent before touching a destination. Unsaved buffers, candidates and sources are not export inputs. Local saving succeeds independently. The destination key includes canonical iCloud root or Notion root page ID plus workspace identity; changed roots create a new namespace, never reuse old receipts against another target.

Store local asset/date/revision, input/projection digests, renderer/API versions, remote identifiers/fingerprint and per-chunk checkpoints in authoritative SQLite. A remote observation includes complete normalized relevant contents, IDs/order and version metadata available from the service. If the current destination equals the last verified receipt, normal update is allowed. If changed/deleted, or an unowned existing file/unique daily section occupies the target, return `needs_confirmation` with a preview; do not silently overwrite/adopt. An absent unowned target permits first creation only when no unresolved earlier write reserves that target. Unchanged local revision plus unchanged remote digest is a verified no-op.

Confirmation token expires after 60 seconds and binds operation, session, exact saved revision, destination, observed remote fingerprint and proposed actions. Re-read immediately before mutation; a changed fingerprint requires a new preview. Each confirmation uses a new [step ID and expected checkpoint](api-and-processes.md#wire-types-and-preconditions). Accept/consume the token and checkpoint its authorized step atomically in SQLite before that step's side effect. A duplicate confirmation first returns the recorded step state even if its token is consumed/expired; it cannot run that step again. Explicit confirmation authorizes only that replacement/recreation, never future overwrites of arbitrary remote changes. Lost replies require read-back; do not blindly replay a create/append. If uniqueness or ownership cannot be established, mark uncertain and require inspection. After partial mutation, re-preview the actual current state before a destructive retry. Cancellation does not imply remote rollback.

## iCloud projection

Write byte-identical saved Authentic Markdown to `<icloud_entries_root>/YYYY/MM/YYYY-MM-DD.md`, without SQLite/private state. Use bounded coordinated availability/read checks, guarded no-clobber creation or expected-fingerprint replacement, staged same-directory publication and verified read-back. Record receipt after publication. File Provider download-needed/unreadable/evicted states are `cloud_unavailable`, never absence. No automatic download or upload-completed guarantee is made by a successful local write. An absent target after a known receipt is external deletion and needs confirmation to recreate.

Ordinary stat checks and a `.icloud` filename convention do not prove availability. The macOS adapter must test provider materialization and concurrent writes; unsupported provider states disable export. Native coordination is cooperative, so arbitrary concurrent external writes cannot be guaranteed lossless. Detect observable divergence, preserve staged saved-revision input and report conflict/uncertain rather than claiming sync success. A retry always exports the operation's fixed saved revision; selecting a newer revision is a new action.

## Notion hierarchy, projection and recovery

Use API version `2026-03-11` as the compatibility target from the [export research](../research/export-config-and-versioning.md), revalidated before implementation. Under the configured root, use uniquely identified year pages (`YYYY`), month pages (`YYYY-MM`), and one daily toggle section titled ISO date containing the projected body blocks. A toggle keeps one stable daily block anchor and supports a long body via paginated children. Receipts record actual page/section/block IDs; titles alone never confer ownership. Paginate all hierarchy/child reads, validate unique dates and ascending order before writing. Duplicate year/month/date targets or pre-existing disorder returns a structural conflict requiring external repair; do not silently re-sort or merge unknown content.

Insert a backdated daily section after the previous dated section, or at the start if earliest. Later chunks go under that daily container after their last verified child. [Notion append semantics](https://developers.notion.com/reference/patch-block-children) provide positional insertion and prohibit moving existing blocks; do not design recovery around a nonexistent move or atomic replace primitive. The deterministic renderer emits paragraphs/headings/lists/quotes/code as supported blocks; unsupported Markdown remains literal text with an explicit projection warning, never silently omitted. No remote image fetch or attachment upload. Preserve Unicode and split rich text on scalar boundaries.

Apply all version-specific service limits together: at most 100 children per append, 2,000 characters per rich-text item, documented nesting/array/payload limits. Use a conservative 400 KB encoded request budget beneath the researched 500 KB bound; account for wrapper overhead and Unicode bytes. Larger bodies are chunked deterministically with chunk digests. Validate current limits from the primary references linked in the research before enabling the adapter. Serialize hierarchy creation per destination/root, month creation per year, and daily changes per destination/month; two months must not concurrently create duplicate year pages. Honor `Retry-After` using a credential-scoped request queue and bounded jittered backoff for safe reads/explicitly rejected requests (maximum five attempts/60 seconds, then retryable failure if no side effect is unresolved). If the requested delay exceeds that budget, surface it and perform no early retry. Current [Notion request limits](https://developers.notion.com/reference/request-limits) distinguish retryable 429/529 throttling from blocked access and ambiguous 503 writes; keep versioned fixtures for their typed codes/guidance. Never retry an ambiguous write solely because of a timeout/5xx.

Before each write persist intended parent/anchor, normalized contents/chunk digest and prior remote observation. After response, read back IDs/content and checkpoint. For a 503 with committed-resource/child IDs, persist the allowlisted IDs and reconcile those exact objects; never log or expose the raw error body. After a lost create/append reply, paginate the relevant parent and compare the proposed content/location against the pre-write observation. A temporarily empty read does not prove an in-flight request cannot commit later. Without definitive non-commit evidence, zero matches remains `uncertain` and must not trigger a duplicate create/append, including after restart or a new operation ID. Multiple/partial matches also remain uncertain. One matching result establishes observed content; automatic ownership adoption additionally requires operation-specific provenance such as the returned committed IDs. Identical externally created content alone cannot authorize later destructive cleanup. Do not insert recovery marker text into the user's diary to guess ownership. Missing receipt/ambiguous ID disables destructive cleanup.

Updating a known section keeps its daily anchor: record the old child IDs/fingerprint and desired new projection, replace child blocks through checkpointed delete/append steps, and verify final complete content/order before committing the receipt. This is not atomic: a partially updated remote section is possible and is shown as pending/uncertain. Remove only children covered by the confirmed/unchanged observation, not newly discovered content. Reconcile each step before retry and pause on external drift. Notion has no app-level compare-and-swap transaction for these multi-request writes; immediate rechecks reduce but cannot eliminate edits racing an in-flight request. Do not advertise an absolute concurrent-edit protection guarantee. Acceptance must demonstrate observed conflict detection and honest partial outcomes, not mock a nonexistent CAS.

Required evidence: fake HTTP tests for paginated duplicate/disordered hierarchy, concurrent year/month creation, backdated first/middle/last insertion, long Unicode entries, rate limits/long Retry-After, deleted anchors, per-chunk failure, delayed commits after empty reads, 503 committed IDs, identical external appends, ambiguous duplicates, receipt loss and duplicate/expired confirmation recovery. Then a synthetic Notion-space round trip using the selected credential/API permissions must establish actual service behavior before enabling export. No real service writes are authorized by this design alone.

## Version and platform strategy

Use `package.json.version` as the eventual release source of truth, ordinary SemVer `1.0.0` for the first personal release. Set Cargo workspace package version and Tauri bundle version to the same literal, with a lightweight existing-tool check rejecting drift and UI reading app metadata (`Version v1.0.0`). `GET /status` reports server version, supported API range and storage/config schema versions. Client/server must have equal API major and compatible explicit minor capabilities; fail handshake if incompatible. Release, API, config, Markdown, SQLite, renderer and review-rule versions are independent. Newer unknown storage versions are read-only/error; migrations require a backed-up, exclusive operation, never automatic downgrade.

Target Apple Silicon, unsandboxed personal shell launch on the owner's macOS 26.7.1 research baseline. This is an acceptance target, not a tested minimum or a support claim for all later versions. Public distribution, Intel, signing/notarization and App Store packaging remain outside scope. TCC/filesystem access for the server and WKWebView/permissions for the client must be tested separately with the actual packaged identities. Finder launch is diagnostic evidence for absolute executable resolution, not a replacement for shell launch.

SF Symbols route: native AppKit runtime lookup (`NSImage(systemSymbolName:accessibilityDescription:)`) behind `get_symbol`, returning bounded in-memory PNG data for an allowlisted name/size/scale/appearance. React supplies accessible text independently; cache only in memory. No SF Symbols application download, exported template library, font or raster asset bundle is added to the repository. Apple's [API reference](<https:/developer.apple.com/documentation/appkit/nsimage/init(systemsymbolname:accessibilitydescription:)>) describes the lookup entry point; availability, appearance/Retina rendering and the native bridge remain to be tested. A missing symbol uses its accessible text label and reports unsupported symbol. The owner requested SF Symbols; this route does not establish license clearance or assign Apple's assets the project's Apache-2.0 license.

## Dependency candidates

These are document-level recommendations, not permission to install packages or change build tools. Check exact-version direct/transitive licenses, notices and platform features for Apache-2.0 compatibility before approval. The [research index](../README.md) links inspected alternatives and versions; revalidate APIs/maintenance before adopting.

| Need                 | Preferred candidate and rationale                                                                                        | Alternative/tradeoff                                                                   |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------- |
| Async server/HTTP    | Tokio + Axum; reqwest for bridge/Notion, separate DTO mapping                                                            | Other HTTP stacks possible; avoid sharing framework types inward                       |
| Serialization/config | Serde/serde_json; evaluate `serde_yaml_ng` with strict duplicate/alias/type fixtures                                     | Do not adopt deprecated `serde_yaml`; parser alone does not validate schema            |
| SQLite               | rusqlite (`=0.41.1`, `bundled`, approved 2026-11-05) behind one serialized connection, explicit migrations               | SQLx adds pool/runtime complexity without cross-file atomicity                         |
| Dates                | Jiff with explicit disambiguation and recorded tzdb source/version                                                       | Chrono + Chrono-TZ viable; never rely on default gap/fold handling                     |
| Digests/IDs          | sha2 and UUID random IDs from OS entropy                                                                                 | Never use a noncryptographic digest as a revision/security key                         |
| Watch invalidation   | notify with explicit rescan fallback                                                                                     | Periodic scans alone simpler but less responsive                                       |
| Markdown spans       | Rust lossless source-range tokenizer, evaluate pulldown-cmark offsets                                                    | A parser that loses separators is unsuitable for review/write-back                     |
| Native adapters      | Minimal AppKit/Foundation bindings, evaluate objc2 family; Apple Foundation Models needs a native Swift framework bridge | Concrete native binding/helper and SDK/build integration need approval before adoption |
| UI                   | Fixed React/CodeMirror stack; CSS Grid, reducer/context and accessible native controls                                   | Calendar/pane/state libraries only after measured need and approval                    |

No Git/history, Keychain, cloud-model SDK, global-shortcut plugin, Markdown preview, frontend filesystem/shell/SQL, new code generator or license-check tool is selected. The Apple system-model adapter is approved product scope; its concrete native dependency/build setup remains subject to approval. Source inspection can inform an algorithm; copied reference code needs attribution and compatibility review.
