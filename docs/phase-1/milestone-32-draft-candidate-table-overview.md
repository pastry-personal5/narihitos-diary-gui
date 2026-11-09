# P1-M32: Draft tab candidate table

Status: Active

Implement the owner's 2026-11-09 Draft tab revision as a task within P1-M32. The owner requested implementation and matching the Snippet tab table's look and feel. Release qualification remains separate. Technical sequencing is in the [work plan](milestone-32-draft-candidate-table-architecture.md).

## Scope and behavior

Replace the Draft tab's candidate list/tree with a table containing one row per candidate and these four columns, in order:

Reuse the Snippet tab table's typography, spacing, alternating rows and compact bitmap controls.

| Column                       | Content and behavior                                                                                      |
| ---------------------------- | --------------------------------------------------------------------------------------------------------- |
| Number                       | Sequential row numbers: 1, 2, 3, …                                                                        |
| Style label                  | The candidate's requested writing style, using the [canonical term](../ux-terms.md#candidate-style-terms) |
| Info button                  | Custom bitmap button; clicking shows that candidate's **Style description**                               |
| Open in Tabbed Editor button | Custom bitmap button; focuses the existing candidate tab or opens it when absent                          |

Info opens a dismissible overlay headed **Style description**, displaying the description for that row. Dismissal returns focus to its Info button. Opening a candidate focuses its read-only editor without creating a duplicate tab. Row clicks do not replace the explicit action buttons.

Keep Authentic access and existing generation, progress, review, discard and stale-baseline behavior. No prompt rewriting, new style selection, multiple-prompt management, dependency changes or changes to candidate retention are included.

## Current factory defaults

The factory default generates three candidates, in this deterministic order:

| Number | Style label | Style description from the bundled generation prompt                                                                       |
| ------ | ----------- | -------------------------------------------------------------------------------------------------------------------------- |
| 1      | standard    | a balanced first-person page that follows the day naturally.                                                               |
| 2      | concise     | short and plain; keep only what matters, in as few sentences as the facts allow (this overrides the paragraph size below). |
| 3      | reflective  | dwell on feelings and thoughts the facts imply, without adding events.                                                     |

Evidence: [style ordering](../../src-tauri/crates/diary-application/src/agent/packet.rs), [generation selection](../../src-tauri/crates/diary-application/src/generation.rs) and [factory prompt](../../assets/prompts/codex/generate-candidate.md). Claude, Qwen and Apple Foundation Models factory prompts use the same descriptions. Chronological and thematic occupy slots four and five when requested.

## Completion checklist

Automated implementation evidence is recorded below. Visual qualification remains pending.

- [x] Draft candidates use a semantic table with exactly the four specified columns and sequential row numbers, including when retained jobs supply several rows.
- [x] Each row displays the Style label belonging to that candidate's actual generation style.
- [x] Custom bitmap Info buttons show the matching Style description, with keyboard activation, dismissal and focus return.
- [x] Custom bitmap Open in Tabbed Editor buttons focus an existing tab or reopen a closed tab, without duplicates; unavailable candidates have an explained disabled state.
- [x] Both buttons have descriptive accessible names/tooltips and share Snippet table styles; a horizontal scroll container keeps narrow layouts reachable.
- [ ] Visually verify the table and bitmap controls in a narrow Explorer using a real browser/native GUI runner.
- [x] Existing streaming, empty/failed/cancelled states, date navigation, Authentic access, review and discard remain correct.
- [x] Focused UI/navigation regressions pass; maintained UX docs reflect the delivered behavior.
- [x] `scripts/gate.sh` passes; M32 release qualification obligations remain tracked separately.

## Review follow-up checklist (2026-11-09)

- [x] Closed candidate tabs stay closed through background streaming and completion updates until explicitly reopened.
- [x] Open during text composition has no resource, content or focus side effects.
- [x] Unavailable actions expose their reason to assistive technology, and style dialog dismissal/cleanup remains correct as results change.
- [x] Candidate and Snippet tables share the same bitmap rendering component as well as table styling.
- [x] Regression tests and the full gate pass after the fixes; visual qualification remains separately tracked above.

## Implementation evidence (2026-11-09)

- `CandidateTable` replaces the Draft list and directly reuses `snippet-table`, `snippet-actions` and `snippet-trash` styles, plus the existing bitmap Info and Entry assets. The shared modal supplies focus trapping, Escape dismissal and focus return.
- An application helper resolves factory styles using the job-local slot index, matching the backend's deterministic `Style::ALL` assignment. The visible row number is independent. All five style descriptions are checked against all four bundled factory prompts.
- Candidate text resolution is shared with the existing streamed-tab synchronization. Open rechecks retained results, focuses an existing tab or restores a closed tab from retained candidate/preview text. Failed, cancelled, pending and unread results have explained disabled controls.
- The delegated test runner passed all five new candidate-table tests, TypeScript, ESLint, Prettier and all seven `scripts/gate.sh` lanes. An initial sandbox run prevented loopback binds in Rust tests; rerunning with loopback networking enabled passed every lane.
- No browser/GUI runner is available in the environment. Narrow-Explorer visual inspection remains unchecked; no browser dependency was added. This task remains Active for that evidence, and the separate M32 packaged/native release qualification is still pending.

## Review findings and fixes (2026-11-09)

- Background streaming/completion used to recreate every closed candidate tab and could activate an editor after the user closed all tabs. Synchronization now creates each slot once and updates only tabs that remain open. Explicit Open still restores current retained content, and discarding a job clears the synchronization bookkeeping.
- Opening a closed candidate during text composition previously created a resource before the tab-switch guard rejected activation. The Open use case now checks composition before creating anything.
- The nested Style description dialog only made siblings inside Explorer inert. Rendering the dialog at the document body now makes the whole application background inert, while retaining the existing focus trap, Escape dismissal and focus return. The table remounts on date changes so a previous date's dialog cannot reappear on return.
- Unavailable action reasons previously existed only as mouse tooltips. The Open buttons now reference screen-reader descriptions of their availability; running candidates also describe their provisional/read-only state.
- Both tables now use the same `Bitmap` component, retaining the existing bitmap masks, tinting and sizes. No dependencies, assets, prompt text or transport contracts changed.

The delegated test runner passed all 15 tests across `candidate-table.test.tsx` and `generation-ui.test.tsx`, plus TypeScript, ESLint, Prettier and all seven gate lanes with loopback access. These checks cover the added review regressions; the original implementation evidence above remains historical. Narrow-Explorer visual qualification remains unchecked and this task stays Active for that evidence.
