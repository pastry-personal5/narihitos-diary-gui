# P1-M23: Background AI activity and results overview

Status: Planned

Complete the shared activity and results experience for AI Chat and generation: truthful progress across dates, discoverable terminal results and recovery during the owning live session. The [work plan](milestone-23-background-ai-activity-results-architecture.md) defines implementation stages and acceptance scenarios; this plan does not certify runtime behavior.

## Entry conditions and owner decision

P1-M21 and [P1-M22, including its review fixes](../archive/milestones/p1-m22/milestone-22-review-overview.md), are Done. Follow the [background AI contract](../architecture/ai-job-experience.md), [API](../architecture/api-and-processes.md), [editing contract](../architecture/editing-and-agents.md) and [handoff E13](implementation-handoff.md).

Owner interview, 2026-11-06: close M23 on automated tests and the full gate. Explicitly move packaged macOS slow-run/streaming editing and autosave, toolbar/details, keyboard/reduced-motion, reconnect/wake and measured performance proof to P1-M28. No human testing or sign-off is an M23 exit gate. Native VoiceOver proof remains exempt under the existing P1-M13 decision. Automated accessibility assertions remain required. Owner follow-up: restore results indicators silently after frontend reload; historical completions do not announce again. No owner question remains unresolved.

## Scope

- Stage A: complete body-less chat/generation progress discovery through server DTOs, paginated job reads and typed bridge validation.
- Stage B: application-owned activity store, live-session reconciliation, truthful delay/freshness, same-ID admission recovery and bounded preview subscriptions.
- Stage C: shared Main Toolbar Strip summary/details, retained result indicator, guarded View results, cancellation, explicit dismissal/discard and coordinated accessible feedback.
- Stage D: fake-clock/adapter/bridge scenarios, real-server synthetic integration, full gate and documentation.

Reuse M22 capture, sequential generation, bounded candidate store, dirty-source choices and read-only cards. Reuse chat transcripts, adapter execution, control reservations and close acknowledgement. Include regression coverage for the M22 review's session/discard races and checked client payload accounting.

Paragraph review/acceptance and review-dependent discard guards belong to P1-M24. Persistent/offline AI jobs, new dependencies/build tools, prompt/adapter changes, system notifications and automatic inference retry are outside scope. No new live-provider proof is required for this presentation/recovery milestone; Qwen live tests remain **exempt: GPU throttling**.

## Completion checklist

Check only when evidence exists. All implementation items remain unchecked during planning. Scenario IDs refer to the work plan.

### Plan review

- [x] R01 — Review transport, recovery, attention and acceptance coverage against current code and maintained contracts; record corrections in the work plan (2026-11-06).
- [x] R02 — Owner chose silent restoration of results indicators after frontend reload; recorded in the work plan (2026-11-06).
- [x] R03 — Delegated validation passed: Prettier on both M23 plans and changelog, local file links, checklist/scenario consistency and `git diff --check` (2026-11-06).

### Stage A — progress and discovery

- [ ] C01 — Job list/get and subscription baselines carry validated, body-less progress for generation and chat; non-AI jobs retain their existing behavior. Closed phases, identities, origin, frozen profile/locale/baseline, truthful counts, elapsed/activity and cancellation are available (T01).
- [ ] C02 — Explicit paginated `list_jobs` and existing `get_job` bridge commands validate identity, bounds, pagination and capability grants; generic HTTP/shell access is absent (T01).
- [ ] C03 — A job not previously known to React is discovered after lost admission or reload while the native bridge retains the same lease. Ready candidate IDs/manifests and the current chat recover without extra inference; discarded results remain unavailable (T02/T03).

### Stage B — activity ownership and recovery

- [ ] C04 — One application-owned store routes by instance/session/job and date/chat, independent of editor mounts. Duplicate/reordered/old-session events and late async reads cannot regress state or restore discarded payloads (T03/T04).
- [ ] C05 — Disconnect marks observations stale; complete paginated reconciliation precedes claims of active/terminal state after gaps, activation and sleep. Confirmed lease loss/server restart clears transient bodies and reports interruption without replay (T03).
- [ ] C06 — Local submission acknowledges immediately; ambiguous admission preserves immutable input/request ID and recovers the original job. The single client-wide busy reservation stays truthful through cancellation acknowledgement; composed chat input survives refusal (T02/T05).
- [ ] C07 — Server-observed elapsed/activity and closed phases drive feedback. At 30 seconds of silence show a waiting hint; heartbeats do not count as model activity. Existing inactivity/wall/lease deadlines remain unchanged (T05/T06).
- [ ] C08 — Preview text stays outside global React metadata, respects checked shared payload accounting and renders at most ten times per second per visible run; elapsed ticks at most once per second. Terminal/cancel/error updates bypass text batching; offscreen text does not render (T07).

### Stage C — shared surfaces

- [ ] C09 — Main Toolbar Strip summarizes active chat/generation across dates. Keyboard-operable expandable details expose origin, profile, Writing Language, counts/stage, elapsed/activity and Cancel, with focus restored on close (T08).
- [ ] C10 — One terminal message per scoped terminal sequence, with historical completions restored silently after reload; retained results remain discoverable until opened or dismissed. View results uses normal save/dirty-source navigation guards; completion never navigates or changes focus/caret/tab/selection/scroll (T08/T09).
- [ ] C11 — Dismissal changes presentation only. Explicit terminal generation discard releases only the owning transient result, preserves admission deduplication and cannot race a read/session replacement. Failed/partial/cancelled outcomes retain valid candidates (T04/T10).
- [ ] C12 — Same-date saved Authentic changes, including Create Empty against an absent baseline, mark candidates readable but requiring regeneration; no merge, write or extra inference occurs (T09).
- [ ] C13 — One coordinated polite status source announces submission, important transitions and completion; output-only busy semantics, named controls, reduced motion and no token/tick announcement flood. No notification, sound or Dock attention request (T08).

### Stage D — proof and documentation

- [ ] C14 — T01–T10 automated matrix and review follow-up cases pass, including no-delta adapters, silence/deadlines, stalled consumer and flooding, partial success/cancel races and recovery. Synthetic built-server coverage proves discovery/reconnect/discard/control availability; no real Qwen leg runs.
- [ ] C15 — Required full gate passes: Rust fmt, Clippy with warnings denied, Rust tests, Prettier, ESLint with zero warnings and Vitest; also run the repository TypeScript check (`scripts/gate.sh`, G01–G07). Delegate execution and first failure analysis to `test_runner`.
- [ ] C16 — README, maintained contracts, handoff E13, phase plan, roadmap, changelog, index and AGENTS describe delivered behavior and actual evidence limits. P1-M28 retains the explicit packaged proof obligation; do not claim native responsiveness from mocks.

## Planning evidence

2026-11-06: inspected the maintained contracts, M22 completion/review records, server job/chat routes, native bridge subscription/get paths, frontend event decoder, chat/generation controllers and existing toolbar/hooks. Findings F1–F6 and acceptance scenarios are in the work plan. Owner resolved the sole evidence-scope question above. Delegated planning validation passed: Prettier for all nine changed Markdown files, local links in the two new plan documents and `git diff --check`. The runner also reviewed checklist/evidence consistency. The full code gate was not run for this planning-only change; implementation and milestone gate remain pending.

Follow-up review validation (2026-11-06): `test_runner` confirmed formatting of the two M23 plans and changelog, local file links, C01–C16/T01–T10 consistency and whitespace checks. No application code changed and no implementation gate was run; M23 remains Planned. R02 was subsequently resolved by the owner: restore results indicators silently.
