# P1-M2: Architecture work plan

Status: Archived

This completed work plan produced [the maintained architecture](../../../architecture.md) and themed contracts. It applies the [owner decisions](../../../phase-1/milestone-02-owner-decisions.md); the [milestone checklist](milestone-02-architecture-overview.md#completion-checklist) records final evidence. The remaining text preserves the original design tasks for audit and is no longer maintained.

## Completed owner clarifications and review

Owner-requested organization correction (2026-11-05): the following completed design work belongs to P1-M2 and its [completion record](milestone-02-architecture-overview.md#architecture-clarifications-and-review), rather than P1-M3. This records milestone ownership without reopening implementation.

- Configuration and process contracts: absolute workspace/runtime paths, CLI `profile_home` and optional executable resolution, runtime discovery and port fallback.
- Agent/prompt contracts: optional Apple Foundation Models through the shared port, external prompt assets, validation/packaging and explicit native evidence limits.
- [Architecture review and interview](../../../phase-1/milestone-02-architecture-review.md): twelve contract corrections, seven owner answers, durable empty-file creation and updated acceptance mapping.
- [Background AI experience](../../../architecture/ai-job-experience.md), logging, shortcuts and user settings: interaction/state/transport ownership, bounds and required evidence. P1-M3 assembles the skeleton from these contracts; later feature milestones implement their full behavior according to the handoff.

## Design approach and deliverables

Write one maintained overall architecture document. Use links to research evidence and the interview rather than copying their inventories. Identify owner requirements, engineering decisions made under D17, and conditional capabilities separately. Explain significant tradeoffs and reject approaches that conflict with owner choices.

The following internal approaches are **planning proposals**, not additional owner interview decisions: keep the API credential in a narrow Tauri Rust bridge; publish a private runtime discovery record; use revision preconditions and idempotent operation identifiers; layer domain/application code beneath transport and infrastructure adapters. Finalize their details and rationale in the architecture document.

## Processes, modules, and interfaces

- Draw separate client/server deployment and trust boundaries. Define shell launch contracts, config selection, endpoint discovery, token lifecycle, origin validation, one-client admission, disconnect/reconnect, and server shutdown. Reconcile the existing `server.port_fallback_count` example with the selected server model rather than treating it as obsolete.
- Define domain models with explicit asset types and Diary-specific kinds, civil dates, instants, revisions, and identities. Keep future financial asset types outside v1 implementation.
- Separate domain rules, application services, infrastructure adapters, and HTTP/SSE transport. Future MCP/web access must reuse application services; serving MCP is distinct from a selected CLI consuming its configured MCP tools.
- Define operation families for config/status, client sessions, date/import/source reads, explicit source save/delete, Authentic load/autosave/review, agent profiles/chat/generation/cancel, exports, and job status. Use backend-resolved identities and typed validation/errors rather than arbitrary frontend paths or shell commands.
- Specify request/revision preconditions, conflict responses, cancellation ownership, event ordering, bounded buffering, and reconnect/status recovery. React accesses backend features through explicit Tauri commands with minimum permissions; choose the concrete HTTP/SSE bridge design and command allowlist.

## Storage and recovery

- Define the single workspace layout, versioned Markdown schema, date-to-file mapping, private SQLite layout/schema responsibilities, and durable versus rebuildable records. Source views do not imply durable managed snippet copies. No Git repository or retained revision history is required for v1.
- Make path-based identity precise for configured source locations, relative paths, renames, duplicate paths, aliases, symlinks, and changes of source configuration. Do not reintroduce stable producer-ID identity under another name.
- Specify ordered autosave acknowledgement, stale-write detection, atomic publication/recovery, workspace/server locking, external edits, read freshness, and client loss. Distinguish an acknowledged save from an edit still in the client buffer; do not promise crash survival before durability.
- Define source adapter parsing bounds, preserved metadata/body editing, malformed/unsupported outcomes, timestamp-conflict resolution, and date filtering. Retain captured import timezone and fixed saved Entry dates; choose explicit behavior for skipped/ambiguous calendar boundaries.
- Specify guarded source writes and validated related-audio sets. Define Trash preview/confirmation freshness, partial results, retry/recovery, and unavailable cloud-file behavior. Require synthetic producer round trips before enabling source writes.
- Define durable intents/receipts for cross-file/SQLite and export work. Recovery journals support incomplete operations, not user-facing history or restoration of arbitrarily old text.

## Frontend and agent ownership

- Map the original GUI structure to components and canonical UX vocabulary. Define per-file CodeMirror state, one Authentic buffer across Draft/Entry views, source explicit-save state, command routing, and independent snippet focus/selection.
- Define paragraph review alignment, insertions/deletions and Markdown structures, accept/reject behavior, autosave boundaries, fixed baselines, and stale-review invalidation. Accepted edits survive; pending suggestions do not. Keep manual editing paused during review.
- Use runtime-selectable Codex/Claude/Qwen adapters with profile-specific status, process invocation, stream normalization, errors, and cancellation. Finalize candidate count limits/styles, timeouts, transcript/context bounds, structured output, and malformed-output behavior as documented engineering choices.
- Keep app chat and candidates transient. Document the owner's trusted-CLI policy without asserting tool confinement, suppression of customizations, or deletion of CLI/provider records. Keep unrelated app credentials out of child environments and logs.
- Specify child/descendant supervision, pipe draining, bounded output, client-close cancellation, disconnect ownership, and server shutdown. Keep editor/import usable when an optional agent is absent.
- Plan accessibility, keyboard/IME behavior, focus preservation, pane sizing, and SF Symbols integration. Record the chosen rendering route without claiming the interview grants third-party license rights.

## Config, export, and validation handoff

- Define explicit config-path handling, file-relative path resolution, YAML validation/schema evolution, source/workspace/export overlap guards, agent executable/profile fields, redacted diagnostics, and invalid reload behavior. Keep the literal Notion credential in private YAML, with no frontend/log disclosure.
- Define optional saved-revision exports: iCloud layout/availability checks and Notion year/month/daily-section identity, ascending insertion, pagination/chunking, retries, ambiguous results, and external-edit confirmation. Specify how an explicit confirmation is tied to the remote revision it approves.
- Choose a version source/consistency strategy and document candidate libraries and license checks. The selected SQLite architecture does not approve a particular crate, HTTP framework, parser, or new build tool. Obtain dependency/build approval before changing manifests.
- Map acceptance evidence to parser/date fixtures, recovery interruption tests, fake CLI/HTTP integration tests, frontend review/save tests, and real macOS/iCloud checks. Missing proof blocks the affected feature, and the architecture must not assert that it works merely because this documentation exists.
- Define the P1-M3 scaffold contract for separately launched processes, authenticated HTTP/SSE, an explicit Tauri bridge, and the full six-command gate. Review whether gate command paths must change for a separate server before editing tooling. Plan later source/edit/review/agent/export and personal-release milestones before their implementation.

## Completion review

Check every owner decision against the architecture and record any new owner changes in the changelog. Update canonical product/UX docs, AGENTS.md, the index, and phase/roadmap status together. Validate local links/anchors and whitespace; retain unchecked completion items until their evidence exists. Do not archive the active phase or start P1-M3 as part of this preparation change.
