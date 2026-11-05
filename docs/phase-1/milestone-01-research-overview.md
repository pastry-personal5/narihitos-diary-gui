# P1-M1: Research overview

Status: Planned
Goal: Plan the research needed to decide the architecture in P1-M2. This doc lists research items only; no research is done yet.

Source: owner's initial ideas (`tmp/initial-owner-ideas.md`, untracked). Exempt from the gate; see [changelog](changelog.md).

## Scope

In: investigating the items below and writing findings to `docs/` (one findings doc per group, or one doc if short).
Out: architecture decisions (P1-M2), code, and scaffolding (P1-M3). Prototype spikes are not planned; add one only if a question cannot be answered by reading.

## Research items

Priority: **A** blocks the architecture, **B** shapes it, **C** can wait for later milestones.

### R1. Source formats (A)

- R1.1 Ayumi snippet format: `tmp/Ayumi/YYYY/MM/DD/<timestamp>.md` front matter fields (tags, weather, location, transcription) and the `.assets` folder.
- R1.2 Narihito snippet format: `tmp/NarihitosDiary/YYYY/MM/DD/<timestamp>-<uuid>.md` front matter (`schema_version`, `kind`, `id`, language, audio, transcription fields) and `.assets`.
- R1.3 Which fields the GUI must keep, show, or ignore; how markdown-only import treats `.assets` and audio files.
- R1.4 UTC directory structure vs. local date: do directories use UTC dates, which timestamp is authoritative (`created_at` has an offset in Ayumi, `Z` in Narihito), and the rule for picking snippets for local dates D-1, D, D+1.
- R1.5 Snippet identity and de-duplication across repeated imports.

### R2. Prior-art codebases (A)

- R2.1 `contrib/narihitos-diary`: language, crate/module layout, and which parts are reusable (iCloud export, Notion export, entry model, config).
- R2.2 `contrib/narihitos-diary-mobile`: how snippets are produced; schema owner and versioning.
- R2.3 Known bugs and the "buggy workflow" so they are not carried over, including the Notion entry sort order that must be fixed (date ascending).
- R2.4 Licenses of both, and what porting to Rust/TypeScript means (copy vs. rewrite). Submodules are reference only, never a dependency.

### R3. Local storage and "git-like" model (A)

- R3.1 What "git-like experience" requires: history, diff, restore. Compare using real git (CLI vs. `git2`/`gix`) against an app-managed history.
- R3.2 Directory and file layout for personal assets with an explicit type (Diary: Snippet, Entry, Draft); how a future Financial Record type fits without restructuring.
- R3.3 File format for entries and drafts (markdown with front matter?), including local timezone metadata for authentic entries.
- R3.4 Authentic draft vs. AI draft candidates: what is persisted and what is transient.
- R3.5 Need for a database as index/monitor (SQLite via `rusqlite` or `sqlx`) vs. filesystem only; rebuildability from files.
- R3.6 Filesystem watching: `notify` crate or Tauri plugin, debounce, behavior on iCloud folders and large trees.
- R3.7 Meaning of `paths.entries_root` and `paths.icloud_entries_root`.

### R4. Tauri 2 on macOS (A)

- R4.1 Reading files outside the app: iCloud Drive paths, TCC prompts, sandboxed vs. unsandboxed builds, and the Tauri capability/permission set needed (least privilege).
- R4.2 Command and event model for backend features, including long-running work and progress (import, export, agent streaming).
- R4.3 Spawning and streaming external processes (AI agents) from Rust.
- R4.4 Dev/build flow on macOS Tahoe 26.7.1+ with Xcode CLT; whether the "generated Xcode project" mention in the ideas applies to a desktop Tauri app at all.
- R4.5 WKWebView limits relevant to the editor and layout.

### R5. AI agents (A)

- R5.1 Codex CLI: `codex_profiles` with `CODEX_HOME`, non-interactive and streaming modes (`codex exec`, app-server or other), auth per profile.
- R5.2 Chat session design: passing the Tabbed Editor content as context, session lifecycle, cancellation, history storage.
- R5.3 Generating several draft candidates from selected snippets: one call or several, prompt inputs, output parsing.
- R5.4 Whether to abstract the agent behind an interface from the start (the config has only Codex profiles today).
- R5.5 Failure and cost handling: missing CLI, expired login, timeouts.

### R6. Export targets (B)

- R6.1 iCloud export: reusable code in the old project, target layout, handling of not-yet-downloaded iCloud placeholder files, overwrite and conflict rules.
- R6.2 Notion API: PAT/internal-integration auth, page creation under `root_page_id`, block and rate limits, long entries, and updating vs. re-creating pages (idempotent export).
- R6.3 Page structure: one page per what (month? year?) with entries ascending by date.
- R6.4 Secret handling: Notion token in a YAML file vs. Keychain; keeping `config/global_config.yaml` out of git.

### R7. Frontend (B)

- R7.1 CodeMirror 6 in React: markdown mode, a dedicated editor module, multiple tabs with separate state, dirty tracking.
- R7.2 VS Code-like layout: toolbar strip, icon sidebar, explorer, tabbed editor, AI chat; resizable panes; candidate libraries and their licenses.
- R7.3 Command palette (Cmd+Shift+P) and keyboard shortcut handling in Tauri.
- R7.4 Calendar component (Date tab) build vs. library; accessibility.
- R7.5 Explorer tree with checkboxes (Snippet tab) and tab content patterns; state management choice.
- R7.6 UI test approach with Vitest for the above.
- R7.7 Alignment of the ideas with [ux-gui.md](../ux-gui.md), [ux-terms.md](../ux-terms.md), and [ux-information-architecture.md](../ux-information-architecture.md); list conflicts for the owner.

### R8. Date and time (B)

- R8.1 Local-time rules: converting UTC-based imports to local dates, DST edge cases, and timezone storage (IANA name vs. offset).
- R8.2 Rust (`chrono`/`chrono-tz`, `jiff`) and TypeScript (`Intl`, `Temporal` support in WKWebView) options.

### R9. Config (B)

- R9.1 Config file location (`config/global_config.yaml` in the project root) vs. a user-level location for an installed app.
- R9.2 YAML parsing and schema validation, defaults, error reporting, and reload on change.
- R9.3 Meaning and use of `server.port_fallback_count` (ports 8771/8772) in a Tauri app with no server; likely a leftover from the old client-server design.
- R9.4 Meaning of source `type` values (`ayumi`, `narihito`) and how new types are added.

### R10. Icons and licensing (C)

- R10.1 SF Symbols license terms for rendering to bitmap and bundling them in an app; export workflow and sizes.
- R10.2 Fallback if the terms do not allow it.
- R10.3 License check process for dependencies (Apache-2.0 compatibility) and tooling such as `cargo-deny`.

### R11. Versioning (C)

- R11.1 Keep "Version v1.0.0" display format and the places that carry the version (`Cargo.toml`, `package.json`, `tauri.conf.json`, UI).

## Open questions for the owner

- The ideas file does not say if import should modify or delete source files (assumed read-only).
- Should snippets already imported be hidden, or shown again, on later imports?
- Is audio, `.assets`, or location/weather ever needed after v1.0.0?
- Are `server.*` and any `agents` types beyond Codex needed in v1.0.0?

## Completion checklist

- [ ] Every research item R1 to R11 has findings written, or is marked deferred with a reason.
- [ ] Findings are linked from this doc and indexed in [docs/README.md](../README.md).
- [ ] Each item that affects P1-M2 has a one-line "implication for architecture".
- [ ] Open questions are answered by the owner or moved to "Undecided" in [AGENTS.md](../../AGENTS.md).
- [ ] Contradictions with existing docs are listed in [changelog.md](changelog.md).
- [ ] [phase-1.md](phase-1.md) status for P1-M1 updated.
