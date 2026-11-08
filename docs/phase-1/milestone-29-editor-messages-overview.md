# P1-M29 follow-up: Editor messages

Status: Done (2026-11-08)

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
