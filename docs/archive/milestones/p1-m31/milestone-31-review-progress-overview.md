# P1-M31: Generate, Chat and Review progress

Status: Done (2026-11-09)

The owner requests Global Messages progress for Generate, Chat and Review according to each running profile's saved toggle. Extend the completed [progress task](milestone-31-agent-progress-overview.md) to cover the independent paragraph-review execution path.

- [x] Report Review request/response stages every two seconds using its captured profile preference.
- [x] Keep two-second updates running throughout Generate/Chat preparation and admission as well as Review and execution, including unchanged waiting stages; never deduplicate away the periodic heartbeat.
- [x] Apply the toggle to Generate/Chat preparation and waiting notices, preserving completion, warnings and errors.
- [x] Stop Review updates after completion, failure, cancellation or disposal; ignore late callbacks and preserve user-chat isolation. During a stale connection, continue the two-second heartbeat with an explicit status-unavailable message, never a claim of ongoing progress.
- [x] Pass focused regressions and the full gate; update maintained behavior docs and record evidence.

## Work plan

Keep paragraph-review protocol and polling in its existing AI adapter. Add an optional content-free progress callback to its application port, carrying request/job identity and lifecycle stage only; the controller owns the captured profile and clears progress with its operation. Extend the existing application reporter to observe that controller alongside AI Activities on the same two-second cadence. Review remains outside user-chat discovery, so reporting cannot adopt its private conversation or overwrite the Chat transcript. Use actual adapter observations and keep raw request/response bodies out of Global Messages.

## Completion evidence (2026-11-09)

- `tests/review-progress.test.ts` exercises the real Review controller and chat AI
  adapter against a synthetic transport: two-second request/response stage updates,
  continuing waits, the captured review profile's independent preference,
  completion while disabled, failure/cancellation/disposal cleanup, late-callback
  refusal and no private prompt/response text in messages. The review conversation
  remains outside user-chat activity discovery.
- `tests/agent-progress.test.ts` covers Generate/Chat preparation heartbeats,
  admission-to-discovery cadence, repeated unchanged waiting stages, terminal
  cleanup and status-unavailable heartbeats across disconnect/resync.
  `tests/activity-feedback.test.tsx` covers preference gating for preparation/wait
  notices, preserved errors/completion, startup gating and settings snapshots
  changing without postponing the next heartbeat.
- Delegated runner: five focused Vitest files, 62 tests PASS; TypeScript PASS;
  final `scripts/gate.sh` G01 fmt, G02 clippy, G03 Rust tests, G04 Prettier,
  G05 ESLint, G06 Vitest and G07 TypeScript all PASS with localhost socket access.
  Documentation formatting and whitespace checks pass.

No live AI provider or native visual inspection was run. M32 retains packaged,
performance, sleep/reconnect and native release qualification. The cadence uses
client timers; no update can render while the app or system is suspended.
