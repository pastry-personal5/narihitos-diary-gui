# P1-M29 follow-up: Editor messages

Status: Done (2026-11-08)

## Snippet detail Close bitmap follow-up (2026-11-08)

Owner request: move Close to the top right of the source snippet detail overlay as a custom bitmap button.

- [x] T67 - Place an original Close PNG button beside the detail title at the upper right, retaining initial focus, dismissal and focus restoration.
- [x] T68 - Verify the generated asset and existing UI behavior, update the UX contract and pass the full gate.

Evidence for T67–T68: the 96×96 RGBA Close mask is transparent, antialiased and reproducible byte-for-byte. The header retains initial focus and the existing modal dismissal/focus restoration. Sixteen focused UI tests and all 391 UI tests pass. All seven gate commands pass, with Rust tests rerun successfully outside the sandbox after loopback permission failures. README, UX and owner decisions updated (2026-11-08). Native visual proof was not run.

Owner-requested follow-up (2026-11-08) to the [completed revisions](../archive/milestones/p1-m29/milestone-29-owner-requested-revisions-overview.md). Remove the visible “Tabbed Editor” title and move passive editor and generation messages into Global Messages.

## Owner decision

The owner requests that candidate count, locale, selected-snippet guidance and similar messages appear in Global Messages. Keep the accessible editor region name and existing editing, recovery and review actions.

## Completion checklist

- [x] T35 - Remove the visible editor title, retaining its accessible region name. Evidence: `tests/client.test.tsx` checks the named region and absent heading.
- [x] T36 - Route generation configuration, blocked reasons, adjacent-date warnings and errors to Global Messages without render-driven duplicates; keep retry/cancel controls. Evidence: `tests/generation-ui.test.tsx` checks guidance, blocked reasons, setting changes and unchanged rerenders; cancellation and dirty-source flows pass.
- [x] T37 - Route passive editor and candidate status/error notices to Global Messages; retain save-conflict recovery controls. Evidence: client test checks initial connection guidance in the log; generation UI tests check candidate status and stale-baseline feedback. Global Messages subscribes directly so initial notices display.
- [x] T38 - Verify affected UI behavior, update user/UX docs and pass `scripts/gate.sh`. Evidence: 18 focused client/generation UI tests pass; all seven gate commands pass (2026-11-08). README, UX behavior and decisions updated. Rust tests required unsandboxed loopback sockets; the initial sandboxed failure was permission-related. Native visual proof was not run.

Plan: [work plan](milestone-29-editor-messages-architecture.md).

## Month navigation bitmap follow-up (2026-11-08)

Owner request: replace Previous month and Next month text buttons with custom bitmap buttons, matching the existing app-owned masks.

- [x] T39 - Generate distinct antialiased chevron PNG masks and use them in the calendar header with accessible names, tooltips and existing navigation/disabled behavior. Evidence: `Explorer.tsx` retains the browse callbacks and boundary guards; 96×96 RGBA assets are reproducible, antialiased and exact horizontal mirrors.
- [x] T40 - Verify generated assets and affected UI checks, update user/UX docs and pass `scripts/gate.sh`. Evidence: all seven gate commands pass, including 378 UI tests; Rust tests pass on rerun outside the sandbox for loopback sockets. README and UX docs updated. Native visual proof was not run.

## Message and editor bitmap follow-up (2026-11-08)

Owner request: custom bitmap buttons for Jump to latest, Draft and Entry.

- [x] T41 - Generate distinct original PNG masks and wire the three icon buttons with accessible names/tooltips, retaining log scrolling and editor view selection. Evidence: `GlobalMessages.tsx` preserves the jump handler and visibility guard; `AuthenticPane.tsx` retains pressed state and the shared editor buffer.
- [x] T42 - Verify reproducible assets and existing scroll/editor behavior tests, update user/UX docs and pass all seven gate commands. Evidence: all three 96×96 RGBA PNGs are distinct, reproducible, transparent and antialiased; all seven `scripts/gate.sh` commands pass, including 378 UI tests (scroll retention/jump and Draft/Entry shared edits). The gate ran outside the sandbox for loopback tests. README and UX docs updated; native visual proof was not run.

## GUI process cleanup follow-up (2026-11-08)

Owner request: closing an app started with `run-gui.sh` must exit its owned processes cleanly.

- [x] T43 - Exit the client after main-window destruction, retaining acknowledged save/session close; supervise the launch-owned process group with bounded TERM/KILL cleanup and signal forwarding, without stopping the independent server. Evidence: ordinary `main.rs` exits on main-window destruction; `run-gui.sh` owns a separate process group, retains exit status and cleans it on EXIT/INT/TERM/HUP, with a two-second TERM grace before KILL.
- [x] T44 - Exercise successful close/launcher exit, command failure, interruption and TERM-resistant descendants with process-level regression tests; verify the native close path where available, update launch docs and pass the full gate. Evidence: `python3 scripts/tests/test_run_gui.py` passes four tests/six scenarios (normal exit, failure, INT/TERM/HUP, TERM-resistant descendants), preserving an independent sentinel. `python3 scripts/smoke/gui-close.py` passes with the ordinary app via the real launcher: AX close exits the launch-owned tree, the disposable server stays alive and immediately admits a new session. All seven gate commands pass, including 378 UI tests; process/native tests and the gate ran outside the sandbox. README, launch contract, contributor guide and smoke commands updated.

## Toolbar save status follow-up (2026-11-08)

Owner request: remove the top-right Saved text and use Global Messages for notifications.

- [x] T45 - Remove the toolbar save-status block and unused styling; retain existing save notifications in Global Messages and save/recovery actions.
- [x] T46 - Verify the affected UI and full gate; update the user and UX contracts.

## Chat clear command follow-up (2026-11-08)

Owner request: remove the composer placeholder, empty-message warning and New chat button; add `/clear` to reset chat context.

- [x] T47 - Replace the New chat button with a local `/clear` command, retaining the active-turn guard and accessible composer label; remove the requested text.
- [x] T48 - Verify command reset and submission behavior, update user docs and pass the full gate.

Evidence for T45–T48: client/settings tests verify the absent toolbar status; Authentic UI tests verify save notices in Global Messages. Chat UI regression verifies removed labels, disabled empty submission, the active-reply guard and local context reset without sending `/clear` to the agent. All seven `scripts/gate.sh` checks pass (2026-11-08), with loopback tests run outside the sandbox. README and UX contracts updated. Native visual proof was not run.

## Canonical AI terminology follow-up (2026-11-08)

Owner request: label AI Chat replies AI Agent and improve canonical UX vocabulary consistency.

- [x] T49 - Use AI Agent for completed/provisional chat speakers and reply announcements; align AI Activities accessibility labels and action wording, retaining technical transcript/API roles.
- [x] T50 - Update canonical vocabulary and affected assertions/docs; pass the full gate.

Evidence for T49–T50: chat UI tests check completed/provisional AI Agent labels and the completion announcement; activity tests check canonical accessibility labels and command-ranking fixtures use AI Activities. Canonical vocabulary and README updated. All seven `scripts/gate.sh` checks pass (2026-11-08); Rust loopback tests ran outside the sandbox.

## AI Chat context box follow-up (2026-11-08)

Owner request: show AI Chat context at the top left of the composer in a thin-bordered box containing a bitmap icon followed by the context text.

- [x] T51 - Add the context box and a distinct generated PNG mask, keeping the displayed context aligned with the document sent and retaining unsaved/empty context guidance.
- [x] T52 - Verify context updates, icon reproducibility and the full gate; update canonical terms and UX docs.

Evidence for T51–T52: chat UI tests verify the context box inside the composer, decorative bitmap, input description and updated/empty context labels. The 96×96 RGBA bitmap has transparency and regenerates byte-for-byte. All seven `scripts/gate.sh` checks pass (2026-11-08), including UI tests and Rust loopback tests outside the sandbox. Canonical vocabulary, README and UX docs updated. Native visual proof was not run.

## AI Chat keyboard follow-up (2026-11-08)

Owner request: Enter sends, Shift+Enter inserts a newline, and the composer shows no shortcut hints.

- [x] T53 - Change composer submission to Enter, retain Shift+Enter newline and IME safety, and remove visible shortcut hints including for `/clear`.
- [x] T54 - Verify keyboard behavior and the full gate; update the user and keyboard contracts.

Evidence for T53–T54: chat UI tests verify unmodified Enter submission, Shift+Enter default newline behavior with multiline submission, `/clear` on Enter, absent hints and both IME guards. All seven `scripts/gate.sh` checks pass (2026-11-08), with Rust loopback tests outside the sandbox. README, UX and keyboard contracts updated.

## AI Chat Send appearance follow-up (2026-11-08)

Owner request: a thin white upward arrow in a filled blue circle with transparent surroundings.

- [x] T55 - Replace the Send artwork with an original bitmap up arrow on a blue circle; preserve accessible name, disabled reasons, focus and submission behavior.
- [x] T56 - Verify bitmap reproducibility and existing chat behavior, update UX docs and pass the full gate.

Evidence for T55–T56: the new up-arrow PNG is 96×96 RGBA with transparency and regenerates byte-for-byte. The Send button retains its accessible name, reason and handler; CSS supplies the blue circle, white arrow and transparent surroundings. All seven `scripts/gate.sh` checks pass (2026-11-08), including existing chat behavior tests and Rust loopback tests outside the sandbox. README and UX docs updated. Native visual proof was not run.

## AI Chat placeholder follow-up (2026-11-08)

Owner request: show a dimmed Chat with AI. placeholder inside the empty input, disappearing immediately when typing.

- [x] T57 - Add the native textarea placeholder with muted styling, preserving the accessible Message label.
- [x] T58 - Update the existing placeholder expectation and user/UX docs; pass the full gate.

Evidence for T57–T58: the existing chat UI test checks the exact native placeholder text; CSS uses the muted palette token. Native textarea behavior hides it when the value is nonempty. All seven `scripts/gate.sh` checks pass (2026-11-08), including the full UI suite and Rust loopback tests outside the sandbox. User/UX docs and the superseding owner decision updated.

## AI Chat backgrounds follow-up (2026-11-08)

Owner request: very dark gray for the whole AI Chat background, with a darker gray input area.

- [x] T59 - Apply dedicated background tokens to AI Chat and its composer, keeping text, placeholder and controls readable.
- [x] T60 - Verify palette contrast, update user/UX docs and pass the full gate.

Evidence for T59–T60: foreground contrast is 11.47:1/12.18:1 for text and 6.79:1/7.21:1 for muted text on the chat/input backgrounds. All seven `scripts/gate.sh` checks pass (2026-11-08), with Rust loopback tests outside the sandbox. README and UX docs updated. Native visual proof was not run.

## AI Chat placeholder refinement (2026-11-08)

Owner request: remove the placeholder’s final period and darken its foreground.

- [x] T61 - Use Chat with AI without punctuation and a dedicated darker placeholder color.
- [x] T62 - Update the existing assertion and docs, verify contrast and pass the full gate.

Evidence for T61–T62: the existing chat UI assertion checks Chat with AI without punctuation; placeholder contrast against the input is 4.95:1. All seven `scripts/gate.sh` checks pass (2026-11-08), with Rust loopback tests outside the sandbox. User and UX docs updated.

## Automatic connection follow-up (2026-11-08)

Owner request: reconnect naturally and automatically after frontend reload, reducing retry delays where needed.

- [x] T63 - Attempt connection immediately after launch/reload/drop, retry at 100–1,000 ms, and retry promptly on activation/network recovery without overlapping handshakes. Retain native session reuse and close/unmount guards.
- [x] T64 - Verify reload, retries, recovery events and cleanup with regressions; update connection docs and pass the full gate.

Evidence for T63–T64: six deterministic connection-hook regressions pass, covering automatic mount/remount, stream drops, stale initialization, bounded retry delays, recovery-event single-flight behavior and cleanup/close suspension. The disconnected command-palette test now explicitly simulates an offline server instead of depending on startup delay. Native discovery/session retention remains unchanged. All seven `scripts/gate.sh` checks pass (2026-11-08), with Rust loopback tests outside the sandbox. README and connection contract updated. A native frontend-reload smoke was not run.

## Fresh-launch connection review (2026-11-08)

Owner request: review and fix fresh GUI automatic connection logic and verify it is valid.

- [x] T65 - Keep the immediate initial attempt under Strict Mode, prevent early resync from publishing readiness, and publish connected only after initialization succeeds.
- [x] T66 - Verify startup races and failed initialization retries, run the ordinary native automatic-admission smoke and full gate, and record any evidence limits.

Evidence for T65–T66: ten connection-hook regressions pass, including Strict Mode fresh startup, early resync, failed initialization and late unmounted handshake responses. `python3 scripts/smoke/gui-close.py` passes with the ordinary GUI in a disposable workspace: automatic session admission occurs without clicking Connect, close removes the launch-owned tree, and the independent server remains available for immediate new admission. All seven required gate commands pass; the Rust lane passed in full on rerun after a transient agent cancellation failure, which also passed in isolation. Tests requiring loopback/native access ran outside the sandbox (2026-11-08).
