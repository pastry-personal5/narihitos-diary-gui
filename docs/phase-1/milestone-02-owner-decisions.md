# P1-M2: Owner decisions

Status: Active

This records the owner's architecture interview answers on 2026-11-05. It supersedes conflicting research recommendations; it records intended behavior, not implementation or runtime verification.

## Deployment and boundaries

- **D01 — Separate processes:** Use client-server architecture. The end user launches the Rust server and Tauri client with different commands in a shell. V1 accepts connections from this Mac only. Closing the client leaves the server running, after waiting for saves and cancelling that client's agent runs.
- **D02 — API and access:** Use a versioned loopback HTTP API and server-sent events for progress. Require a local authentication token and validate browser origins. Allow one active client. Future MCP and web interfaces are extension ideas, outside v1 implementation scope.
- **D03 — Personal delivery:** Target personal use on the owner's Apple Silicon Mac, with shell launch. Public distribution, signing, notarization, and other CPU targets are deferred. The original machine/OS baseline is recorded in the research; this interview does not establish tested minimum versions.

## Storage and editing

- **D04 — Workspace:** Use one configured workspace. Interpret `paths.entries_root` as its root, including Authentic content and private application state. Source and export folders remain separate. Store Authentic text in Markdown and use local SQLite for a catalog, durable operation records, and export receipts. Distinguish rebuildable catalog data from authoritative records.
- **D05 — History deferred:** Defer history beyond v1, including Git integration, historical diff, and restore. Candidate-review diffs remain in scope. Recovery records are not a promise of user-facing version history.
- **D06 — Authentic and saves:** Maintain one autosaved Authentic file per diary date; Draft and Entry are views of that same file. On date change or client close, wait for pending saves. On failure, keep the editor available with retry and an explicit discard option. No separate durable unsaved-edit buffer was selected.
- **D07 — Candidate review:** Use code-review-style accept/reject **per paragraph** in the editor. Pause manual editing while reviewing a fixed saved revision; invalidate stale review on external changes. Autosave accepted edits only. Pending suggestions are transient and do not survive closing the app. Whole-file replacement and append-only adoption were not selected.

## Source snippets

- **D08 — Import and identity:** Imported snippets are views of original source files, not separately editable managed copies. Use path-based identity: moved sources or renamed snippet paths become new identities. Refresh changed files with a guard for unsaved edits; re-imported snippets remain visible.
- **D09 — Metadata and saving:** Preserve metadata while showing body text in the editor. Flag header/filename timestamp disagreements instead of guessing, and exclude unresolved conflicts from generation. Explicit source saves preserve unknown metadata and update producer-required fields only after compatibility tests pass. Unsupported write formats remain readable with saving disabled.
- **D10 — Deletion:** Explicitly confirmed deletion moves the snippet and validated related audio to macOS Trash. Report partial failures and offer retry. This is a product contract, not authorization to delete files during documentation work.

## Agents, configuration, and export

- **D11 — Agent behavior:** Keep Codex, Claude Code, and Qwen Code as runtime-selectable adapters; missing agents are optional. Select one profile per chat session or generation. Generate three candidates sequentially by default with an adjustable count, rather than a multi-profile fan-out. Discard app chat transcripts on close.
- **D12 — Trust installed CLI:** The owner selected trusting the installed CLI's normal access, configuration, and account policy, accepting access beyond explicitly selected text. This supersedes the research recommendation to require verified text-only confinement before enabling a profile. Do not claim that tools, hooks, MCP, provider retention, or CLI-owned transcripts are disabled. App chat retention and CLI/provider retention are separate.
- **D13 — Configuration:** The separately launched server takes an explicit config path (`--config PATH`); relative paths resolve against the config file's directory. Use `config/global_config.yaml` for the private configuration, including the literal Notion credential. Keychain and an environment-variable credential reference were not selected. Do not expose credentials to frontend state, logs, or unrelated child agents.
- **D14 — Export:** Export is optional and independent of local saving. Notion uses year/month pages under the configured root with daily sections sorted by diary date ascending. For Notion and iCloud, detect external edits since the last export and ask before replacing them; unchanged exports can update normally.

## GUI and design authority

- **D15 — GUI baseline:** Use the original VS Code-like layout: Main Toolbar Strip, left icon bar, Explorer with Date/Snippet/Draft/Entry tabs, Tabbed Editor, and AI Chat. Draft and Entry share the Authentic file. Detailed visual styling is separate from overall software architecture.
- **D16 — SF Symbols:** The owner requires SF Symbols and explicitly directed: “Anyway, use SF symbols. ignore any license.” Record the requested asset choice without claiming that the research cleared licensing. No assets are bundled in this docs-only task; the rendering/integration route remains architecture work. Apache-2.0 remains the project code license.
- **D17 — Internal design discretion:** The owner authorized engineering judgment for internal module boundaries, API details, recovery protocols, and document-level library recommendations. Installing packages or changing build tools still requires later approval. Such internal design choices must be identified as engineering decisions, not attributed to this interview.

## Related records

The [P1-M2 overview](milestone-02-architecture-overview.md) tracks unfinished work. The [architecture work plan](milestone-02-architecture-architecture.md) describes how to produce `docs/architecture.md`. Earlier source/time clarifications remain in [product behavior](../product-behavior.md) and the [changelog](changelog.md).
