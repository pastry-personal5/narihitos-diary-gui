# P1-M1: Research overview

Status: Done
Goal: Produce evidence and explicit open questions for architecture decisions in P1-M2. Documentation research covers R1–R14; verification gaps and owner questions remain explicit blockers on dependent decisions.

Sources: owner's initial ideas (`tmp/initial-owner-ideas.md`, untracked) and the owner notes recorded in [R5 findings](../research/agent-assisted-diary-composition.md). The working baseline below preserves the relevant intent for readers without the local ideas file. Exempt from the code gate; documentation review still applies; see [development-process.md](../development-process.md#definition-of-done).

## Scope

In: investigating the items below and writing findings to `docs/` (one findings doc per group, or one doc if short).
Out: architecture decisions (P1-M2), code, and scaffolding (P1-M3). Start with source inspection and documentation. If a critical claim needs execution, record a bounded spike proposal with its question, synthetic inputs, expected evidence, and destination milestone. Do not defer an architecture-blocking feasibility check to P1-M3 without explicitly blocking the dependent P1-M2 decision.

## Working baseline and decision boundaries

- The fixed stack is Tauri 2, Rust, React/TypeScript, and CodeMirror 6. Submodules are reference material, never runtime or build dependencies.
- The initial ideas describe Diary assets only for v1, markdown-only snippet import, optional iCloud/Notion export, a Markdown source editor, and local dates throughout the GUI. Financial Record support and Markdown preview are outside this research's implementation scope.
- The intended flow is local date selection → import snippets for D-1/D/D+1 → select snippets → generate candidates → edit the Authentic file → optionally export. The owner clarified that Authentic draft and Authentic Entry are one autosaved file; candidates are transient. Do not introduce a separate draft-to-Entry promotion or separate persistence copies.
- Owner clarification: source edit/save is explicit; source snippet plus related audio deletion is an explicit, confirmed action in the Explorer tree. Markdown-only import does not remove the need to validate related-audio ownership during deletion. Import captures the system timezone; saved Entry dates stay fixed.
- R5 records an owner requirement for Codex, Claude Code, and Qwen Code profiles, selected per chat session and per generation. Its proposed configuration and one-shot adapter approach remain research recommendations until P1-M2.
- [product-behavior.md](../product-behavior.md), [ux-gui.md](../ux-gui.md), [ux-terms.md](../ux-terms.md), and [ux-information-architecture.md](../ux-information-architecture.md) currently contain `TBD` placeholders. They cannot yet settle product or interaction questions. R12 must identify the owner confirmations needed to make them useful; this review does not fill them with invented behavior.
- The owner confirmed Phase 1 delivers v1.0.0; see the updated [phase plan](phase-1.md). Research → Architecture → Scaffolding remain first, followed by feature/release milestones to be planned.

## Review completion checklist (this plan revision)

- [x] Reconcile existing findings and owner notes with the item list and status (working baseline, R5, and findings register).
- [x] Identify missing research, dependencies, and unresolved product decisions without deciding them (expanded subitems, R12–R14, and owner questions).
- [x] Define evidence requirements and a verifiable P1-M2 handoff (research sequence and milestone completion checklist).
- [x] Restore roadmap/process links and check changed documentation for broken local links and whitespace errors.

Review validation (2026-11-05): 53 local Markdown links/anchors checked, excluding code examples; 14 research groups and 66 unique sequential subitems checked; `git diff --check` passed. The code gate was not run under the docs-only exemption. These checks complete the plan review, not P1-M1 research.

## Research items

Priority: **A** blocks the architecture, **B** shapes it, **C** can wait for later milestones.

Priority controls sequencing, not permission to skip an unanswered dependency. An item can be deferred only when its consumer and the consequence of deferral are documented. License clearance is required before copying code or bundling assets regardless of its group's priority.

### R1. Source formats (A)

- R1.1 Ayumi snippet format: `tmp/Ayumi/YYYY/MM/DD/<timestamp>.md` front matter fields (tags, weather, location, transcription) and the `.assets` folder.
- R1.2 Narihito snippet format: `tmp/NarihitosDiary/YYYY/MM/DD/<timestamp>-<uuid>.md` front matter (`schema_version`, `kind`, `id`, language, audio, transcription fields) and `.assets`.
- R1.3 Which fields the GUI must keep, show, or ignore; how markdown-only import treats `.assets` and audio files.
- R1.4 UTC directory structure vs. local date: do directories use UTC dates, which timestamp is authoritative (`created_at` has an offset in Ayumi, `Z` in Narihito), and the rule for picking snippets for local dates D-1, D, D+1.
- R1.5 Snippet identity and de-duplication across repeated imports.
- R1.6 Malformed or unsupported front matter, unknown schema versions, invalid timestamps, encoding, filename/front-matter disagreement, changed content under an existing ID, and identical IDs across sources. Define synthetic fixtures and candidate handling policies; never use private diary content in committed fixtures.

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
- R3.8 Durability and recovery: atomic file replacement on macOS, interruption between asset/history/index writes, corrupt or missing indexes, external edits, and concurrent app instances. Distinguish user-owned files from rebuildable state; compare options against the preservation requirements from R12.

### R4. Tauri 2 on macOS (A)

- R4.1 Reading files outside the app: iCloud Drive paths, TCC prompts, sandboxed vs. unsandboxed builds, and the Tauri capability/permission set needed (least privilege).
- R4.2 Command and event model for backend features, including long-running work and progress (import, export, agent streaming).
- R4.3 Spawning and streaming external processes (AI agents) from Rust.
- R4.4 Dev/build flow on macOS Tahoe 26.7.1+ with Xcode CLT; whether the "generated Xcode project" mention in the ideas applies to a desktop Tauri app at all.
- R4.5 WKWebView limits relevant to the editor and layout.
- R4.6 Launching a packaged GUI from Finder: executable discovery without a shell startup file, configurable CLI paths, child process groups and descendant cleanup, bounded stdout/stderr buffering, and event ordering/backpressure. Cross-reference R5 and R13 before recommending a process runner.

### R5. AI agents (A)

- R5.1 Codex, Claude Code, and Qwen Code: per-profile homes (`CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `QWEN_HOME`), headless/streaming interfaces, authentication, supported versions, and capability differences. Reconcile the initial `codex_profiles` shape with the `agents.profiles[]` proposal in the existing findings.
- R5.2 Chat session design: passing the Tabbed Editor content as context, session lifecycle, cancellation, history storage.
- R5.3 Generating several draft candidates from selected snippets: one call or several, prompt inputs, output parsing.
- R5.4 Compare adapter interfaces for the recorded three-agent, per-use selection requirement. Separate common capabilities from provider-specific ones and identify compatibility checks; the interface itself is decided in P1-M2.
- R5.5 Failure and cost handling: missing CLI, expired login, timeouts.

Existing evidence: [R5 findings](../research/agent-assisted-diary-composition.md) covers R5.1–R5.5, with recommendations and unverified claims. No live model turn was run. Follow up on tools/MCP disablement, session persistence, auth probes that might incur cost, schema-constrained output, and cancellation of descendants. Cross-reference R13; a read-only sandbox or empty working directory alone does not establish confidentiality. Keep this item partial until the evidence gaps are verified or explicitly block the dependent decision.

### R6. Export targets (B)

- R6.1 iCloud export: reusable code in the old project, target layout, handling of not-yet-downloaded iCloud placeholder files, overwrite and conflict rules.
- R6.2 Notion API: PAT/internal-integration auth, page creation under `root_page_id`, block and rate limits, long entries, and updating vs. re-creating pages (idempotent export).
- R6.3 Page structure: one page per what (month? year?) with entries ascending by date.
- R6.4 Secret handling: Notion token in a YAML file vs. Keychain; keeping `config/global_config.yaml` out of git.
- R6.5 Export recovery: interrupted or partially successful exports, retries after an ambiguous response, remote edits/deletions, and stable local-to-remote identity. Identify how ascending order is preserved when backdated entries are added or edited. Compare supported Notion API versions and required integration permissions; record the researched version rather than assuming "PAT" is the correct credential type.

### R7. Frontend (B)

- R7.1 CodeMirror 6 in React: markdown mode, a dedicated editor module, multiple tabs with separate state, dirty tracking.
- R7.2 VS Code-like layout: toolbar strip, icon sidebar, explorer, tabbed editor, AI chat; resizable panes; candidate libraries and their licenses.
- R7.3 Command palette (Cmd+Shift+P) and keyboard shortcut handling in Tauri.
- R7.4 Calendar component (Date tab) build vs. library; accessibility.
- R7.5 Explorer tree with checkboxes (Snippet tab) and tab content patterns; state management choice.
- R7.6 UI test approach with Vitest for the above.
- R7.7 Alignment of the ideas with [ux-gui.md](../ux-gui.md), [ux-terms.md](../ux-terms.md), and [ux-information-architecture.md](../ux-information-architecture.md); list conflicts for the owner.
- R7.8 Keyboard-only navigation, focus restoration, VoiceOver, IME composition, and large snippet trees/long documents. Record measurable scenarios for P1-M3 and feature milestones rather than choosing arbitrary performance thresholds here.

### R8. Date and time (A)

- R8.1 Local-time rules: converting UTC-based imports to local dates, DST edge cases, and timezone storage (IANA name vs. offset).
- R8.2 Rust (`chrono`/`chrono-tz`, `jiff`) and TypeScript (`Intl`, `Temporal` support in WKWebView) options.
- R8.3 Distinguish a diary's selected civil date from a snippet's instant, import time, and Entry save time. Determine which timezone governs re-import after travel or a system-zone change, and whether saved Entry dates remain stable. Test local midnights, year boundaries, UTC date crossings, and 23/25-hour DST days; D-1/D/D+1 are calendar dates, not fixed 24-hour durations.

### R9. Config (B)

- R9.1 Config file location (`config/global_config.yaml` in the project root) vs. a user-level location for an installed app.
- R9.2 YAML parsing and schema validation, defaults, error reporting, and reload on change.
- R9.3 Meaning and use of `server.port_fallback_count` (ports 8771/8772) in a Tauri app with no server; likely a leftover from the old client-server design.
- R9.4 Meaning of source `type` values (`ayumi`, `narihito`) and how new types are added.
- R9.5 Schema version/migration, relative path resolution, overlapping source/storage/export roots, duplicate aliases, invalid or missing directories, and redacted diagnostics. Reconcile R5's profile proposal without reading or committing local secrets.

### R10. Icons and licensing (B)

- R10.1 SF Symbols license terms for rendering to bitmap and bundling them in an app; export workflow and sizes.
- R10.2 Fallback if the terms do not allow it.
- R10.3 License check process for dependencies (Apache-2.0 compatibility) and tooling such as `cargo-deny`.

### R11. Versioning (B)

- R11.1 Keep "Version v1.0.0" display format and the places that carry the version (`Cargo.toml`, `package.json`, `tauri.conf.json`, UI).
- R11.2 Compare a version source of truth and consistency checks for P1-M3; do not select new build tooling during research.

### R12. Asset lifecycle and unresolved product contracts (A)

- R12.1 Turn the working flow into a proposed state/transition table: imported Snippet, candidate, Authentic draft, and Entry. Identify creation, replacement, editing, autosave/history, and re-opening rules for the single Authentic file shown as draft/Entry; ask whether multiple Authentic files can share a date.
- R12.2 Use the owner-confirmed single-file Authentic autosave model; identify owner choices for pending saves on close/date change, candidate adoption (replace vs. merge), and regeneration while Authentic content has edits. Define what must survive cancellation, app exit, or a crash; do not invent deletion or retention policies.
- R12.3 Record what history/diff/restore means for each persistent asset and whether restore creates a new history event. Feed preservation requirements into R3's storage comparison.
- R12.4 Produce a gap/conflict register against the product/UX placeholders and the ideas. Obtain owner confirmation for architecture-blocking behavior before P1-M2 builds on it; reflect the owner-confirmed Phase 1 v1.0.0 scope while preserving foundation milestone order.

### R13. Trust boundaries and diary privacy (A)

- R13.1 Map data flows among the WebView, Rust commands, source folders, managed storage/history, CLI profiles/processes, provider endpoints, and export services. Identify which diary text/metadata/secrets leave the device and which locations retain transcripts, temp files, logs, or credentials.
- R13.2 Compare confinement guarantees for all three agents, including inherited config, hooks, plugins/MCP, filesystem tools, symlinks, and network access. Prompt delimiters are not access controls. Document fail-closed behavior when the requested tool restrictions cannot be enforced, and any residual risk requiring an owner decision.
- R13.3 Define safe handling of imported Markdown/front matter as untrusted data: size limits, parsing, path traversal, links/embedded assets, and prompt injection. Backend paths and Tauri commands must enforce boundaries independently of frontend validation.
- R13.4 Propose bounded synthetic checks for attempted out-of-directory reads/writes, unexpected tool calls, cancellation, and persistence cleanup. Record vendor/account-policy questions separately from technical evidence; do not send real diary data or make paid model calls as part of plan review.

### R14. Validation and macOS delivery feasibility (B)

- R14.1 Map risk to test layers: parser/date fixtures (R1/R8), recovery and repeat-import/export checks (R3/R6), fake CLI protocol/error streams (R5), and editor/state tests (R7/R12). Identify which failures need a real packaged macOS smoke test instead of a Vitest mock.
- R14.2 Research a reproducible macOS validation path for Tauri command permissions, Finder launch, WKWebView/IME/keyboard behavior, and optional/missing CLIs. List prerequisites and limitations before proposing any additional dependency or build tool.
- R14.3 Separate local development, personal installation, and public distribution requirements: minimum macOS/CPU targets, signing/notarization, entitlements, and agent binary availability. Verify platform/version claims in the ideas rather than treating them as established support requirements; leave release scope to the owner.

## Research sequence and evidence

1. Inventory available source samples/prior art (R1/R2), the product gaps (R12), and existing R5 evidence. Record unavailable local inputs as blockers; `tmp/` material must not be a prerequisite for understanding the final findings.
2. Establish date semantics (R8), preservation needs (R12), and trust boundaries (R13), alongside macOS feasibility (R4). These constrain storage (R3) and agent integration (R5).
3. Use those constraints to compare export, frontend, and config options (R6/R7/R9). Review licensing before reuse (R2/R10), then versioning and validation/delivery (R11/R14).
4. Summarize recommendations, alternatives, unanswered owner questions, and blocked decisions for P1-M2. Do not choose dependencies or silently promote a research recommendation to an approved decision.

Each findings document must identify covered subitem IDs, research date and inspected versions/commits, primary-source links or local code paths, observed behavior vs. inference, alternatives/tradeoffs, an implication for P1-M2, and remaining questions. For a local check, record the command/setup, synthetic input, expected vs. observed outcome, and limits of the evidence. Do not copy private samples, absolute personal paths, tokens, or raw diary output into findings.

### Findings register

| Items                             | Evidence                                                                                | Remaining work                                                                                                                        |
| --------------------------------- | --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| R1.1–R1.6, R2.1–R2.4              | [Sources/prior art](../research/source-formats-and-prior-art.md)                        | Producer coverage limits and import/revision policy remain explicit. Source-write interoperability needs verification.                |
| R3.1–R3.8, R12.1–R12.4            | [Storage/lifecycle](../research/storage-and-asset-lifecycle.md)                         | Compare engines and choose identity/recovery contracts in P1-M2; pending-save/adoption semantics need owner decisions.                |
| R4.1–R4.6, R14.1–R14.3            | [Tauri/macOS/validation](../research/tauri-macos-and-validation.md), partial            | Packaged access/process feasibility and distribution requirements remain unverified.                                                  |
| R5.1–R5.5                         | [Agent findings](../research/agent-assisted-diary-composition.md), partial              | Live schema/confinement/persistence evidence still missing; dependent claims blocked.                                                 |
| R6.1–R6.5, R9.1–R9.5, R11.1–R11.2 | [Export/config/versioning](../research/export-config-and-versioning.md)                 | Export hierarchy/config discovery/secret strategy need decisions; real service/platform behavior not exercised.                       |
| R7.1–R7.8, R10.1–R10.3            | [Frontend/licensing](../research/frontend-and-licensing.md), partial                    | Inspected SF Symbols license does not clear bitmap bundling; canonical UX docs and packaged accessibility evidence remain incomplete. |
| R8.1–R8.3                         | [Date/time](../research/date-and-time.md)                                               | Capture import zone and preserve Entry date confirmed; choose library and ambiguity policy in P1-M2.                                  |
| R13.1–R13.4                       | [Trust/source mutations](../research/trust-boundaries-and-source-mutations.md), partial | Verify effective tool/config restrictions and source-pair operations before dependent decisions/features.                             |

Split rows as findings arrive. A deferral must name its subitem IDs, reason, destination milestone, and consequence for P1-M2. Do not mark a partial item complete merely because a findings document exists.

## Open questions for the owner

These are pending decisions, not assumptions authorizing implementation. Carry unresolved architecture blockers to "Undecided" in [AGENTS.md](../../AGENTS.md) before the P1-M2 handoff.

- R1/R12: Explicit source edit/save and confirmed snippet-plus-related-audio deletion are approved. What happens when an imported snippet changes or reappears, and should it remain visible on later imports?
- R1: Markdown-only import is stated in the ideas. Should embedded asset links and audio/location/weather front matter be preserved, displayed, or omitted? Future audio support need not be decided to finish v1 research.
- R8: Import captures the system timezone and saved Entry dates remain fixed, per owner decision. Define timestamp disagreement and skipped/ambiguous local-date handling in P1-M2.
- R12: Authentic draft and Entry are one autosaved file, per owner clarification. What happens during pending/failed autosaves, and can more than one Authentic file exist for a date?
- R3/R5/R13: Should chat transcripts persist, and what history/retention/restore behavior is expected for each asset? Is provider transmission of selected diary/editor content acceptable under each configured account's policy?
- R5: The three-agent per-use selection requirement is recorded. Must missing agents be tolerated, may one generation fan out to multiple agents, and what candidate count/style choices are wanted?
- R9: Is `server.*` required, and how should the installed app locate the config while preserving the initial project-root workflow?
- R14: Phase 1 delivers v1.0.0, per owner clarification. What installation/distribution targets are intended?

## P1-M2 handoff

Owner decisions: Phase 1 delivers v1.0.0; Authentic draft/Entry are one autosaved file; source edit/save is explicit and snippet-plus-related-audio deletion requires an explicit Explorer action and confirmation; import captures the system timezone and preserves saved Entry dates. See [product behavior](../product-behavior.md) and [changelog](changelog.md).

Recommended decision order:

1. Resolve asset/source identity, re-import conflicts, candidate adoption, and autosave/history semantics. Record the actual UX contracts rather than building from the placeholder docs.
2. Settle distribution/access feasibility and CLI auth/confinement. The bounded checks in [Tauri findings](../research/tauri-macos-and-validation.md) and [trust findings](../research/trust-boundaries-and-source-mutations.md) block claims about those capabilities until executed; reading docs did not verify runtime enforcement.
3. Select file schema/layout, timezone library, history engine, catalog/durable-state boundaries, source-mutation recovery, and typed Tauri command/channel contracts.
4. Select installed config discovery/secret handling, Notion hierarchy/reconciliation, minimal frontend dependencies, and version metadata. Obtain dependency/tool approval before changing the build.
5. Define P1-M3 scaffold evidence and add later feature/release milestones for v1.0.0. The inspected SF Symbols terms leave bitmap bundling uncleared, which blocks that asset route, not a text-labeled scaffold. Native/real-service tests belong in the corresponding implementation acceptance criteria; do not claim them passed from this documentation research.

All fourteen groups have written findings; this is not a claim that every feasibility question is settled. No numbered subitem has silently been deferred. Remaining owner choices are in [AGENTS.md](../../AGENTS.md#undecided--ask-before-inventing); remaining experimental gaps are explicit in the findings register. P1-M1 closes as documentation research with those blockers retained; closure does not certify unexecuted feasibility checks or authorize the blocked implementation.

## Milestone completion checklist

- [x] Every numbered subitem under R1–R14 is covered by findings or an explicit nonblocking deferral in the findings register.
- [x] Findings are linked from this doc and indexed in [docs/README.md](../README.md).
- [x] Findings identify sources, versions/dates, alternatives, and verification limits; recommendations are distinguishable from decisions.
- [x] Each item that affects P1-M2 has an "implication for architecture" and identifies its dependent decision.
- [x] Architecture-blocking feasibility gaps are verified, or explicitly listed as blockers preventing the dependent P1-M2 decision; any proposed spike has a question, bounded scope, and destination milestone.
- [x] Open questions are answered with owner evidence or moved to "Undecided" in [AGENTS.md](../../AGENTS.md); blocking questions remain visibly blocking even if this research milestone closes.
- [x] Product/UX placeholder gaps and contradictions are listed with their disposition in [changelog.md](changelog.md); no unapproved behavior is treated as settled.
- [x] A P1-M2 handoff lists recommendations, deferred subitems with destinations, unresolved blockers, and the order in which decisions can safely proceed.
- [x] Documentation links and whitespace are checked; the recorded docs-only gate exemption is preserved without claiming the code gate passed.
- [x] [phase-1.md](phase-1.md) status for P1-M1 updated.

Acceptance evidence (2026-11-05): all 66 subitems are mapped in the findings register; eight findings documents are indexed, with explicit implications, alternatives, and limits. Owner clarifications are recorded in product behavior/changelog; unresolved decisions are carried in AGENTS.md. Local-link/anchor, subitem-coverage, and whitespace checks passed. Synthetic sample/date checks and CLI/toolchain inspection are documented in the findings. No code gate, live inference, service write, source mutation, or app runtime test was run; the docs-only exemption and blocked capability claims remain explicit.
