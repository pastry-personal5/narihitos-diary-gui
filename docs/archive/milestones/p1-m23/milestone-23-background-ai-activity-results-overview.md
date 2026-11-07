# P1-M23: Background AI activity and results overview

Status: Done

Complete the shared activity and results experience for AI Chat and generation: truthful progress across dates, discoverable terminal results and recovery during the owning live session. The [work plan](milestone-23-background-ai-activity-results-architecture.md) defines implementation stages and acceptance scenarios; this plan does not certify runtime behavior.

## Entry conditions and owner decision

P1-M21 and [P1-M22, including its review fixes](../p1-m22/milestone-22-review-overview.md), are Done. Follow the [background AI contract](../../../architecture/ai-job-experience.md), [API](../../../architecture/api-and-processes.md), [editing contract](../../../architecture/editing-and-agents.md) and [handoff E13](../../../phase-1/implementation-handoff.md).

Owner interview, 2026-11-06: close M23 on automated tests and the full gate. Explicitly move packaged macOS slow-run/streaming editing and autosave, toolbar/details, keyboard/reduced-motion, reconnect/wake and measured performance proof to P1-M28. No human testing or sign-off is an M23 exit gate. Native VoiceOver proof remains exempt under the existing P1-M13 decision. Automated accessibility assertions remain required. Owner follow-up: restore results indicators silently after frontend reload; historical completions do not announce again. No owner question remains unresolved.

## Scope

- Stage A: complete body-less chat/generation progress discovery through server DTOs, paginated job reads and typed bridge validation.
- Stage B: application-owned activity store, live-session reconciliation, truthful delay/freshness, same-ID admission recovery and bounded preview subscriptions.
- Stage C: shared Main Toolbar Strip summary/details, retained result indicator, guarded View results, cancellation, explicit dismissal/discard and coordinated accessible feedback.
- Stage D: fake-clock/adapter/bridge scenarios, real-server synthetic integration, full gate and documentation.

Reuse M22 capture, sequential generation, bounded candidate store, dirty-source choices and read-only cards. Reuse chat transcripts, adapter execution, control reservations and close acknowledgement. Include regression coverage for the M22 review's session/discard races and checked client payload accounting.

Paragraph review/acceptance and review-dependent discard guards belong to P1-M24. Persistent/offline AI jobs, new dependencies/build tools, prompt/adapter changes, system notifications and automatic inference retry are outside scope. No new live-provider proof is required for this presentation/recovery milestone; Qwen live tests remain **exempt: GPU throttling**.

## Completion checklist

Check only when evidence exists. Implementation items are checked only after their full acceptance evidence passes. Scenario IDs refer to the work plan.

### Plan review

- [x] R01 — Review transport, recovery, attention and acceptance coverage against current code and maintained contracts; record corrections in the work plan (2026-11-06).
- [x] R02 — Owner chose silent restoration of results indicators after frontend reload; recorded in the work plan (2026-11-06).
- [x] R03 — Delegated validation passed: Prettier on both M23 plans and changelog, local file links, checklist/scenario consistency and `git diff --check` (2026-11-06).

### Stage A — progress and discovery

- [x] C01 — Job list/get and subscription baselines carry validated, body-less progress for generation and chat; non-AI jobs retain their existing behavior. Closed phases, identities, origin, frozen profile/locale/baseline, truthful counts, elapsed/activity and cancellation are available (T01).
- [x] C02 — Explicit paginated `list_jobs` and existing `get_job` bridge commands validate identity, bounds, pagination and capability grants; generic HTTP/shell access is absent (T01).
- [x] C03 — A job not previously known to React is discovered after lost admission or reload while the native bridge retains the same lease. Ready candidate IDs/manifests and the current chat recover without extra inference; discarded results remain unavailable (T02/T03).

### Stage B — activity ownership and recovery

- [x] C04 — One application-owned store routes by instance/session/job and date/chat, independent of editor mounts. Duplicate/reordered/old-session events and late async reads cannot regress state or restore discarded payloads (T03/T04).
- [x] C05 — Disconnect marks observations stale; complete paginated reconciliation precedes claims of active/terminal state after gaps, activation and sleep. Confirmed lease loss/server restart clears transient bodies and reports interruption without replay (T03).
- [x] C06 — Local submission acknowledges immediately; ambiguous admission preserves immutable input/request ID and recovers the original job. The single client-wide busy reservation stays truthful through cancellation acknowledgement; composed chat input survives refusal (T02/T05).
- [x] C07 — Server-observed elapsed/activity and closed phases drive feedback. At 30 seconds of silence show a waiting hint; heartbeats do not count as model activity. Existing inactivity/wall/lease deadlines remain unchanged (T05/T06).
- [x] C08 — Preview text stays outside global React metadata, respects checked shared payload accounting and renders at most ten times per second per visible run; elapsed ticks at most once per second. Terminal/cancel/error updates bypass text batching; offscreen text does not render (T07).

### Stage C — shared surfaces

- [x] C09 — Main Toolbar Strip summarizes active chat/generation across dates. Keyboard-operable expandable details expose origin, profile, Writing Language, counts/stage, elapsed/activity and Cancel, with focus restored on close (T08).
- [x] C10 — One terminal message per scoped terminal sequence, with historical completions restored silently after reload; retained results remain discoverable until opened or dismissed. View results uses normal save/dirty-source navigation guards; completion never navigates or changes focus/caret/tab/selection/scroll (T08/T09).
- [x] C11 — Dismissal changes presentation only. Explicit terminal generation discard releases only the owning transient result, preserves admission deduplication and cannot race a read/session replacement. Failed/partial/cancelled outcomes retain valid candidates (T04/T10).
- [x] C12 — Same-date saved Authentic changes, including Create Empty against an absent baseline, mark candidates readable but requiring regeneration; no merge, write or extra inference occurs (T09).
- [x] C13 — One coordinated polite status source announces submission, important transitions and completion; output-only busy semantics, named controls, reduced motion and no token/tick announcement flood. No notification, sound or Dock attention request (T08).

### Stage D — proof and documentation

- [x] C14 — T01–T10 automated matrix and review follow-up cases pass, including no-delta adapters, silence/deadlines, stalled consumer and flooding, partial success/cancel races and recovery. Synthetic built-server coverage proves discovery/reconnect/discard/control availability; no real Qwen leg runs.
- [x] C15 — Required full gate passes: Rust fmt, Clippy with warnings denied, Rust tests, Prettier, ESLint with zero warnings and Vitest; also run the repository TypeScript check (`scripts/gate.sh`, G01–G07). Delegate execution and first failure analysis to `test_runner`.
- [x] C16 — README, maintained contracts, handoff E13, phase plan, roadmap, changelog, index and AGENTS describe delivered behavior and actual evidence limits. P1-M28 retains the explicit packaged proof obligation; do not claim native responsiveness from mocks.

## Planning evidence

2026-11-06: inspected the maintained contracts, M22 completion/review records, server job/chat routes, native bridge subscription/get paths, frontend event decoder, chat/generation controllers and existing toolbar/hooks. Findings F1–F6 and acceptance scenarios are in the work plan. Owner resolved the sole evidence-scope question above. Delegated planning validation passed: Prettier for all nine changed Markdown files, local links in the two new plan documents and `git diff --check`. The runner also reviewed checklist/evidence consistency. The full code gate was not run for this planning-only change; implementation and milestone gate remain pending.

Follow-up review validation (2026-11-06): `test_runner` confirmed formatting of the two M23 plans and changelog, local file links, C01–C16/T01–T10 consistency and whitespace checks. No application code changed and no implementation gate was run; M23 remains Planned. R02 was subsequently resolved by the owner: restore results indicators silently.

## Implementation evidence

2026-11-06, discovery foundation: exposed the existing allowlisted `list_jobs`
command through the typed frontend adapter. Added bounded page validation in both
native and frontend bridges, including identifiers, sequence bounds, duplicate
records, replay watermarks and cursor validation. Native reads reject
nonadvancing cursors. This is partial C02 work; AI progress transport and complete
reconciliation remain pending. No dependency, prompt or adapter invocation changed.

Delegated checks: TypeScript passed; focused activity-validation and adapter tests
passed (29 tests); existing native bridge tests passed (10 tests). The existing
native tests do not yet exercise malformed discovery pages directly. The full
milestone gate has not run. No implementation checklist item is certified by this
incremental evidence.

Chat discovery foundation (2026-11-06): current retained chat turns now expose
body-less request/chat/session identity, frozen profile/locale/origin, observed
phase/activity, cancellation and elapsed time through job list/get. Completion
freezes elapsed time. Delegated application chat tests passed (5), API chat tests
passed (9), Rust formatting and M23 document formatting passed. Router assertions
cover matching list/get projection, absence of transcript/provisional fields and
seeded conversation text, and frozen terminal elapsed time. This remains partial
C01 work: native/frontend progress transport, closed validation and recovery are
not yet delivered. Additive API v1 compatibility is recorded in the work plan.

Native progress transport (2026-11-06): job responses now additionally carry a
nested progress projection while preserving existing flat API fields. Native job
DTOs preserve typed generation/chat metadata in subscription baselines and reads,
validating identities, closed phases/errors, generation counts, candidate IDs and
baseline revisions during deserialization. Ordinary jobs remain compatible.
Delegated proof: 5 native tests matching the progress filter passed (including
round-trip, invalid projection, ordinary job and generation coverage); 10 existing
bridge tests, 9 API chat tests and 8 API generation tests passed. Rust formatting
and client Clippy with warnings denied passed. Frontend progress decoding and
session-level cross-field reconciliation remain pending; C01/C02 are not yet
complete. No full milestone gate has run.

Frontend progress and store foundation (2026-11-06): list/get/cancel decoders and
subscription snapshots preserve allowlisted chat/generation progress, reject
malformed identity/phase/count/baseline fields and verify snapshot ownership.
Added metadata-only ActivityStore with staged complete baselines, stale disconnect
state, silent historical restoration, attention dismissal and sequence-regression
protection. The store is not yet wired to controllers or UI; authoritative polling,
epoch/replay ownership, unavailable-result handling and recovery remain pending.
Delegated frontend transport checks passed (55 focused tests, TypeScript, ESLint
and formatting). Store/validation checks passed (20 tests, TypeScript, ESLint and
formatting). These are foundation tests, not certification of the full T03/T08
scenarios. C01–C16 remain unchecked pending their complete evidence.

Runtime integration foundation (2026-11-06): App routes native baseline/replay
observations into ActivityRuntime. It discovers retained generation jobs and the
current chat without starting inference, serializes per-job reads, polls active
metadata every second, backs failures off for 30 seconds and suppresses reads from
superseded runtime epochs. The generation controller adopts unknown results and
restores its active-job/busy state for a running generation. Delegated validation:
55 focused frontend tests passed for runtime integration; 17 generation controller
tests passed after the active-job recovery fix; formatting, TypeScript and changed
file ESLint passed. These changes do not yet establish complete recovery: result
availability, same-sequence ordering under every event race, wake reconciliation,
announcement consolidation and shared toolbar/results surfaces remain pending.

Toolbar/details foundation (2026-11-06): added expandable shared AI details with
captured origin/profile/language, phase/counts, elapsed and cancellation status,
stale observations and retained-result attention. Explicit View uses the normal
guarded date change (now returning success); refused navigation retains unread
attention. Dismiss preserves metadata, while confirmed controller discard removes
the indicator and runtime tombstones prevent late reads from restoring it. Action
failures are visible. Escape/Close restores disclosure focus. Delegated checks:
activity details/runtime/store suites passed (5 tests); existing client/catalog/
navigation checks passed; formatting, TypeScript and changed-file ESLint passed.
The DOM details test covers focus return, retained attention on refused navigation
and dismissal without metadata removal. Full discard race coverage, coordinated
announcements, delay feedback and wake/reconnect reconciliation remain pending.

Coordinated feedback/reconciliation follow-up (2026-11-06): integrated AI
submission/completion feedback now uses Global Messages; chat and generation
panes opt out of duplicate completion live regions while their standalone defaults
remain compatible. Metadata updates carry a terminal notice counter so repeated
identical outcomes can be announced without tick/delta noise. Runtime invalidates
in-flight reads on resync and each new baseline and pauses polling through partial
baselines. Added a discard test covering both late reads and replayed baselines.
Delegated checks passed: 48 tests across eight focused suites, TypeScript,
formatting and changed-file ESLint. Delay/activity-age feedback, wake reconciliation,
complete scoped terminal-sequence deduplication and full synthetic/gate evidence
remain pending; this does not certify C07/C13/C14.

The added `activity-feedback.test.tsx` passed separately (1 test; focused total
49), with formatting and ESLint clean. It verifies silent initial restoration,
no elapsed-only messages, one later terminal message, and no replay from duplicate
observations or a subsequent same-session baseline.

Silence and refresh bounds (2026-11-06): API chat/generation progress now adds
`silence_ms`, computed using the server's monotonic clock and last agent event
(or admission when no event exists). The optional additive field survives native
and frontend validation; fresh active details show the waiting hint from 30 seconds.
No heartbeat or frontend clock advances agent activity. Normal metadata hints are
limited to one read per second per job, with one in flight; a newly observed
terminal sequence bypasses scheduling delay, and repeated normal hints preserve
failure backoff. Delegated checks passed: 28 activity-focused frontend tests,
5 native progress tests, 9 API chat tests, 8 API generation tests, Rust/Prettier
formatting, TypeScript and ESLint. Focused tests cover the 30-second UI boundary,
hint flooding and terminal bypass. Full silence/deadline/heartbeat matrix and
packaged timing proof remain unverified (packaged proof belongs to P1-M28).

Activation/session integration (2026-11-06): frontend adapter activation,
visibility restoration and long heartbeat gaps check the existing lease and start
a fresh native paginated baseline/replay without closing the session. Overlapping
refreshes coalesce and old-channel deliveries are suppressed. A confirmed new
instance/session clears chat bodies and pending state; disconnect alone retains
them. The interruption notice enters coordinated feedback. Delegated evidence:
26 adapter/wake/runtime tests and 14 chat/activity tests passed; formatting,
TypeScript, ESLint and whitespace checks passed.

Extended `python3 scripts/smoke/agent.py` passed against the built server with
synthetic CLIs. It verifies request-ID discovery from job lists, body-less nested
progress identities/origin/candidate IDs/silence age, lease heartbeat availability,
and post-discard absence of progress and candidate access. Initial sandbox listener
failure was resolved by running with local listener permission; a harness heartbeat
expectation was corrected to the existing 204 response. This is partial C14 proof,
not full pagination/reconnect/flooding acceptance. No real Qwen ran; live Qwen stays
**exempt: GPU throttling**. The full milestone gate remains pending.

Admission recovery follow-up (2026-11-06): chat freezes request ID, profile,
locale and document at submission and retains the original pending message/busy
reservation after an ambiguous send response. Its explicit recovery action resends
the frozen request ID/input. Matching request-ID discovery can resolve uncertain
chat/generation admissions without a new inference. Generation transfers out of
its pending request reservation when the original job is found. Transient chat
read failures preserve pending state; only authoritative absence clears it.
Delegated validation passed: 46 tests across five focused suites, then 30 chat/
generation-controller tests after failed-read and discovery regressions; formatting,
TypeScript and ESLint passed. Coverage includes mutated-after-submit document
isolation, same-ID retry, failed recovery reads retaining busy, and generation
discovery settling lost admission with one inference. Deferred cancellation during
chat admission and bounded/offscreen chat preview publication still require work.

Chat cancellation/budget follow-up (2026-11-06): cancellation during uncertain
admission remains requested until same-ID retry or discovery identifies the
original job, then targets that job and waits for authoritative settlement. Cancel
while chat opening is pending skips Send and restores the message. Chat now shares
the application's SessionBudget: frozen context is checked before opening/sending
and fetched snapshots are checked before retention; exhaustion offers explicit
retry-reading feedback while retaining previous work. Delegated checks passed:
30 tests across chat/chat-UI/client/activity-feedback suites, including both
cancellation paths and pre-inference capacity refusal; formatting, TypeScript and
ESLint passed. Preview separation/rate limits and full shared-budget lifecycle
coverage still require implementation and evidence before C08 can be checked.

Chat preview ownership (2026-11-06): provisional output now lives outside ChatState
in a dedicated PreviewBuffer. Mounted output alone subscribes; updates coalesce at
100 ms, offscreen updates schedule no rendering work, and terminal/session/dispose
clears cancel pending batches immediately. Running text/revision changes do not
notify metadata subscribers. Shared budget checks include fetched output and the
previously published buffer before replacing retained payloads; controller disposal
releases chat allocations and invalidates reads. Delegated evidence: 33 tests across
five focused suites, formatting, TypeScript and ESLint passed. Tests flood the
buffer, exercise offscreen updates and terminal bypass, and prove 30 controller
text refreshes leave metadata subscribers silent. Packaged render/memory evidence
still belongs to M28; complete budget lifecycle and generation/off-date preview
coverage remain part of the outstanding C08/C14 audit.

Native pagination audit (2026-11-06): explicit listing and subscription baselines
now share page bounds/duplicate/cursor checks. Subscription baselines reject IDs
repeated across pages and verify progress instance/session ownership; native job
Get/Cancel validates requested identity and owning scope. Added actual API coverage
with 201 retained jobs: insertion behind the first page cursor is absent from the
remaining page and recovered through SSE replay from the first watermark.
Delegated evidence: API pagination/replay test, 10 existing native bridge tests,
7 native progress/page/generation tests, Rust formatting and client Clippy passed.
The audit also identified that adapter teardown currently closes the lease on a
transport disconnect; separating subscription detachment from acknowledged client
close remains required before C05 can be certified.

Lease-preserving teardown (2026-11-06): closed subscription Stop now detaches only
the native delivery flow/task; frontend teardown and setup failure no longer close
the owning lease. Native reconnect retains prior trusted state across failed
discovery reads/status transport errors, validates an existing lease by heartbeat,
and clears expired identity on authoritative 401. Window close still uses the
acknowledged close path. Updated README and maintained process contract.
Delegated evidence: 31 adapter/wake/client tests, 12 native bridge tests, Rust/TS
formatting, TypeScript, ESLint and client Clippy passed. Native regressions prove
no DELETE during detach/reconnect, same-lease resume after status failure, preserved
lease across invalid discovery bytes, and cleared identity after confirmed expiry.
The earlier teardown audit gap is resolved; full reconnect/lease-result lifecycle
acceptance still needs the remaining matrix and full gate.

Availability/attention audit (2026-11-07): terminal-sequence deduplication now has
a scoped content-free ledger independent of visible results. Authoritative reads
without retained progress and complete baselines remove unavailable indicators and
release controller payloads; transient read failures retain them. Generation result
loss releases its budget and invalidates pending reads without sending Discard or
starting inference. Chat transcript cleanup preserves a newer retained turn for
the same chat. Delegated validation: 73 tests across eight focused suites, then
19 generation-controller tests including explicit unavailable-payload budget release;
formatting, TypeScript and ESLint passed. The full acceptance audit, activity-metadata
budget accounting, maintained documentation completion and full gate remain pending.

Activity metadata accounting (2026-11-07): live metadata, scoped terminal history
and staged baseline pages now reserve shared SessionBudget allocations before
retention. Capacity failure retains the previous baseline, releases only staging,
marks observations stale and offers explicit retry after freeing space. Disposal
releases allocations. Runtime suppresses payload discovery for observations that
cannot be retained and backs reads off. Delegated checks passed: 59 tests across
seven focused suites, formatting, TypeScript and ESLint, including staged-capacity
failure, prior-state retention, explicit retry recovery and zero allocations after
disposal.

C02 is checked on the combined explicit typed list/get adapters, fixed native
commands and existing minimal grants, shared native page/ownership/response-identity
validation, native malformed-page tests, frontend DTO tests and actual API
pagination/replay test. This check does not certify the full recovery matrix or
milestone gate, which remain pending.

C12 baseline evidence (2026-11-07): 20 generation-controller tests and the API
`stale_baseline_reported_and_final_candidates_stay_readable` test passed. Controller
coverage refreshes retained jobs on saved-Authentic invalidation without editor/date
ownership, preserves already read candidate payloads and makes no new inference.
The API test covers absent baseline becoming a present saved Authentic across all
fake adapter types, verifies staleness and keeps final candidates readable. This
includes Create Empty's absent-to-present baseline rule; the existing Authentic
create/save/restart smoke owns file creation itself. Formatting and ESLint passed.

Broad frontend integration check (2026-11-07): delegated full Vitest passed
(40 files, 305 tests). Workspace Rust formatting and overview Prettier checks
passed. This is broad regression evidence after metadata budgeting and lease
changes; it is not the remaining full milestone gate or unimplemented acceptance
scenarios.

Acceptance audit update (2026-11-07): C01/C03/C04/C06/C09/C10/C11 are checked
against the transport/controller/runtime/UI suites and synthetic evidence above.
Additional audit checks passed: 66 activity/runtime/UI/client/generation tests;
21 generation-controller tests including baseline discovery before online setup;
10 API chat tests including a no-delta adapter with no fabricated activity and
heartbeat-invariant frozen terminal progress; and 21 shared-summary/UI tests.
Rust/client and API Clippy, formatting, TypeScript and ESLint passed for the
corresponding changes. View/Discard acknowledgements now verify session scope;
completion preserves editor focus/caret; chat busy semantics apply to output only.
The collapsed summary uses captured origin/phase and server-observed elapsed.

Evidence mapping for checked items: C01 uses API chat/generation projections and
native/frontend progress validation tests. C03 uses request-ID discovery, startup
ordering, frozen admission retries and the built-server discovery/discard harness.
C04 uses runtime epoch/serialized-read/reconciliation and native channel filtering
plus discard regression tests. C06 uses chat/generation admission and cancellation
controller tests with API deduplication. C09/C10 use ActivityDetails/Feedback DOM
and store tests for controls/focus, silent reload, terminal attention and guarded
navigation. C11 uses generation delayed-manifest/candidate-read/discard/new-session
regressions, runtime tombstones, payload accounting and built-server partial/cancel/
discard/deduplication assertions.

Remaining audit work: C05 must prove replay overflow cannot establish a usable
baseline prematurely; C07 still needs the combined deadline/silence evidence
record; C08 must verify generation terminal bypass of preview scheduling; C13
needs final coordinated important-transition/reduced-motion assertions. C14–C16
remain unchecked until the complete matrix, full gate and maintained docs close.

Replay completion and terminal bypass (2026-11-07): negotiated SSE streams emit
an internal replay-complete marker only after retained events drain. Native pages
remain provisional, replayed job IDs are refreshed through bounded authenticated
fixed routes, and only then does the bridge commit a complete baseline. Overflow
emits resync with no complete baseline; legacy streams keep their vocabulary.
Dedicated server and native HTTP/IPC tests passed for replay ordering, refreshed
jobs before commit and overflow without completion. Native progress/page tests
(7), store/runtime tests (11), both affected crate Clippy checks, formatting,
TypeScript and ESLint passed. The maintained API contract records negotiation.
Together with the lease-preservation/replacement and stale-read tests, this checks
C05.

Generation terminal hints now bypass the 100 ms preview polling timer; a hint
arriving during a serialized read requests an immediate follow-up. Generation and
runtime suites passed (28 tests), including a zero-timer-advance terminal update;
formatting, TypeScript and ESLint passed. C08 remains open for its final combined
render/budget audit; C13–C16 still require their remaining evidence and closeout.

Final progress/render evidence (2026-11-07): coordinated feedback announces waiting
and cancellation transitions once per active run without repeated silence ticks;
pre-admission cancellation has explicit local notices. Busy semantics stay on chat
output while the composer remains editable. Delegated frontend checks passed
(94 tests across 12 files), including six generation UI tests with visible-preview
DOM mutation bounds and off-date absence of text. Existing preview tests cover
chat metadata isolation, ten-per-second publication, terminal bypass and unmounted
output. Shared payload accounting tests cover frozen requests, retained output,
staged metadata, exhaustion and recovery.

Deadline/control proof: infrastructure `agent_process::inactivity_deadline_times_out`
and the explicit integration invocation of `wall_deadline_times_out` passed (one
wall test executed, not filtered). Application lease expiry, API
`slow_consumers_have_bounded_slots_and_do_not_block_controls`, and native delivery
full-window timeout, reserved terminal and encoded-size tests passed. No production
deadline changed. The synthetic built-server agent smoke also passed with negotiated
SSE replay completion, body-less discovery and heartbeat/discard checks. No real
Qwen test ran. Formatting, TypeScript and ESLint passed for the corresponding
changes. The no-delta/heartbeat test and 30-second UI boundary evidence complete
the combined silence/deadline picture; packaged measurements remain M28 work.

C07/C08/C13 are checked on that combined evidence. The reduced-motion
ActivityDetails DOM suite passed (4 tests), including named textual phase/language/
Cancel availability with no decorative animation; formatting and ESLint passed.
These assertions establish automated presentation behavior, not packaged AppKit/
WKWebView timing or native keyboard proof. C14's final matrix audit, the full gate
and maintained-documentation closeout remain before milestone completion.

## Final acceptance matrix audit

2026-11-07: inspected implementation and the dedicated results recorded above;
no acceptance scope was replaced by the passing tests. The required packaged
proof is explicitly outside this milestone under the owner's M28 handoff.

| Scenario | Current authoritative evidence                                                                                                                                                                                                                                |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T01      | API chat/generation projections; native progress/page validation and ownership tests; frontend activity validation; 201-job pagination and behind-cursor replay integration.                                                                                  |
| T02      | Chat frozen-input retry and generation discovery-before-connect/lost-admission tests; API same-ID deduplication; built-server request-ID discovery smoke.                                                                                                     |
| T03      | Native lease detach/resume/expiry and reconciliation commit/overflow tests; adapter wake/channel supersession; activity runtime unavailable/failed-read tests; chat/generation scope replacement tests.                                                       |
| T04      | Runtime serialized same-sequence, reconciliation-overtaken read and tombstone tests; generation delayed manifest/candidate/discard/replacement-session regressions; staged-budget failure preserves prior baseline.                                           |
| T05      | Chat and generation synchronous submission, before-send cancellation, deferred admission cancellation and busy tests; server cancellation lifecycle/late-success guards.                                                                                      |
| T06      | API no-delta heartbeat-invariant terminal progress; 30-second UI boundary; process inactivity and wall deadlines; application lease expiry/cancellation.                                                                                                      |
| T07      | Chat PreviewBuffer flooding/offscreen/terminal tests and metadata isolation; generation DOM mutation/off-date and terminal-hint tests; UTF-8/serialized capacity tests; API slow-consumer and native bounded delivery tests.                                  |
| T08      | ActivityDetails keyboard/focus/named controls/reduced-motion DOM checks; ActivityFeedback silence/cancellation/no-tick/completion tests; integrated panes opt out of duplicate live sources; native command capabilities provide no system-attention surface. |
| T09      | Guarded View refusal and old-session acknowledgement tests preserve unread; completion preserves editor focus/selection; saved-baseline invalidation preserves readable candidates; frozen capture/locale tests remain unchanged.                             |
| T10      | Store dismiss/open/terminal-sequence ledger and unavailable-result tests; generation partial/cancel/discard/deduplication API/controller tests; built-server candidate access disappears after discard.                                                       |

Review additions are covered by actual API pagination/replay and native negotiated
overflow tests, nonadvancing cursor validation, serialized/epoch-safe runtime reads,
30-second failure backoff, retained payloads on failed reads and immediate terminal
refresh. The pending full gate will verify the complete enabled tree together.

## Completion record

P1-M23 completed 2026-11-07 on automated evidence and the full gate, following the
owner's evidence decision. All C01–C16 items and T01–T10 scenario groups have the
implementation/test evidence mapped above. No dependency, build tool, prompt or
adapter invocation changed. The enabled tree passed `scripts/gate.sh`: G01 Rust
fmt, G02 Clippy all targets with warnings denied, G03 Rust tests, G04 Prettier,
G05 ESLint with zero warnings, G06 Vitest (40 files, 312 tests), and G07 TypeScript.
The initial gate found two formatting files; formatting was corrected before the
passing complete rerun. Logs: `/tmp/narihito-m23-final-gate-rerun`.
The final built-server `python3 scripts/smoke/agent.py` also passed, including the
negotiated SSE replay boundary. No real-provider call was necessary; Qwen live
proof remains **exempt: GPU throttling**.

README, AGENTS, maintained API/background-AI contracts, handoff E13, phase plan,
roadmap, changelog and index describe delivered behavior and its evidence limits.
These completed plans are archived. P1-M24 is next and owns paragraph review and
review-dependent discard guards. P1-M28 explicitly retains packaged slow-run and
maximum-stream editing/autosave, toolbar/details/off-date results, native keyboard/
reduced motion, wake/reconnect, and measured submission/responsiveness/cancel/render/
memory proof. Native VoiceOver proof remains exempt. Automated tests and synthetic
fixtures do not establish those native performance claims.
