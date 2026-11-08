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
