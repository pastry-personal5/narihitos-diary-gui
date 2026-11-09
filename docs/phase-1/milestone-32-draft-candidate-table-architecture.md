# P1-M32: Draft tab candidate table work plan

Status: Active

Implement the owner's four-column Draft candidate table, authorized on 2026-11-09, with the Snippet table's look and feel. The [overview](milestone-32-draft-candidate-table-overview.md) owns behavior and acceptance; this file records implementation steps and review outcomes.

## Boundaries and sequence

1. Inspect `src/components/explorer/Explorer.tsx`, the generation contracts/controller and navigation resource owner. Preserve stable candidate identity via the existing job/slot key; display row numbers separately from that key. Keep the existing row order and number the visible rows from one across retained jobs.
2. Resolve each row's Style label and Style description from its actual generation style and factory prompt semantics. Inspect what style metadata is already available before adding anything; do not infer style from the visible row number, which can span jobs. Current contracts omit a separate style field: factory generation assigns `Style::ALL` deterministically using the job-local slot index. Resolve that existing style in an application presentation helper, with tests checking all four bundled prompts; no transport change is needed. Keep the mapping consistent with all four bundled factory prompts without modifying prompt text.
3. Replace only the candidate list with a semantic table. Reuse established table styling and semantic theme tokens. Keep Number and bitmap action columns compact and allow Style label to wrap; support horizontal scrolling when necessary in a narrow Explorer.
4. Use the project's custom bitmap control approach for Info and Open in Tabbed Editor. Inspect existing assets first; create appropriate bitmap assets during implementation if missing. Use actual buttons with tooltips, accessible names identifying the candidate and visible keyboard focus. The user explicitly requires bitmap controls; no icon-library dependency is proposed.
5. Show the matching description in a dismissible overlay using existing overlay patterns. Heading: Style description. Support keyboard dismissal and return focus to the invoking Info button. Reading style information does not open or activate an editor.
6. Route Open in Tabbed Editor through the existing `model.resources.openCandidate` operation. Verify that it focuses an already-open candidate and reopens a closed candidate without duplicating its resource. Preserve current availability rules for pending, failed, cancelled or discarded results and explain any disabled action.
7. Keep generation's existing automatic candidate-tab creation and streaming behavior. The new explicit open button handles focus/reopening. Preserve Authentic access, dirty-buffer protections, off-date results, stale-baseline notices, review and result discard.

## Verification plan

Delegate command execution and first-pass failure analysis to the `test_runner` agent under the repository working agreement. During implementation, extend focused generation UI/navigation checks for the four headers, multiple-job row numbering, actual style matching, Info overlay content/focus, existing-tab focus and closed-tab reopening. Cover unavailable and discarded rows and absence of duplicate tabs. Verify the bitmap controls and reachable columns visually in a narrow Explorer. Run the full `scripts/gate.sh` after relevant checks pass.

Review Markdown links, scope, checklist consistency and whitespace alongside implementation checks. Update maintained GUI behavior documentation while keeping the existing factory generation behavior intact. This task does not discharge the separate M32 release qualification obligations.

## Implementation outcome (2026-11-09)

The owner requested matching the Snippet tab table. `CandidateTable` directly reuses its table/action CSS classes and existing app-owned bitmap assets. `application/generation/candidates.ts` owns style resolution, shared candidate text and reopening policy; the tab synchronization hook uses the same text resolver. Existing transport contracts and prompt text are unchanged.

The test runner passed the five focused regressions and all seven gate lanes. Narrow-layout visual inspection could not run because no browser/GUI runner is available; see the [remaining checklist item](milestone-32-draft-candidate-table-overview.md#completion-checklist). No new dependency was introduced.

## Review follow-up (2026-11-09)

The tab synchronization hook tracks successfully mirrored candidate keys. New slots still open automatically, but subsequent output never recreates a deliberately closed tab; explicitly reopened tabs continue receiving updates. Discarded keys are removed from the bookkeeping. The Open use case checks composition before resource creation.

The Style description modal uses a document-body portal so the shared modal's inert-background logic applies to the whole application. Date-keyed table mounts reset old dialog selection. Availability descriptions are associated with each Open control for assistive technology, and both Explorer tables share the bitmap renderer. Regressions cover streaming/completion after close, explicit reopen, close-all/discard, composition guards, background inertness, focus trapping and discard cleanup.

Follow-up verification passed: 15 focused candidate-table/generation UI tests, TypeScript, ESLint, Prettier and all seven gate lanes, run by the delegated test runner. The shared bitmap extraction also passed the complete frontend test suite in the gate; no dedicated Snippet table test file exists. Narrow-Explorer visual qualification remains pending.
