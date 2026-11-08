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

## Toolbar save status follow-up

Remove the toolbar save-status block and its unused import/styles. Existing resource state transitions already publish save notices to Global Messages; retain that path and the save/conflict recovery controls. Update the shell expectation for the absent status and run the full gate.

## Chat clear command follow-up

Intercept a trimmed `/clear` in the composer before ordinary send validation, invoking the existing controller newChat reset without sending a model request or capturing document context. Keep reset unavailable during an active turn. Remove the New chat button, placeholder and empty-draft warning; preserve the accessible Message label and disable empty submission. Verify reset behavior through the UI and run the gate.

## Canonical AI terminology follow-up

Use AI Agent in rendered chat speaker labels and completion announcements, retaining the schema role `assistant`. Align activity region/status accessibility names and unavailable action wording with AI Activities. Update existing tests for completed/provisional replies and activity regions. Clarify the speaker convention and save-status location in canonical vocabulary, then run the gate.

## AI Chat context box follow-up

Place a non-interactive AI Chat context box before the composer textarea, inside its existing boundary, aligned left. Use the existing context chip as the label and a new original document-context PNG mask from the standard-library icon generator. Keep the bitmap decorative, associate the box with the textarea, wrap long labels, and remove the duplicated context from the send hint. Existing document capture and transcript chips remain unchanged. Verify rerendered contexts and asset reproducibility, then run the gate.

## AI Chat keyboard follow-up

Handle unmodified Enter as send while allowing Shift+Enter’s native textarea newline. Leave composing Enter untouched, including the legacy key-code 229 composition guard. Keep submit’s existing empty/blocked/reset checks. Remove shortcut hints from the composer reason line, retaining disabled reasons and send alignment. Update keyboard regressions and user documentation, then run the gate.

## AI Chat Send appearance follow-up

Generate a distinct thin up-arrow PNG mask with the existing bitmap script. Render it in white inside a blue circular span while the Send button surface stays transparent. Keep hover/press feedback inside the circle, disabled opacity and existing focus outline; retain the accessible name, disabled reason and send handler. Verify reproducibility and existing chat behavior with the gate.

## AI Chat placeholder follow-up

Use the textarea’s native placeholder attribute with the exact text Chat with AI. and the existing muted color token. Native placeholder visibility follows the empty value immediately without inserting text into the draft. Update the existing chat UI assertion and UX contract, then run the gate.

## AI Chat backgrounds follow-up

Add dedicated chat palette tokens: #141414 for the column and pane, #0c0c0c for the composer. The transparent textarea inherits the darker input surface. Keep existing foreground, context border, Send and focus colors. Verify foreground contrast and run the gate; no behavior changes or new tests are needed.

## AI Chat placeholder refinement

Remove the final period and use a dedicated #808080 placeholder token instead of the shared muted text color. Keep native placeholder behavior and verify contrast against #0c0c0c, with the existing test and full gate.

## Automatic connection follow-up

Keep native discovery validation and retained-session resumption unchanged. Schedule the first frontend attempt at zero delay, then 100, 200, 400, 800 and at most 1,000 ms between failed attempts. When disconnected, focus, visible activation or online events can trigger an immediate attempt. Guard concurrent handshakes with the connection epoch and clear retry/listener state on cleanup or close. Add deterministic hook tests for automatic launch/remount, backoff cap, recovery events, close and stale in-flight work. Run the full gate.

## Fresh-launch connection review

Reset retry history on connection-effect mount so Strict Mode’s setup/cleanup replay preserves the zero-delay first attempt. Gate status-only resync refresh until the initial status/session/settings/model initialization completes, and publish connected only after that sequence succeeds with the same epoch. Keep generator offline on initialization failure. Add Strict Mode, early-resync and initialization-failure regressions. Use the ordinary disposable-workspace gui-close smoke to prove automatic native session admission without clicking Connect, and run the full gate.
