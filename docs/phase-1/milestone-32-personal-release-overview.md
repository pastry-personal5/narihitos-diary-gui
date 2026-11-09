# P1-M32: Personal release qualification

Status: Active

Qualify Diary v1.0.0 for personal use on the owner's Apple Silicon Mac, with independently shell-launched server and packaged client. This is the milestone-wide release checklist; the existing M32 feature checklists retain their implementation evidence. The [work plan](milestone-32-personal-release-architecture.md) defines sequencing, verification and evidence handling.

## Plan revision completion checklist (2026-11-09)

This checklist covers the requested documentation review only; release execution
below remains pending.

- [x] Separate mandatory automated/artifact acceptance from by-human observations throughout the active release plan and authority docs.
- [x] Document repeatable by-human procedures, expected outcomes and evidence limits without making their execution an exit gate.
- [x] Record the owner's completed Qwen Generate Draft, Review Paragraphs and AI Chat run without inventing artifact, version or quality evidence.
- [ ] Pass delegated documentation checks and the repository gate; record the results.

## M31 and latest-change review follow-up (2026-11-09)

Review the completed M31 buffer/source and workflow boundaries, text-only snippet
support, and the latest AI progress change. Fix demonstrated defects within M32's
existing regression and defect-triage scope; preserve concurrent working changes.

Findings reproduced by delegated pre-fix regressions:

- Generation read the configuration epoch after profile lookup, allowing a profile from the previous configuration to be captured with a newer epoch. Capture now samples the epoch before lookup.
- Chat preparation fallback ignored terminal activity observations and continued reporting a completed turn while its transcript read lagged. The matching job acknowledgement now stops that fallback; a fresh submission still reports preparation.
- Paragraph review deferred observing its context promise until after opening a conversation. A failed or cancelled context could produce an unhandled rejection and acquire an unnecessary conversation. Context is now awaited inside the cancellation race before opening.

- [x] Reject generation capture when configuration changes during profile lookup.
- [x] Stop Chat fallback heartbeats when the same job has an authoritative terminal observation.
- [x] Handle paragraph-review context failures and cancellation before opening a conversation.
- [x] Pass focused regressions and the full delegated gate; record evidence and limits.

Delegated evidence: the six new regression cases failed before implementation.
After fixes, all 32 tests in `agent-progress`, `activity-feedback`, `review-progress`
and `paragraph-review-adapter` pass, including a fresh Chat submission after a
terminal acknowledgement and late conversation-open cleanup. All 76
`diary-application` Rust tests pass, including the configuration/profile race.
Synthetic `scripts/smoke/agent.py` and `scripts/smoke/review.py` pass with temporary
loopback permission. Final `scripts/gate.sh` G01–G07 all pass after formatting;
`git diff --check` passes. Earlier sandbox/formatting failures were resolved.
This evidence supersedes the earlier failed gate result for this working tree;
it does not qualify a packaged release or add live-provider/native evidence.

## Scope

- Ordinary release binaries and `.app`, matching versions, embedded prompts/helper, notices, private configuration and separate shell launch.
- Complete editing, source, AI, review, export and recovery workflows, including the packaged/native obligations in [E1–E13](implementation-handoff.md#capability-evidence-register).
- Native keyboard, Korean IME, focus, resize, Retina symbols/bitmaps, settings persistence, reduced motion, reconnect/wake and measured AI responsiveness.
- Installation/run/recovery instructions and an honest record of remaining limitations.

This plan adds no product behavior, dependency or build tool. Public distribution, Intel, signing/notarization, App Store delivery, history, and Phase 2 multiple-prompt management/evaluation remain outside scope. Ayumi mutations and text-only snippet Trash remain disabled. Unresolved iCloud conflict versions remain a known gap, not a new implementation task.

## Existing M32 work and evidence limits

| Work                                                                    | Existing evidence                                                                                                                 | Optional by-human observations                                                                                                                         |
| ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| [Draft candidate table](milestone-32-draft-candidate-table-overview.md) | Implementation, regression fixes and full gate passed                                                                             | Narrow Explorer visual inspection, bitmap controls, Info focus return and candidate tab reuse in the packaged app                                      |
| [Caret paragraph review](milestone-32-paragraph-review-overview.md)     | Application/adapter/editor boundaries, prompt/context follow-ups, independent profile selection, regressions and full gate passed | Native suggestion/Accept/Reject/undo/autosave and isolated user chat; synthetic live-model examples and explicit limits on writing-quality conclusions |
| [Settings and Status Line](milestone-32-status-line-overview.md)        | Ordering, saved toggles, eligibility, foreground colors, left alignment and full gate passed                                      | Packaged layout, persistence, disabled/ready transitions and operational status when readiness indicators are hidden                                   |

Previously passed gates establish the recorded revisions only. They do not qualify the final release artifact. The overall milestone remains Planned until release execution starts; no acceptance box below is satisfied by writing this plan.

## Acceptance policy (owner update, 2026-11-09)

By-human tests are documented in the [manual test guide](milestone-32-by-human-tests.md)
and **exempt from M32 and Phase 1 exit gates**. This supersedes earlier requirements
to obtain human-run packaged/native, phone/live-iCloud, visual/IME or live-model
proof before closure, including deferrals from earlier milestones. Unrun manual
cases remain unverified; exemption does not mean passed. Missing manual access or
evidence cannot keep an otherwise complete milestone open.

Required acceptance remains the automated regressions, ordinary release artifact
and process checks, feasible packaged automation, documentation and final gate
specified below. A06 may close with an evidenced feasibility disposition; missing
GUI tooling must not turn into a mandatory human test. Known failures still need
triage and fixes or an explicit scope decision; this exemption does not dismiss
observed defects. Service-write, dependency and user-data boundaries still apply.

## Completion checklist

Each required automated/artifact item needs a dated result, exact artifact/revision,
method, observations and evidence link. For mixed items, record the required result
separately from the linked non-gating H cases. A blocked required automated case
stays unchecked; exempt manual cases need no further waiver. Follow the
[execution order and evidence rules](milestone-32-personal-release-architecture.md#execution-order-and-evidence-rules).

- [ ] R01 — Inventory E1–E13, prior milestone deferrals, existing M32 checklists and unresolved findings; map each to current evidence or a release scenario, with no unexplained gaps.
- [ ] R02 — Build ordinary Apple Silicon release server/client and `.app`; verify package/Cargo/Tauri `1.0.0`, API/schema compatibility, notices, server-embedded generation/chat prompts and Apple helper, client-bundled paragraph-review prompts and absence of fixture/fault-injection features. Record hashes, OS/toolchain and build commands; helper fallback validation does not satisfy live Apple integration.
- [ ] R03 — Build an automated copied-install/process smoke using absolute paths and private synthetic configuration; prove launch without Vite or checkout/build-cache resources, discovery/authentication rejection, second-client/stale-discovery handling and owned-process cleanup. Exercise packaged launch/close where automation is feasible (A06); human folder-access/close observations are H01.
- [ ] R04 — Verify Create Empty, Draft/Entry shared Authentic editing, composition-aware autosave, date/tab/close guards, external changes, failed/uncertain save recovery and restart persistence through automated application/editor/integration tests. Assert acknowledged durable content and preserved unsaved buffers; native editing/IME observations are H02/H08.
- [ ] R05 — Verify import/date/selection/source navigation and guarded body-only saves for Narihito schema 1 recordings and schema 2 manual snippets through automated producer reopen/read-back tests. Preserve metadata/audio and truthful unsupported/conflict outcomes. Phone/live-iCloud and mounted-provider observations are H03.
- [ ] R06 — Reconfirm automated native/server Trash interruption/receipt/restart and producer catalog/reopen coverage on the final revision; cover preview/dialog/recovery controls in UI tests and feasible packaged automation. Use disposable fixtures. Human Trash observations (H04) are optional, consistent with M15 automated-only acceptance.
- [ ] R07 — Verify generation, AI Chat and candidate review with automated eligible-snapshot, unavailable-agent, provisional/validated, partial/cancelled, off-date, stale-baseline, serialized accept/reject, discard and restart-loss cases. Reuse approved real-profile evidence with an impact assessment; additional live/manual runs are H05/H06.
- [ ] R08 — Audit automated coverage across all three M32 feature plans: candidate tab retention, independent Paragraph Review profile, review without a generated candidate, stale acceptance refusal, undo/autosave, user-chat isolation and creation-time context. Feasible packaged automation is A06; remaining visual/live quality observations are H05–H07.
- [ ] R09 — Verify shortcut/palette, focus, composition guards, resize/layout constraints, symbol fallback, settings persistence and guarded config reload in existing automated suites and feasible packaged automation. Native Korean IME, Retina appearance and direct focus/layout observations are H07/H08.
- [ ] R10 — Verify slow/valid-stream/malformed-flood, save/control availability, bounded publications/payloads and reconnect/reload/restart/lease behavior with deterministic automated tests and ordinary-artifact synthetic process checks. Same-session baseline restoration is silent; a new completion after temporary disconnect announces once; no duplicate inference. Direct packaged timing/RSS and actual sleep observations are H09.
- [ ] R11 — Verify iCloud export saved-revision guards, external-edit confirmation, unavailable/download-needed/denied mapping, cancellation, uncertain/restart reconciliation and staging cleanup with automated adapter/temp-directory/fault tests. Record that these do not prove File Provider behavior or remote upload; live observations are H10.
- [ ] R12 — Verify Notion saved-revision/conflict/recovery and UI controls with synthetic service and application tests. Assess changes against prior authorized live round-trip evidence. Any additional human/live round trip is optional H11 and stays within the authorized throwaway root.
- [ ] R13 — Review privacy/redacted diagnostics, dependency notices and enabled capability limits; update the user guide/README with tested installation, independent launch, shutdown, configuration, recovery and known limitations. Reconcile stale documentation against current independent Paragraph Review profile behavior.
- [ ] R14 — Delegated final `scripts/gate.sh` (all seven lanes), version checks, production build and affected synthetic/native smokes pass for the final revision. Record artifact provenance and distinguish release-artifact results from debug/fault-injection results.
- [ ] R15 — Audit every required automated/artifact scenario and Phase 1 exit criterion, resolve failures or record explicit scope decisions, and give each non-gating H case an honest disposition. Complete required linked checklists and update phase/roadmap/handoff status together; archive plans only when acceptance evidence supports Done. Unrun exempt manual cases do not block closure.

## Automated test tasks

Owner follow-up (2026-11-09): automate release checks wherever practical. These
are implementation tasks within M32, not tests added or passed by this planning
update. Complete them before the final R14/R15 audit; link each result to its
release checklist item. Reuse existing coverage first and add tests for uncovered
failure modes rather than duplicating passing assertions. The [automation
matrix](milestone-32-personal-release-architecture.md#automated-test-matrix) defines
placement, assertions and evidence limits.

- [x] A01 — Add release-path artifact verification for versions, architecture, notices/resources, helper availability and fixture exclusion, with negative cases that prove missing/wrong/contaminated artifacts fail (R02/R14).
- [ ] A02 — Extend isolated process smokes to accept explicitly supplied ordinary release artifacts; automate independent launch, discovery/rejection, restart and cleanup, proving an unrelated process and the separately launched server survive client cleanup (R03/R14).
- [x] A03 — Audit and fill integration gaps across source/Authentic save, review and export recovery using existing producer, fault and fake-service harnesses; assert read-back, metadata/audio preservation, deduplication and no repeated side effects. Link existing coverage when no addition is needed (R04–R07/R11–R12).
- [x] A04 — Audit cross-feature UI/application regressions for candidate lifecycle, paragraph-review isolation/undo/stale guards and persisted independent profiles/Status Line settings; add only missing cases and link existing coverage (R08–R09).
- [ ] A05 — Audit and extend deterministic slow/flood/disconnect/reload/lease tests for continued saves/control, bounded publications/payloads, correct restored/new completion announcements and zero implicit inference. Exercise ordinary release process paths with an external synthetic CLI; feasible packaged automation is A06 and optional real-device measurements are H09 (R07/R10).
- [x] A06 — Automate feasible ordinary packaged GUI actions, screenshots and focus/geometry assertions with available macOS tooling; report unavailable GUI automation and link native/IME/provider observations to the exempt manual guide (R08–R10).
- [ ] A07 — Document and run separate offline, artifact and GUI automation commands with meaningful failure statuses, bounded deadlines, disposable data and exact artifact provenance; pass delegated checks and link remaining gaps to R01–R15 (R14/R15).

Automation task completion needs either passing implemented tests or an explicit
coverage/feasibility disposition with evidence. Existing equivalent tests can
satisfy A03–A05 after a relevant rerun. For A06, an unavailable GUI runner may
complete the feasibility assessment but cannot establish unobserved native
behavior. Record those observations as exempt H cases. No skipped required
automated/artifact case counts as passed.

## Preserved evidence decisions

The [decision register](../decisions.md) remains authoritative:

- Native VoiceOver proof is exempt; automated accessibility coverage remains required.
- Qwen automated compatibility coverage remains required. The owner reports the by-human Generate Draft, Review Paragraphs and AI Chat run completed; see the [scoped evidence note](milestone-32-by-human-tests.md#qwen-ai-agent-owner-reported-completion). Earlier GPU-throttling exemptions remain historical; no additional Qwen live run is an exit gate.
- M15 has automated-only acceptance. Phone/live-iCloud Trash behavior is an evidence limit, not a deferred manual release gate.
- M14 phone/live-iCloud and M26 live File Provider observations are now non-gating H03/H10 under the owner update above. Local directory smokes still do not establish those capabilities.
- The M11 packaged navigation waiver closes that milestone only. Qualify the current complete release workflow without reopening its historical acceptance.
- Notion test writes remain limited to the previously authorized dedicated throwaway root. This plan authorizes no private diary mutation, new destination, account spend or real-data deletion.

## Evidence record

Planning baseline: 2026-11-09. This revision runs no release workflow or live inference; the separately owner-reported Qwen manual completion is recorded in the manual guide. Add execution results here using the work plan's evidence format; retain previous feature evidence at its linked source.

## Plan review and corrections (2026-11-09)

Review against the actual build, asset adapters, smoke harnesses and existing
evidence contracts found and corrected these planning gaps:

- Added stage dependencies, prerequisite discovery, E1–E13 traceability and explicit pass/blocked/exempt rules, including evidence reuse and invalidation after changes.
- Corrected asset ownership: server-embedded helper and generation/chat prompts, frontend-bundled paragraph prompts. Added copied-install qualification to catch accidental checkout/build-cache dependencies.
- Made ordinary packaged slow/valid-stream testing executable through the existing external fake CLI, with separate malformed-output tests and measurement definitions.
- Removed implied duplicate-test obligations; coverage audits can reuse equivalent passing tests. Unavailable GUI automation does not pass the remaining native scenarios.
- Distinguished silent baseline history from newly observed completion after disconnect, helper fallback from live Apple success, and POSIX-denied access from actual TCC/provider evidence.

Release and automation execution remain pending. The subsequent owner update
above changes manual acceptance policy only; product behavior is unchanged.

Documentation evidence: the acceptance policy and R03–R12/R15 below separate
required and exempt coverage; [H01–H12](milestone-32-by-human-tests.md) contain
procedures/expected outcomes and the scoped Qwen record. The decision register,
phase, roadmap, handoff and linked feature plans carry the same override.

## Additional review corrections (2026-11-09)

- Split mixed R items into verifiable automated obligations and linked H observations, avoiding manual requirements hidden in release closure or feature checklists.
- Retain A06 feasibility closure and deterministic load coverage without promising inaccessible GUI/phone/provider automation or treating synthetic results as real-device measurements.
- Apply the manual exemption to Phase 1, the handoff and feature follow-ups; preserve defect triage and service/data authorization boundaries.
- Record the owner's Qwen completion separately from agent-executed or artifact-qualified evidence.

## Plan revision validation (2026-11-09)

Delegated `test_runner` validation passed changed-document Prettier, local-link
and new policy/Qwen-anchor checks, plus `git diff --check`. The full
`scripts/gate.sh` run passed G01/G02/G04/G05/G06/G07. G03 initially failed because
the sandbox denied loopback sockets; with loopback access the 94 client tests
passed. The workspace rerun still failed nine Apple Foundation Models tests with
unavailable/probe-timeout results. G03 remains failed; the validation checklist
item stays unchecked. No passing full gate or release acceptance is claimed.

This change updates plans and evidence policy only. No release artifacts,
by-human procedures or live workflow were executed by this documentation task;
the Qwen completion note is the owner's separately supplied report. Concurrent
implementation/test edits were preserved.

## Execution checklist (2026-11-10)

- [x] Implement explicit ordinary-artifact scans with negative regression cases.
- [x] Implement copied-install lifecycle checks with PID-bound readiness, bounded cleanup and harness regressions.
- [ ] Run offline coverage, production builds, artifact/process checks and assess GUI feasibility.
- [ ] Record coverage ledger, exact provenance, limitations and final delegated gate.

Execution ledger: [release evidence](milestone-32-release-evidence.md).

2026-11-10: A01/A04 and the harness implementation checklist are supported by
[delegated results](milestone-32-release-evidence.md#delegated-results-2026-11-10).
A02/R03 native cleanup and A06 packaged actions remain separate; The approved M24 findings are resolved in the post-hardening evidence below. M32 is Active, not release accepted.

## Approved review-defect resolution (2026-11-10)

Owner approved resolving the five historical M24 findings within M32.

- [x] Appended paragraphs own missing separators without a spurious replacement or merged text.
- [x] Reject checks disk state outside the global review mutex and revalidates admission.
- [x] Review startup preserves edits/composition that arrive during the server request.
- [x] Watcher reads from before a completed decision cannot invalidate its newer revision.
- [x] Lost-reply stale decisions clear obsolete controls and reload saved state safely.
- [x] Focused regressions, final gate, affected smokes and ordinary release artifacts pass after these runtime changes.

Final post-hardening gate and artifact results are in the [execution ledger](milestone-32-release-evidence.md#final-post-hardening-results-2026-11-10). Required copied-app lifecycle and ordinary-release streaming/save-control scenarios remain open; exempt human observations do not block closure.

## Implementation review and enhancement

- [x] Regress and fix cleanup when an owned process leader exits before its descendants.
- [x] Reject malformed or unrelated discovery records without crashing readiness.
- [x] Verify packaged license contents against the shipped notice inventory, including missing/changed-file negatives.
- [x] Handle a committed review decision whose updated editor presentation is refused, without retrying the mutation.
- [x] Recheck paragraph append subset/boundary behavior and fix demonstrated regressions.
- [x] Pass delegated focused checks, full gate and affected release checks; update evidence.

Implementation review validation: [focused, gate and rebuilt artifact results](milestone-32-release-evidence.md#implementation-review-follow-up). The full Rust workspace rerun passed after the initial transient synthetic Apple-helper failure. M32 release scenarios listed above remain open.

## Packaged GUI test attempt

- [x] Launch an isolated ordinary release fixture and record artifact identity.
- [x] Exercise observable native GUI controls, editing/settings/dialog focus and close where computer-use tooling permits.
- [x] Record passed/failed/unverified observations and verify owned-process cleanup separately from the independent server.

[Packaged GUI evidence](milestone-32-release-evidence.md#packaged-native-gui-run-2026-11-10) records the successful scoped run, clean client exit and durable synthetic text. AI Activities focus return remains inconclusive; copied installation and AI/provider/native IME scenarios are not claimed.
