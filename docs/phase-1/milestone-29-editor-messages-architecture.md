# P1-M29 follow-up: Editor messages work plan

Status: Done (2026-11-08)

Implementation plan for the [editor messages checklist](milestone-29-editor-messages-overview.md).

Remove only the visible heading from `AuthenticPane`, retaining the region label and Draft/Entry controls. Pass the app-owned `Messages` store into the editor and generation controls. Effects publish passive messages on meaningful input changes, with consecutive-message deduplication. Generation errors already flow through `ActivityFeedback`; avoid a second error announcement in integrated controls. Preserve actionable save-conflict explanations, confirmations, retry, cancel and review controls. Route candidate lifecycle/stale/error notices to Global Messages while retaining candidate review/discard controls. Subscribe the log directly to its store so notices emitted during initial mount are immediately visible.

Verify that generation configuration changes and blockers appear in the Global Messages log, never in the editor controls; check that the editor remains accessible and recovery actions remain available. Run the full gate. No dependency, backend, prompt or build-tool changes.

## Month navigation bitmap follow-up

Extend the existing standard-library bitmap drawing script with original left/right chevrons in `src/assets/navigation/`. Replace only the two calendar header button contents in `Explorer`, keeping their browse actions and boundary disabled states. Use decorative theme-tinted PNG masks, explicit accessible names and tooltips, and the existing icon button visual treatment. Keep the month label centered between the buttons without wrapping the header. No dependency, native symbol or build-tool change is needed. Verify asset geometry, run the existing UI suite and full gate, and document the visible controls.

## Message and editor bitmap follow-up

Extend the bitmap drawing script with a down arrow landing on a line (Jump to latest), a page with a pencil (Draft), and a checked page (Entry), stored in `src/assets/controls/`. Reuse `ToolbarButton` for the existing icon button treatment without changing navigation icons. Keep Jump to latest visibility and scrolling unchanged, and preserve Draft/Entry pressed semantics and shared editor buffer. Provide action-name tooltips and accessible labels. Verify reproducible PNGs, existing message/editor interaction tests and the full gate. No new dependencies, native symbols or build tools.

## GUI process cleanup follow-up

Keep the existing save/close guard and server cancellation acknowledgement. Explicitly exit the Tauri application on main-window destruction in ordinary as well as fixture builds. Give the npm/Tauri/Vite development tree its own Bash job-control process group in `run-gui.sh`; on launcher completion or INT/TERM/HUP, terminate only that group, allow a bounded graceful interval, then kill remaining members. Preserve the command exit status and avoid global process-name/port matching. Keep the standalone server outside this group. Use the existing Bash and Python test tooling without new dependencies. Add process-level synthetic tests with fake npm trees and an unrelated sentinel process, and attempt real native window-close proof using a disposable workspace only. Run the full gate and document any native evidence limit.
