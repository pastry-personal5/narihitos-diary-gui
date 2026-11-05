# Storage and asset lifecycle findings

Status: Active

Research date: 2026-11-05. Covers R3.1–R3.8 and R12.1–R12.4 as option analysis and a proposed lifecycle. The owner clarified that Authentic draft and Authentic Entry are one autosaved file; other transitions below remain proposals.

## Evidence

The owner's initial ideas require local files, explicit asset types, a git-like experience, transient candidates, and optional exports. A later owner clarification makes draft/Entry two presentations of the same Authentic file. [Prior-art findings](source-formats-and-prior-art.md) identify the inspected revisions and durable local-output/SQLite code. External library docs inspected: [Git](https://git-scm.com/docs/git), [git2 0.21.0](https://docs.rs/git2/0.21.0/git2/), [gix 0.90.0](https://docs.rs/gix/0.90.0/gix/), [rusqlite 0.41.1](https://docs.rs/rusqlite/0.41.1/rusqlite/), [SQLx 0.9.0](https://docs.rs/sqlx/0.9.0/sqlx/), [notify 8.2.1](https://docs.rs/notify/8.2.1/notify/), and [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html). No benchmark, crash injection, dependency installation, or storage spike was run.

## R12 Proposed lifecycle and decision gaps

| Asset/state                       | Established behavior                                                                     | Proposed transition or unresolved rule                                                                                                                                           |
| --------------------------------- | ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Source Snippet                    | External Markdown; explicit edit/save and confirmed snippet-plus-audio deletion approved | Import records provenance/revision. Separate source edit/write-back from import; metadata updates and guarded related-file deletion need specification.                          |
| Imported Snippet                  | Listed by source, checked by default; focus shows Markdown                               | Re-import must distinguish unchanged, revised, and conflicted content. Whether local edits also write to source is unresolved.                                                   |
| Candidate                         | Multiple suggestions, transient                                                          | Explicit adoption into Authentic draft; replace/merge and overwrite protection remain owner choices. Never autosave a candidate as if authentic.                                 |
| Authentic draft / Authentic Entry | **One file, autosaved**, per owner; editable and optionally exported                     | No separate promotion/copy. Versioned autosave acknowledgement and failure display recommended. Candidate adoption, reopening, and one-per-date identity remain to be specified. |
| Historical revision               | Git-like experience requested                                                            | Propose history/diff and restore-as-new-revision. Decide retention, restore scope, and which asset types participate.                                                            |

Autosave needs a per-draft ordered queue: a slow older write must not overwrite a newer revision, and acknowledgement must identify the exact revision persisted. Debounce is an optimization, not a guarantee that pending edits survive immediate process termination. Show saving/saved/failed state and retain unsaved content on errors. Close/date-change behavior while saves are pending needs an owner rule; do not silently discard edits or claim crash recovery without a durable write.

The product and UX docs are placeholders. Keep this transition table as a proposal until owner approval, then update those canonical docs before implementing the corresponding behavior. The previous Phase 1/v1 scope conflict is resolved: the owner expanded Phase 1 to v1.0.0 on 2026-11-05; subsequent feature milestones are still unplanned.

## R3.1 History options

| Option                                              | Benefits                                                                               | Costs and unresolved evidence                                                                                                                                                                     |
| --------------------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Git CLI in an app-managed repository                | Standard objects/history/diff and external inspection                                  | Finder executable discovery; subprocess/error handling; isolate config/hooks/filters/signing, author identity, and unrelated user changes. Autosave does not have to mean a commit per keystroke. |
| `git2`                                              | In-process libgit2 API; no external Git executable needed                              | Native dependency/build surface; map app operations to repository locking and errors; inspect selected feature licenses.                                                                          |
| `gix`                                               | Rust repository API with explicit trust model                                          | Verify every required write/diff/restore operation and feature set before selection; API/feature complexity differs from CLI.                                                                     |
| App-managed immutable snapshots + revision manifest | Direct mapping to asset revisions and autosave; no repository tool/config side effects | Must build diff, restore, retention, integrity checks, and recovery; no Git interoperability automatically.                                                                                       |

Recommendation for P1-M2: choose the smallest history contract the owner actually needs before selecting the engine. Compare explicit history checkpoints plus crash-safe autosave against committing every autosave. If ordinary Git interoperability is a requirement, prefer a real Git representation; otherwise app snapshots remain viable. Never initialize history in the source folder or this code repository as an incidental consequence of import. History containing removed diary text is still sensitive retained data.

## R3.2–R3.4 Files and metadata

Illustrative layout only: an owned root containing `assets/diary/snippets/`, a single `assets/diary/authentic/` collection, with separate private state and history. Compare date-oriented filenames (`entries/YYYY/MM/YYYY-MM-DD.md`) with stable asset-ID filenames plus date metadata: the former is easy to browse but assumes uniqueness per date; the latter tolerates multiple assets/date and renames. Preserve explicit asset type/kind and schema version; future financial records get another namespace rather than silently sharing diary fields.

Compare Markdown front matter with separate JSON metadata. Front matter keeps date/zone/identity beside portable text; sidecars preserve an entirely plain body but create multi-file consistency/recovery work. Recommend versioned front matter for managed Authentic files if owner-readable metadata is acceptable, with `kind`, stable `id`, explicit `diary_date`, timezone, and creation/update instants. Exact fields and normalization are a P1-M2 decision, not a new schema in this research.

Candidates stay in memory; the Authentic file is durable and is shared by the Draft and Entry presentations. Imported original bytes and provenance should be distinguishable from an edited normalized body. `paths.entries_root` owns local assets only if the owner confirms that expanded meaning; alternatively keep it strictly for Entries and choose a separate managed workspace. `paths.icloud_entries_root` is an optional export root in the ideas/prior art, not a second source of truth. Reject source/export/storage root overlap that could cause recursive re-import or unintended overwrite.

## R3.5 Index versus authoritative state

| Choice                    | Useful when                                                | Tradeoff                                                                                             |
| ------------------------- | ---------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Filesystem scan only      | Small catalog; few persistent jobs                         | Simplest persistence, but repeated parsing/search and duplicate tracking grow expensive              |
| Files + `rusqlite` worker | Local catalog, provenance, export receipts, ordered writes | Synchronous DB calls belong off UI/async executor threads; serialized worker is easy to reason about |
| Files + SQLx SQLite pool  | Async query integration is already needed                  | Pool/runtime/migration setup; async API does not make the file/DB pair a single transaction          |

Recommend files as content authority and a rebuildable catalog, but separate **non-rebuildable** information: remote page/block IDs, pending write intents, source namespace identities, and user choices cannot necessarily be reconstructed from Markdown. Either encode that durable metadata in owned files or back it up as authoritative state. SQLite transactions protect DB operations; they do not atomically commit an unrelated Markdown write or Notion request. Keep the live DB on local app storage rather than treating iCloud sync as DB replication.

## R3.6 Watching

`notify` documents macOS FSEvents, editor replacement patterns, debouncing helpers, and large-tree caveats. A Tauri filesystem plugin can expose watches to the frontend, but Rust ownership avoids granting a second path-access surface solely for monitoring. Compare native watch + debounce/rescan against periodic bounded scanning. Recommendation: treat watcher events as invalidation hints; rescan on startup, refresh, overflow/error, and relevant activation. Do not infer successful cloud download or complete writes from one event. Watch only configured roots and avoid `.assets`, private history, and generated files when unnecessary. Actual iCloud behavior remains a packaged macOS validation item.

## R3.8 Recovery requirements

Use a same-directory temporary file, bounded writes, flush/sync, atomic replacement where supported, and directory durability handling. The prior art's hard-link create prevents clobber on initial publication, but create-only export is not the same operation as frequent draft replacement. Compare immutable revisions plus an atomic current-pointer update with in-place current-file replacement plus a recovery journal.

Proposed write sequence: record intent/expected old digest → persist new revision → atomically publish current content → record history/index completion. Recovery inspects bytes/revision IDs before retry; it must classify crashes between every step. Compare single-instance enforcement with per-workspace locks; process-local queues alone do not protect against another app instance or external editor. External changes require expected-revision checks and a visible conflict, not last-writer-wins replacement of user content. File check-then-write races and iCloud coordination need platform evidence before claiming correctness.

P1-M2 implications: settle Authentic-file identity and autosave semantics and workspace ownership first; then select history/index engines, an ordered autosave contract, and a recoverable cross-file/DB write protocol. No implementation depends on the illustrative layout yet.
