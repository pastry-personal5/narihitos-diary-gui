# P1-M29: Owner-requested revisions work plan

Status: Active

Approach and sequencing for the [P1-M29 overview](milestone-29-owner-requested-revisions-overview.md). Filled in per request; this skeleton holds only the working rules.

## Rules

- Group related requests and order them by dependency; do bug fixes before behavior changes that touch the same code.
- Each task names the modules it touches and the test or smoke that proves it. Add a regression test for every bug fix.
- Keep layers and the [architecture](../architecture.md) contracts; an architecture change updates the affected architecture doc in the same change.
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
