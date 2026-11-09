# P1-M32: Personal release qualification

Status: Planned

Qualify Diary v1.0.0 for personal use on the owner's Apple Silicon Mac, with independently shell-launched server and packaged client. This is the milestone-wide release checklist; the existing M32 feature checklists retain their implementation evidence. The [work plan](milestone-32-personal-release-architecture.md) defines sequencing, verification and evidence handling.

## Scope

- Ordinary release binaries and `.app`, matching versions, embedded prompts/helper, notices, private configuration and separate shell launch.
- Complete editing, source, AI, review, export and recovery workflows, including the packaged/native obligations in [E1–E13](implementation-handoff.md#capability-evidence-register).
- Native keyboard, Korean IME, focus, resize, Retina symbols/bitmaps, settings persistence, reduced motion, reconnect/wake and measured AI responsiveness.
- Installation/run/recovery instructions and an honest record of remaining limitations.

This plan adds no product behavior, dependency or build tool. Public distribution, Intel, signing/notarization, App Store delivery, history, and Phase 2 multiple-prompt management/evaluation remain outside scope. Ayumi mutations and text-only snippet Trash remain disabled. Unresolved iCloud conflict versions remain a known gap, not a new implementation task.

## Existing M32 work and evidence limits

| Work                                                                    | Existing evidence                                                                                                                 | Remaining release evidence                                                                                                                             |
| ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| [Draft candidate table](milestone-32-draft-candidate-table-overview.md) | Implementation, regression fixes and full gate passed                                                                             | Narrow Explorer visual inspection, bitmap controls, Info focus return and candidate tab reuse in the packaged app                                      |
| [Caret paragraph review](milestone-32-paragraph-review-overview.md)     | Application/adapter/editor boundaries, prompt/context follow-ups, independent profile selection, regressions and full gate passed | Native suggestion/Accept/Reject/undo/autosave and isolated user chat; synthetic live-model examples and explicit limits on writing-quality conclusions |
| [Settings and Status Line](milestone-32-status-line-overview.md)        | Ordering, saved toggles, eligibility, foreground colors, left alignment and full gate passed                                      | Packaged layout, persistence, disabled/ready transitions and operational status when readiness indicators are hidden                                   |

Previously passed gates establish the recorded revisions only. They do not qualify the final release artifact. The overall milestone remains Planned until release execution starts; no acceptance box below is satisfied by writing this plan.

## Completion checklist

Each item needs a dated result, exact artifact/revision, method, observations and evidence link. A blocked required case stays unchecked until resolved or explicitly waived by the owner. Follow the [execution order and evidence rules](milestone-32-personal-release-architecture.md#execution-order-and-evidence-rules); automated/native/live labels describe the method, not interchangeable proof.

- [ ] R01 — Inventory E1–E13, prior milestone deferrals, existing M32 checklists and unresolved findings; map each to current evidence or a release scenario, with no unexplained gaps.
- [ ] R02 — Build ordinary Apple Silicon release server/client and `.app`; verify package/Cargo/Tauri `1.0.0`, API/schema compatibility, notices, server-embedded generation/chat prompts and Apple helper, client-bundled paragraph-review prompts and absence of fixture/fault-injection features. Record hashes, OS/toolchain and build commands; helper fallback validation does not satisfy live Apple integration.
- [ ] R03 — Copy the deliverable and notices outside the checkout, then follow documented independent shell launch using absolute paths and private synthetic configuration. Prove launch without Vite or checkout/build-cache resources. Exercise discovery/authentication rejection, second-client/stale-discovery behavior, actual packaged folder access/denial and ordinary close; the separately launched server survives client close.
- [ ] R04 — In the packaged client, prove Create Empty, Draft/Entry shared Authentic editing, IME-aware autosave, date/tab/close guards, external changes, failed/uncertain save recovery and restart persistence. Preserve unsaved content on failure; do not infer durable success from a timer.
- [ ] R05 — Prove import/date/selection/source navigation and guarded body-only saves for Narihito schema 1 recordings and schema 2 manual snippets, producer reopen and phone/live-iCloud round trips using disposable fixtures. Preserve metadata/audio; keep unsupported/conflicting source actions truthful. Record mounted-volume/provider availability limits.
- [ ] R06 — Reconfirm automated native/server Trash interruption/receipt/restart and producer catalog/reopen coverage on the final revision, plus packaged preview/dialog/recovery controls on disposable fixtures. Do not add a human or phone/live-iCloud Trash exit gate.
- [ ] R07 — Qualify packaged generation, AI Chat and candidate review: eligible snapshots, missing/offline agents, provisional versus validated results, partial/cancelled outcomes, off-date results, stale baselines, guarded serialized accepts/rejects, discard and restart loss. Integrate supported real-profile evidence without repeating inference automatically.
- [ ] R08 — Complete the remaining packaged visual/native checks in all three M32 feature plans, including caret review without a generated candidate, independent Paragraph Review profile, stale acceptance refusal and creation-time context. Record live-model examples separately from automated compatibility; do not claim general factuality or writing quality.
- [ ] R09 — Pass native keyboard/shortcut/palette, Korean IME, focus return, pane resizing, narrow Explorer/minimum-window layout, Retina symbol/bitmap and reduced-motion scenarios. Persist Writing Language, profile choices, shortcuts and Status Line settings across restart; validate guarded config reload.
- [ ] R10 — Measure packaged slow-run/maximum-stream editing and autosave, feedback (100 ms target), cancellation acknowledgement, render cadence and memory bounds. Prove disconnect/reconciliation, frontend reload, server restart, activation and sleep/lease expiry with no duplicate inference. Same-session baseline restoration is silent; a new completion first observed after a temporary disconnect announces once.
- [ ] R11 — Qualify packaged iCloud export on a disposable live File Provider destination: exact saved Markdown, external-edit confirmation, unavailable/download-needed/denied outcomes, cancellation, uncertain/restart reconciliation and staging-file cleanup. Distinguish verified local write from remote upload; retain unobservable provider cases as explicit gaps.
- [ ] R12 — Qualify packaged Notion controls and saved-revision/conflict/recovery behavior against synthetic service scenarios; link the prior authorized live round trip to the tested contract and assess changes since it. Any necessary additional live round trip uses only the authorized throwaway root and leaves its pages intact.
- [ ] R13 — Review privacy/redacted diagnostics, dependency notices and enabled capability limits; update the user guide/README with tested installation, independent launch, shutdown, configuration, recovery and known limitations. Reconcile stale documentation against current independent Paragraph Review profile behavior.
- [ ] R14 — Delegated final `scripts/gate.sh` (all seven lanes), version checks, production build and affected synthetic/native smokes pass for the final revision. Record artifact provenance and distinguish release-artifact results from debug/fault-injection results.
- [ ] R15 — Audit every required scenario and Phase 1 exit criterion, resolve failures or obtain explicit scope decisions, complete linked checklists and update phase/roadmap/handoff status together. Archive completed plans and fix links only after acceptance evidence supports Done.

## Automated test tasks

Owner follow-up (2026-11-09): automate release checks wherever practical. These
are implementation tasks within M32, not tests added or passed by this planning
update. Complete them before the final R14/R15 audit; link each result to its
release checklist item. Reuse existing coverage first and add tests for uncovered
failure modes rather than duplicating passing assertions. The [automation
matrix](milestone-32-personal-release-architecture.md#automated-test-matrix) defines
placement, assertions and evidence limits.

- [ ] A01 — Add release-path artifact verification for versions, architecture, notices/resources, helper availability and fixture exclusion, with negative cases that prove missing/wrong/contaminated artifacts fail (R02/R14).
- [ ] A02 — Extend isolated process smokes to accept explicitly supplied ordinary release artifacts; automate independent launch, discovery/rejection, restart and cleanup, proving an unrelated process and the separately launched server survive client cleanup (R03/R14).
- [ ] A03 — Audit and fill integration gaps across source/Authentic save, review and export recovery using existing producer, fault and fake-service harnesses; assert read-back, metadata/audio preservation, deduplication and no repeated side effects. Link existing coverage when no addition is needed (R04–R07/R11–R12).
- [ ] A04 — Audit cross-feature UI/application regressions for candidate lifecycle, paragraph-review isolation/undo/stale guards and persisted independent profiles/Status Line settings; add only missing cases and link existing coverage (R08–R09).
- [ ] A05 — Audit and extend deterministic slow/flood/disconnect/reload/lease tests for continued saves/control, bounded publications/payloads, correct restored/new completion announcements and zero implicit inference. Exercise the ordinary packaged app with an external synthetic CLI for real-device measurements (R07/R10).
- [ ] A06 — Automate feasible ordinary packaged GUI actions, screenshots and focus/geometry assertions with available macOS tooling; report unavailable GUI automation and retain specific native/IME/provider cases for direct qualification (R08–R10).
- [ ] A07 — Document and run separate offline, artifact and GUI automation commands with meaningful failure statuses, bounded deadlines, disposable data and exact artifact provenance; pass delegated checks and link remaining gaps to R01–R15 (R14/R15).

Automation task completion needs either passing implemented tests or an explicit
coverage/feasibility disposition with evidence. Existing equivalent tests can
satisfy A03–A05 after a relevant rerun. For A06, an unavailable GUI runner may
complete the feasibility assessment but cannot satisfy any unobserved native
release scenario; retain that scenario under R08–R10 for direct qualification.
No skipped required release case counts as passed.

## Preserved evidence decisions

The [decision register](../decisions.md) remains authoritative:

- Native VoiceOver proof is exempt; automated accessibility coverage remains required.
- Qwen live proof is **exempt: GPU throttling** until the owner lifts that exemption. Keep synthetic Qwen compatibility coverage; never report an exempt live run as passed.
- M15 has automated-only acceptance. Phone/live-iCloud Trash behavior is an evidence limit, not a deferred manual release gate.
- M14 source-save phone/live-iCloud evidence and M26 live File Provider evidence remain required; a local directory smoke cannot satisfy them.
- The M11 packaged navigation waiver closes that milestone only. Qualify the current complete release workflow without reopening its historical acceptance.
- Notion test writes remain limited to the previously authorized dedicated throwaway root. This plan authorizes no private diary mutation, new destination, account spend or real-data deletion.

## Evidence record

Planning baseline: 2026-11-09. No release qualification has run as part of preparing this plan. Add execution results here using the work plan's evidence format; retain previous feature evidence at its linked source.

## Plan review and corrections (2026-11-09)

Review against the actual build, asset adapters, smoke harnesses and existing
evidence contracts found and corrected these planning gaps:

- Added stage dependencies, prerequisite discovery, E1–E13 traceability and explicit pass/blocked/exempt rules, including evidence reuse and invalidation after changes.
- Corrected asset ownership: server-embedded helper and generation/chat prompts, frontend-bundled paragraph prompts. Added copied-install qualification to catch accidental checkout/build-cache dependencies.
- Made ordinary packaged slow/valid-stream testing executable through the existing external fake CLI, with separate malformed-output tests and measurement definitions.
- Removed implied duplicate-test obligations; coverage audits can reuse equivalent passing tests. Unavailable GUI automation does not pass the remaining native scenarios.
- Distinguished silent baseline history from newly observed completion after disconnect, helper fallback from live Apple success, and POSIX-denied access from actual TCC/provider evidence.

All release and automation implementation checklist items remain pending. This
review changes the plan only; existing product decisions and waivers remain in
force.
