# Product behavior

Status: Active

Product scope: what the app does and what it deliberately does not do.

## Owner-confirmed behavior

- Imports capture the system timezone; saved Entry dates remain fixed.
- Authentic draft and Authentic Entry are one autosaved file. AI draft candidates are transient suggestions, not additional saved Authentic files.
- Users can explicitly edit and save a source snippet. They can explicitly delete a source snippet and its related audio files from the Explorer tree, with confirmation of deletion.
- Optional agent types are Codex, Claude Code, Qwen Code and Apple Foundation Models. The Apple agent uses the on-device system model when available; an unavailable agent leaves editing usable and never silently switches providers. The [agent contract](architecture/editing-and-agents.md#apple-foundation-models-adapter) records the native integration and evidence still required.
- AI writing language is chosen in Settings, initially uses the Mac's preferred language, and is remembered across sessions. Generate requires at least one selected eligible source snippet; changing dates lets generation finish in the background for its original date. **Create Empty** immediately saves an empty Authentic draft for the selected date without requiring snippets, and it remains visible after restart. See the [architecture review interview](phase-1/milestone-02-architecture-review.md#owner-interview).
- Long AI work keeps ordinary editing/navigation available, with visible progress, cancellation and in-app completion feedback; no macOS notifications are used. If the saved Authentic changes during generation, its candidates require regeneration before review. See the [background AI experience](architecture/ai-job-experience.md).

## P1-M2 owner-confirmed contracts

The [owner interview](phase-1/milestone-02-owner-decisions.md) is the detailed decision record; its D01–D17 identifiers provide provenance.

- V1 uses separately shell-launched local server/client processes, one active client, and one workspace. Closing the client leaves the server running (D01–D04).
- One Authentic file exists per diary date. History is deferred beyond v1. Candidate suggestions are reviewed per paragraph with manual editing paused; accepted edits autosave, pending suggestions remain transient (D05–D07).
- Date changes and close wait for saves. Save failure retains the editor and offers retry or explicit discard (D06).
- Imports are views of sources with path-based identity. Refresh protects unsaved edits. Preserve metadata, flag timestamp disagreements, and exclude unresolved conflicts from generation. Guarded explicit source saves support verified Narihito schema 1 recordings and schema 2 manual text-only snippets; manual snippets require no audio/transcription metadata and retain their title/provenance. Other formats require producer compatibility; confirmed deletion uses Trash for the snippet and validated related audio, with partial-failure retry (D08–D10). Text-only Trash remains disabled.
- The Draft tab presents candidates in a table matching the Snippet tab: Number, Style label, bitmap Info showing Style description, and bitmap Open in Tabbed Editor focusing or reopening the candidate. See [GUI behavior](ux-gui.md).
- Chat is memory-only. Generation uses one selected profile, three sequential candidates by default, and an adjustable count. Missing agents are tolerated. The owner accepts the installed CLI's normal access/configuration and account policy; app transcript deletion does not promise CLI/provider deletion (D11–D12).
- Private YAML contains the Notion credential. Optional exports use saved Authentic revisions; Notion uses year/month pages and ascending daily sections. External edits require confirmation before replacement (D13–D14).

The [software architecture](architecture.md) specifies internal contracts and conditional capabilities; [UX GUI](ux-gui.md) specifies their visible interactions. These are planned product behaviors, not claims that an app implementation exists. Research recommendations do not establish additional behavior. See the [Phase 1 changelog](phase-1/changelog.md) for earlier clarifications.
