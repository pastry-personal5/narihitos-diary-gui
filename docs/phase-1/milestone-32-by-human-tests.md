# P1-M32: By-human tests and observations

Status: Active

Repeatable manual checks for the personal release, including observations automation
cannot reliably make. The owner exempts all by-human execution from M32 and
Phase 1 exit gates (2026-11-09); this guide records coverage and limitations,
without requiring these tests before release closure.

## Acceptance and recording

Required automated/artifact work stays in the [release checklist](milestone-32-personal-release-overview.md#completion-checklist)
and [work plan](milestone-32-personal-release-architecture.md). Missing human
availability, permissions, phone, provider states or live-model access does not
block those checklists. Exempt means no execution requirement, not a passing
result. A defect found during an optional run still needs triage, a fix or an
explicit scope disposition; never relabel a known failure as passed.

For each H case, record date, tester/evidence source, observed result
(`not run`, `completed`, `passed`, `failed` or `blocked`), `gate: exempt-by-human`,
revision/build mode and artifact hash when known, environment, steps performed,
expected/observed behavior and a sanitized evidence reference. Mark unknown
fields unknown. Completion without detailed assertions does not establish that
every expected outcome passed. Link any finding to its implementation checklist.

## Setup and safety

1. Use an ordinary release server and copied `.app` outside the checkout, with an isolated private synthetic workspace/runtime/config. Follow the work plan's absolute-path independent shell launch; identify the build before testing. Record uncommitted runtime changes when applicable.
2. Prepare disposable producer schema 1 recording/audio and schema 2 manual snippets, including a header timestamp near a civil-date boundary. Keep original fixture copies for byte/metadata comparison. Use synthetic diary text in screenshots and model requests.
3. Select explicit Generation, Paragraph Review and Chat profiles and Writing Language in Settings. Use existing approved accounts/paths only; this guide grants no additional inference, service-write, spending, data-deletion or dependency authorization.
4. Phone/iCloud cases require an available test device and disposable destination. Notion writes stay within the previously authorized dedicated throwaway root; leave its pages intact. If a prerequisite is absent, record the observation as blocked/unverified and exempt, then continue independent work.

## H01 — Copied installation, discovery and ordinary close

Maps to R02/R03; E1/E5/E10/E12.

1. Start the copied standalone server and client independently from a directory outside the checkout, without Vite. Include spaces in the installation/config paths.
2. Try stale/wrong discovery and a second client; then connect correctly. Exercise actual folder-access denial where safely available, distinguishing TCC/provider denial from POSIX permission denial.
3. Close the client normally, then reconnect a new client to the still-running server.

Expected: safe connection errors, no borrowed discovery/session, usable successful
connection, no checkout resource dependency, clean client close and surviving
independently launched server. Record any inaccessible privacy state as unverified.

## H02 — Authentic editing, persistence and recovery

Maps to R04; E2/E12.

1. Create Empty on a disposable date. Edit through Draft and Entry and confirm both show the same Authentic content.
2. Change date/tab and close/reopen after an acknowledged autosave; verify saved bytes after server restart.
3. Make an external edit to the disposable file, then try saving a dirty buffer. Exercise available save retry/recovery controls without damaging workspace metadata. Use existing synthetic fault harnesses for failures that cannot safely be induced in the ordinary app.

Expected: acknowledged content survives restart; external conflicts are explicit;
failed/uncertain saves preserve the buffer and do not claim durable success.
Native composition is covered separately by H08.

## H03 — Source saves, producer and phone/iCloud round trip

Maps to R05; E3/E4.

1. Import disposable schema 1 recordings and schema 2 manual snippets across D-1/D/D+1 using the captured timezone. Navigate and select sources, including unsupported/conflicting entries.
2. Edit and explicitly save supported snippet bodies. Reopen in the producer and compare identity, title, unknown metadata and recording/audio bytes with the fixture baseline.
3. Where available, let the disposable files synchronize through live iCloud and reopen on the phone. Test guarded external edits and unavailable provider/mounted-volume states only when safely reproducible.

Expected: body-only changes and producer reopen succeed; unrelated metadata/audio
remain intact; unsupported/conflicting actions stay truthful. Local read-back is
not proof of phone synchronization or remote upload. Text-only Trash stays disabled.

## H04 — Trash controls and partial recovery

Maps to R06; E2/E4. M15 retains automated-only acceptance.

1. On a disposable recording fixture, inspect the exact snippet/audio ownership preview and cancel once; confirm no mutation.
2. Confirm once, inspect the OS Trash and reopen the producer catalog. If a partial/recovery state is available from a supported fixture, retry only outstanding members.
3. Verify text-only, ambiguous/shared or unsupported ownership actions remain disabled with an explanation as applicable.

Expected: exact ownership, truthful result and safe retry without repeated completed
side effects. Do not delete real data or manufacture live-cloud failures. Phone/
live-iCloud Trash behavior has no exit requirement.

## H05 — Generate Draft and candidate paragraph acceptance

Maps to R07/R08; E5/E6/E11/E13.

1. Select synthetic snippets and a Generation profile, then Generate Draft. Observe immediate local feedback, ongoing progress, provisional read-only output and validated candidates. Continue editing/navigating while generation runs.
2. Navigate away and return; inspect origin-date results and candidate table styles. Open, close and reopen a candidate, then verify background updates do not reopen closed tabs.
3. Exercise candidate paragraph review Accept/Reject against the captured Authentic baseline, including an intervening Authentic edit. Cancel a separate run when authorized. Restart and check the documented loss of transient results.

Expected: correct captured context/profile/date, no focus theft, stale acceptance
refused, independent serialized decisions and durable accepted content. Partial,
failed and cancelled outputs remain truthful. Observing plausible prose does not
establish general factuality or writing quality.

## H06 — Caret Review Paragraphs and AI Chat isolation

Maps to R07/R08; E5/E6/E11/E13.

1. Set different Paragraph Review and Chat profiles, open a synthetic user chat, then focus an Authentic paragraph without generating a candidate. Invoke the toolbar paragraph review.
2. Inspect the suggestion below that paragraph. Reject once without changes; request again, Accept, undo and verify normal autosave. Edit the baseline before accepting another suggestion and confirm stale refusal.
3. Include a typo, unsupported detail and a snippet with creation time but no stated event time. Inspect typo correction, reflective wording and whether uncertain creation-time clues remain tentative rather than invented event times.
4. Return to AI Chat, send with Enter and insert a newline with Shift+Enter. Verify the user transcript/context and independent profile selection survive paragraph review/cancellation. Exercise `/clear` only when no turn is running.

Expected: paragraph-only suggestion, guarded normal editor transaction, intact user
chat, correct context and independent profiles. Record concrete prose examples only
with synthetic content; a small sample is not a model-quality certification.

## H07 — Candidate table, bitmaps and native layout

Maps to R08/R09; E9.

1. Resize to the supported minimum window and a narrow Explorer, then resize AI Chat. Inspect candidate numbering, Style label and reachable bitmap Info/Open controls.
2. Open Style description with keyboard and pointer, dismiss with Escape and inspect focus return/background inertness. Open the same candidate repeatedly and confirm one tab.
3. Inspect Retina bitmaps/SF Symbols, readable fallback text, clipping and contrast. Inspect suggestion placement and top-right Accept/Reject controls.

Expected: reachable controls, readable/scrollable table, correct descriptions, no
duplicate tabs, trapped modal focus and correct focus restoration. DOM tests alone
do not establish these visual observations.

## H08 — Korean IME, keyboard, settings and reduced motion

Maps to R04/R09; E9/E12/E13.

1. Enable Korean input and compose in Authentic and Chat. Press Enter during composition; attempt date/tab changes and close while composing, then finish composition and save/send deliberately.
2. Exercise shortcuts, palette and modal focus return; inspect reduced-motion behavior under the macOS preference.
3. Confirm Settings order (Server first, Status Line before Keyboard Shortcuts last), independent profile selections, Writing Language, shortcut edits and Status Line toggles survive restart. Hide ready indicators and verify operational errors/progress remain visible. Exercise guarded config reload.

Expected: no premature send/save/navigation or dropped composition; predictable
keyboard focus, persisted settings and truthful ready/blocked transitions. Native
VoiceOver proof remains exempt independently of this guide.

## H09 — Packaged responsiveness, cancellation and wake

Maps to R10; E5/E9/E13.

1. Configure the approved external synthetic CLI in an isolated profile, keeping the ordinary app/server unchanged. Run slow/no-output, admitted valid-stream and malformed-flood cases separately.
2. While running, type and observe acknowledged autosaves; measure local input-to-feedback against the existing 100 ms target, publication cadence, cancellation acknowledgement/terminal/descendant cleanup, logical payload and process RSS separately.
3. Record stream/input sizes, duration, sample count, median/tail/max where meaningful and idle memory after repeated cycles. Do not invent RSS/cancel thresholds; a fake clock is not real-device latency evidence.
4. Disconnect/reconnect, reload frontend, restart server and sleep/wake before/after lease expiry. Observe origin-date routing, silent restored baseline, once-only new completion after disconnect and zero implicit new inference.

Expected: responsive editing/control, bounded updates/payloads, truthful stale/
interrupted state and no focus theft or duplicate inference. Record target misses
as findings. Missing measurements remain unverified and exempt.

## H10 — Live iCloud export and provider states

Maps to R11; E7.

1. Export acknowledged saved synthetic Markdown to a disposable live File Provider destination; read back exact bytes.
2. Externally edit the destination, then inspect confirmation freshness and cancel/retry controls. Exercise unavailable, evicted/download-needed and denied states only when safely inducible.
3. Observe cancellation, uncertain/restart reconciliation and leftover staging files using available supported procedures. Check remote/device visibility separately if available.

Expected: saved-revision/destination guards and truthful local-write/uncertain
outcomes, with staging cleanup. Record which provider states could not be induced.
A local write does not prove completed upload.

## H11 — Notion controls and optional live round trip

Maps to R12; E8.

1. Review the current fake-service recovery matrix and prior authorized live proof for contract changes before considering an additional live run.
2. If authorized and necessary, export synthetic saved text under the existing throwaway root. Read back heading/order/Unicode and inspect confirmation/conflict/recovery controls.
3. Record created page IDs privately; leave pages intact. Exercise partial/lost-reply/rate-limit failures in fake-service tests rather than inducing destructive remote failures.

Expected: exact saved-revision intent and truthful non-atomic/ambiguous outcomes;
no silent replay or writes outside the authorized root. No new live run is required
for exit acceptance.

## H12 — Installation guide and redacted diagnostics

Maps to R13; E10/E12.

1. Follow the documented copied-install, independent start/shutdown and recovery steps using synthetic data.
2. Inspect notices, optional-agent unavailable messages and current Settings/profile instructions.
3. Inspect a supported redacted trace and synthetic screenshot before sharing; confirm no raw credentials, diary content, prompts or private paths enter repository evidence.

Expected: reproducible instructions, accurate limitations and reviewable sanitized
evidence. Required automated notices/redaction checks remain separate.

## Qwen AI Agent: owner-reported completion

Owner report (2026-11-09): **Qwen AI Agent by-human run is done: Generate Draft,
Review Paragraphs and AI Chat.** This is a completed human-run note for H05/H06,
not a new agent-executed test or a request to rerun inference.

- Evidence source: the owner's report in this task; `result: completed`, `gate: exempt-by-human`.
- CLI/model version, configuration, revision/artifact hash, exact review surface (candidate review versus caret review), detailed assertions and logs were not supplied. Do not infer them or mark every H05/H06 step passed.
- This records workflow completion only; cancellation/load/recovery coverage, packaged artifact qualification and general factuality/writing quality are not established by the report.
- Historical GPU-throttling exemptions remain part of earlier milestones' evidence. This new completion note neither rewrites those runs nor makes further live/manual execution mandatory. Automated Qwen protocol/compatibility coverage remains required.

## Other results

No other by-human procedure was executed by this documentation change. Unless a
linked earlier record supplies a scoped observation, H01–H12 remain `not run`
with `gate: exempt-by-human`; H05/H06 have only the scoped Qwen completion above.
