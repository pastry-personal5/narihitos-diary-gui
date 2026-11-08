# P1-M29: Owner-requested revisions work plan

Status: Archived — P1-M29 completed 2026-11-08

Approach and sequencing for the [P1-M29 overview](milestone-29-owner-requested-revisions-overview.md). Filled in per request; this skeleton holds only the working rules.

## Rules

- Group related requests and order them by dependency; do bug fixes before behavior changes that touch the same code.
- Each task names the modules it touches and the test or smoke that proves it. Add a regression test for every bug fix.
- Keep layers and the [architecture](../../../architecture.md) contracts; an architecture change updates the affected architecture doc in the same change.
- No new dependency, build tool or product behavior outside a request without owner approval.

## Task notes

T21 moves the connection display and manual Connect action from `App.tsx` to a Server card in `SettingsView.tsx`, passing the existing connection state and callback. `useConnection` remains app-scoped. UI tests verify placement, connection states and the disabled action during connection; existing shell tests connect through Settings.

T25 removes only the active-view `h3` in `Explorer.tsx`. Existing icon buttons retain their tooltip, visually hidden name and `aria-pressed` state. Verify the component diff and full gate; no navigation behavior changes.

## Risks

_Added per request._

T23 updates `generation/navigation.ts` so a closed Authentic tab has no local save prerequisite, while loaded tabs still settle their edits and loading/error states still block. `ActivityFeedback.tsx` reports generation controller errors using the existing content-free user messages. The integrated Generate regression closes the Entry tab before submission; feedback coverage checks actionable errors without repeated announcements.

T24 instruments `GenerationController` through the existing optional bridge diagnostic port and `log_client_event`, with closed event/error tags in the shared Rust diagnostic model. Native validation rejects private fields; logging is best effort and never awaited by generation. The API logs capture failures before job admission. Controller tests verify stage order, save failure, redaction and diagnostic rejection; native tests verify allowlists and rejection of body/raw error fields. Logging and contribution docs describe reproduction and trace selection.

T26 moves existing cards and the app-scoped profile controllers into the requested settings categories. A dedicated authenticated server route supplies the configured folder only to the native bridge; a main-window-only command opens that folder in Finder and returns no path to React. No arbitrary path is accepted from the webview. UI tests cover placement and reveal feedback; API tests cover session enforcement and configured/unavailable folders.

T27 reconciles only the maintained UX docs with T26: category/card ownership in `ux-gui.md`, Settings containment in `ux-information-architecture.md`, and the same labels in `ux-terms.md`. Check stale placement references, Markdown formatting, local links and the full gate; no runtime changes.

T28 adds `TrashRecovery.tsx` using the shared focus-trapping `ModalDialog` and existing `TrashActivity` coordinator controls. `App.tsx` owns toolbar/dialog visibility, shows only one recovery or result dialog at a time and returns to recovery after inspecting or cancelling a continuation. Global Messages retains ordinary notices. Existing notices direct users to Trash Recovery. The dialog uses the app’s dark palette, a viewport-bounded 960 × 740 layout, fixed header/close control and scrollable status/guidance cards. UI tests cover toolbar placement, disconnected/checking/error/read-only states, close/Escape/focus and inspection/continuation return without implicit confirmation.

T29 reconciles the three maintained UX documents with the delivered Trash Recovery behavior: toolbar trigger, modal containment, current situation, recovery actions, return/close behavior and Global Messages boundaries. Add the canonical AI Agents / AI Agent vocabulary for configured providers, distinguishing it from AI Chat, AI Activity and profile cards. Validate the cross-document wording, relative links, formatting and full gate; no runtime changes.

T30 moves `ActivityDetails` into `AIActivitiesDialog`, retaining the app-scoped `ActivityFeedback` and activity store. The existing bitmap info asset opens a separate focus-trapping explanation overlay; the main dialog uses a fixed header and scrollable 960 × 740 body. `ActivityStatus` projects metadata into the bottom status line with no per-tick live announcement. View results closes the dialog only after successful guarded navigation; failed navigation retains unread state and the dialog. Tests cover actions, modal focus/overlays, live status and disconnected observations; run a browser layout smoke and the full gate.

T31 aligns `AIActivitiesDialog.tsx` with the existing Trash Recovery visual hierarchy, adding an “AI Agents & results” eyebrow. Group the matching selectors in `styles.css` so both dialogs share shell, header, title, eyebrow, close, body, intro and guidance-card styling. Keep explanation-overlay styling scoped. Verify existing modal/action tests and the full gate.

T32 uses a shared one-pixel divider inside the existing six-pixel resize track in `styles.css`, with accent hover/focus/drag feedback. `PaneSeparator.tsx` exposes dragging state and preserves pointer capture, bounds and arrow-key behavior. Verify separator interactions.

T33 introduces `ToolbarButton.tsx` as the bitmap-only toolbar action primitive with a closed action-to-PNG map. App uses it for all eight actions, retaining handler, disabled, shortcut tooltip and pending-label logic. Original transparent PNG masks are reproducibly drawn by a dependency-free script. Document unique-icon and accessible-name rules; test the toolbar inventory, conditional Save and bitmap uniqueness.

### T34: Command Palette overlay

Owner requirements: the Main Toolbar Strip Command Palette button toggles a dedicated overlay. A search bar at the top updates a recommended command list below it as the user types. Calculate ranking internally and use it only to sort the list; display no rank numbers or ranking scores. The owner authorized implementation on 2026-11-08 by requesting the unresolved M29 work. The following design is implemented with evidence tracked in the overview.

#### Review findings and proposed fixes

- The previous plan left ranking and ties undefined. Specify a deterministic local calculation and examples below, including the empty query.
- A shared `ModalDialog` makes background siblings inert and traps focus, preventing a second click on the toolbar toggle. Use a non-modal palette dialog with explicit dismissal and focus rules instead; do not change the shared modal primitive.
- The dispatcher currently suspends shortcuts whenever any dialog is present. Preserve that protection while the palette is open; the existing palette shortcut opens it when closed, while the toolbar supplies the requested toggle.
- Current palette selection is an array index and can drift when availability or ordering changes. Track selection by action ID and recheck the current handler at execution.
- The existing unavailable-action test expects a warning after an attempted run. Retain that behavior and command confirmations; ranking must not bypass guards.

#### Overlay, lifecycle and focus

Use a compact dialog centered horizontally below the Main Toolbar Strip, with a fixed search/header area and a vertically scrollable list. Bound its width and height to the viewport, including at the app's 780 px minimum width. Use existing dark-theme tokens, visible focus/selection states and the existing distinct toolbar bitmap. Keep the toolbar button unobscured and expose its expanded state and controlled overlay ID.

The dialog is non-modal (`role="dialog"` with an accessible name; no `aria-modal="true"`), so the toolbar remains operable. Opening starts with an empty query and focuses the labeled search bar. The toolbar closes an open palette, including after focus has moved into search. Escape closes it outside IME composition. A pointer action outside the overlay or keyboard focus leaving it dismisses the palette without stealing focus from the new target. Treat the toolbar toggle as a special case in outside/focus dismissal so one click cannot dismiss and immediately reopen it. Tab/Shift+Tab follow normal focus order; do not trap focus in a non-modal dialog.

Escape returns focus to the live invoker, falling back to the toolbar button if necessary. Toolbar dismissal leaves focus on that button. Command execution closes the palette before invoking the handler, allowing Settings, previews and confirmation dialogs to take focus without a palette cleanup stealing it back. Restore invoker focus only when the command has not established a new focus destination. Closing discards query and selection. Do not open above another modal or shortcut-capture session; if a modal appears while the palette is open, dismiss the palette and let the modal own focus. Keep existing global shortcut suspension while the palette is visible; normal search editing and clipboard keys remain available.

#### Internal ranking and matching

Use a pure local calculation over existing registry labels and current availability. No new dependency, server request, AI service, usage history, persisted preference or command-priority table is needed.

1. Normalize labels and the query with Unicode NFC, lowercase, trimmed edges and collapsed whitespace; keep the original labels for display. An empty or whitespace-only query includes every registry action.
2. For a nonempty query, include a command if it matches one of these tiers, using its best tier: full label equality; full label starts with the query; every whitespace-separated query token prefixes a label word in query order using distinct word positions; full label contains the query. Label words are whitespace-separated. This preserves existing substring search such as `open set` matching `Open Settings`. No fuzzy typo matching, shortcut-string search or description search is added.
3. Sort by match tier first, then available before unavailable, then normalized label in deterministic code-point order, then stable action ID. For an empty query, omit the tier and use the same availability/label/ID ordering. Thus an exact unavailable match still precedes a weaker available match. Recommendations reflect availability without hiding unavailable commands or inventing history-based preferences.
4. Rank values remain internal. Rows show command label, current shortcut hints and any unavailable reason, with no numeric rank, score, percentage or numbered-list presentation.

Examples for ranking tests (labels below are fixtures; they do not add commands):

| Query / context                                         | Expected order or result                                                                        |
| ------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `save`; all available                                   | `Save`, `Save source`, `Open Save`, `Autosave` (exact, label prefix, word prefix, substring)    |
| `  OPEN   SET  `                                        | Matches `Open Settings`; normalization does not alter the displayed label                       |
| `open ai`; all available                                | `Open AI activity` before `Open recent AI activity` (label prefix before ordered word prefixes) |
| `save`; exact `Save` unavailable                        | `Save` still precedes available `Save source`; its reason remains visible                       |
| Empty or spaces; available `Save`, unavailable `Export` | `Save`, then `Export`                                                                           |
| Same tier, availability and label                       | Stable action ID breaks the tie; input registry order does not change the result                |
| `zzzz`; no label matches                                | No selected command; show `No matching command` and Enter performs no action                    |

#### Selection, execution and accessibility

Keep DOM focus in the search combobox during arrow navigation and identify the active option through `aria-activedescendant`, stable option IDs, `aria-selected` and the associated listbox. Scroll the active option into view. A changed query selects the first result; an unchanged query preserves the selected action ID through availability updates, falling back to the first result if it disappears. Empty results clear selection. Arrow navigation clamps at the first/last row. Unavailable rows remain inspectable and selectable, with `aria-disabled` and a readable reason.

Enter or pointer activation uses the selected action ID and the latest handler/availability, never a stored array index or cached permission. Close first, then run once or post the existing unavailable warning. Preserve delete/export previews, explicit confirmations, save guards and asynchronous failure feedback. Typing and ranking alone never execute a command. During IME composition, Enter/arrows/Escape must not execute, navigate or dismiss the palette; process the committed search text normally. Prevent held Enter from executing repeatedly. Result counts may be announced politely without announcing internal scores or moving focus; retain understandable labels and shortcut speech.

#### Placement, sequence and evidence

1. Add a small pure ranking helper under `src/application/actions/` and focused ranking fixtures, including normalization, every match tier, ordered tokens, ties, empty query and no matches.
2. Update `CommandPalette.tsx` for ranking, ID-based selection, combobox semantics and composition-safe interactions. Update `App.tsx` and `useAppActions.ts` for separate toolbar toggle versus existing `palette.open` semantics, dismissal, focus and modal coordination. Preserve the registry and its action IDs; do not broaden command scope or add unrelated actions.
3. Add scoped overlay styles in `styles.css`. Update `tests/settings-ui.test.tsx` and focused palette tests for toolbar open/close, reopening/reset, shortcut opening, outside click/focus dismissal, toggle event ordering, result sorting with hidden scores, selection after live availability changes, unavailable warnings, single execution, guarded command dialogs, IME, Escape/focus and keyboard accessibility.
4. Run browser layout/focus smoke at 1280 × 900 and 780 × 600, including long labels/reasons, list overflow, toolbar reachability and command-to-dialog focus handoff. Check WKWebView keyboard/IME behavior when a native run is available; record any remaining native evidence in P1-M32 rather than claiming browser tests prove it.
5. Update `ux-gui.md`, `ux-information-architecture.md`, `ux-terms.md`, the keyboard-shortcut contract and affected user documentation with the implementation. Run `scripts/gate.sh` and record evidence against each T34 checkbox. Completion still requires the evidence recorded in the overview; the owner-authorized live Notion proof is recorded there as passed.
