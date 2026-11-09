# P1-M32: Caret paragraph review

Status: Active

Review and correct the owner's paragraph toolbar request. Use an isolated transient chat through the existing bridge, leaving the user's chat transcript intact. Capture the focused Authentic buffer and caret paragraph; show a block suggestion below it and apply only after an explicit Accept against an unchanged buffer.

See the [work plan](milestone-32-paragraph-review-architecture.md) for transport, lifecycle and editor details.

## Manual evidence policy (owner update, 2026-11-09)

Remaining native/visual/live observations are documented in the [by-human test guide](milestone-32-by-human-tests.md)
and exempt from M32/Phase 1 exit gates. Earlier statements below that keep this
feature open solely for human evidence are superseded. Preserve the historical
results; unrun manual checks are unverified, not passed. Automated requirements
and disposition of observed defects still apply.

## Completion checklist

- [x] Paragraph Review works without a generated candidate and respects connection, profile, composition and review availability.
- [x] Only the caret paragraph is sent for review; the suggestion appears below it with Accept and Reject at top right.
- [x] Accept uses a normal editor transaction and autosave; Reject leaves text unchanged; changed drafts refuse acceptance.
- [x] Focused regression tests and the repository gate pass.

## Owner-requested GUI follow-up: AI Chat width

- [x] AI Chat opens at twice its previous default width (480px) while remaining resizable.
- [x] Layout regression coverage verifies the 480px default and existing resize bounds; the focused test and full gate pass.

## Changeability refactor checklist

Owner authorization (2026-11-09): paragraph review is a growing advanced feature; incompatible internal and transport changes are permitted.

- [x] React only presents paragraph-review state and dispatches actions; application code owns eligibility, lifecycle, cancellation and decisions.
- [x] AI transport, polling and prompt construction sit behind an application-owned paragraph-review port, independently replaceable from editor and UI.
- [x] Editor capability is explicit and complete; paragraph selection policy and CodeMirror rendering have separate owners.
- [x] Server conversation ownership and routing are centralized, with named purposes instead of feature booleans.
- [x] Module responsibilities, dependency direction, extension points and future scope are documented and protected by boundary tests.
- [x] Application lifecycle regressions, adapter cancellation tests, existing behavior tests and the full gate pass.

## Reflective, evidence-supported prompt follow-up

Owner authorization (2026-11-09): permit clearly supported details from context
and default Paragraph Review to reflective writing: dwell on feelings and thoughts
the facts imply, without adding events.

- [x] All four provider assets use hypothesis checking, evidence gathering, optional 5W1H and reflective composition, followed by a factual audit.
- [x] Instructions distinguish supported context details from uncertain interpretation, preserve voice and uncertainty, and return only one revised paragraph.
- [x] Maintained prompt documentation describes the default and evidence limits; delegated focused checks and the full gate pass.

Prompt follow-up evidence: all four provider assets were reviewed for the shared
hypothesis/evidence/5W1H/reflection/audit policy and paragraph-only output. The
delegated runner passed the five paragraph AI adapter tests, changed-file Prettier
and whitespace checks, and all seven `scripts/gate.sh` lanes with authorized
localhost access after the sandbox initially denied bridge sockets. These checks
verify asset and integration compatibility; reflective writing quality and factual
restraint remain unverified by live-model evaluation.

## Loose 5W1H guide clarification

Owner clarification (2026-11-09): 5W1H is a loose guide, not a required sentence
structure or order.

- [x] Make the loose-guide wording explicit in all four provider prompts and maintained documentation; preserve natural reflective flow.
- [x] Delegated prompt checks and the full gate pass.

Clarification evidence: delegated validation passed all five paragraph adapter
tests, changed-file Prettier and whitespace checks, and all seven gate lanes with
authorized localhost access after sandbox socket denial. Live writing quality
remains unverified.

## Typo-first review follow-up

Owner request (2026-11-09): correct typos in every sentence of the paragraph
before the remaining review process.

- [x] Add a first typo-correction pass to all four provider prompts, preserving intended wording when a correction is uncertain; document the sequence.
- [x] Delegated prompt checks and all seven gate lanes pass.

Typo-first evidence: delegated validation passed all five paragraph adapter tests,
changed-file Prettier and whitespace checks, and all seven gate lanes with
authorized localhost access. Live-model typo-correction quality remains unverified.

## Prompt quality review follow-up

Owner request (2026-11-09): review and enhance Paragraph Review prompts with
careful attention to LLM behavior.

- [x] Refine all four provider assets: typo-first, evidence-supported context enrichment, loose 5W1H, reflective default and paragraph-only output remain intact.
- [x] Clarify original-text priority, uncertain typo corrections, source dates versus event dates, unsupported emotions despite hedging, and empty/conflicting context; include short boundary examples.
- [x] Update maintained prompt documentation and record delegated adapter checks, asset checks and all seven gate lanes; distinguish automated compatibility from live writing quality.

Quality-review evidence: all four provider assets were reviewed against the
paragraph/context request schema and surrounding chat instructions. The delegated
runner formatted the four prompt files and passed five paragraph adapter tests,
twenty Rust infrastructure prompt tests, whitespace checks and all seven gate
lanes with authorized localhost access. No runtime contract or model setting
changed. These results establish automated compatibility; they do not establish
live-model factuality or reflective writing quality.

## Creation-time evidence follow-up

Owner request (2026-11-09): gather evidence using snippet creation time when the
text does not mention time.

- [x] Send parsed header creation time, precision, diary-local projection and timestamp-conflict status with snippet context; do not substitute filename or filesystem times.
- [x] All four provider prompts distinguish explicit event time from tentative creation-time clues, preserve relative-time wording, and avoid inventing exact event times.
- [x] Cover metadata mapping and prompt payload preservation, update maintained docs, and pass delegated checks plus all seven gate lanes.

Creation-time evidence: delegated validation passed 21 focused context, adapter,
controller and frontend boundary tests, changed-file Prettier and whitespace
checks, and all seven gate lanes with authorized localhost access. Tests cover
UTC/local day boundaries, header-only and missing timestamps, conflicts, and the
actual AI request metadata payload. Live-model temporal inference quality remains
unverified.

## Current approach

The session-owned application controller owns eligibility, decisions and cancellation through complete AI/editor ports. The AI adapter owns prompts, polling, timeout and remote cleanup. CodeMirror owns widget layout, selection access and transactions; pure application policy identifies the paragraph. Composition supplies the current editor/profile/language and invalidates navigation synchronously. See the maintained [module boundaries](../architecture/paragraph-review.md). Existing candidate review remains separate.

## Evidence

Refactor evidence: the delegated runner passed the focused paragraph controller, AI adapter, UI, editor and frontend-boundary suites, server conversation isolation/cancellation tests, and all seven final `scripts/gate.sh` lanes with sandbox loopback restrictions lifted. The boundary contract identifies each owner and extension point; provider-specific feature instructions are now assets rather than source literals. Client task assets are separately inventoried and parsed with the Rust asset parser, without changing the historical server prompt-render fixture.

`tests/paragraph-review.test.tsx` verifies a paragraph-only request in the isolated chat slot and rejection without writes. `tests/review-editor.test.ts` verifies the block suggestion, normal edit/autosave notification and stale-draft refusal. The API regression `paragraph_chat_preserves_user_chat_and_closes_independently` proves user transcript isolation. The delegated test runner passed all seven `scripts/gate.sh` lanes after the final lint correction. Native visual inspection and a live AI run remain release qualification work.

## Profile isolation and busy recovery follow-up

- [x] Give Paragraph Review its own profile selection independent of Chat.
- [x] Retry transient busy profile checks promptly rather than backing off for up to an hour.
- [x] Cover both regressions and pass the full gate.

Follow-up evidence: delegated validation passed 19 focused profile/paragraph tests and all seven gate lanes. Rust tests passed with localhost socket permission after the sandbox blocked loopback setup. Profile selection remains independent across Settings close/reopen; busy probes retry at the base interval after prior backoff. Native/live-model reproduction remains unverified.
