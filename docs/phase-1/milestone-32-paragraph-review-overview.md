# P1-M32: Caret paragraph review

Status: Active

Review and correct the owner's paragraph toolbar request. Use an isolated transient chat through the existing bridge, leaving the user's chat transcript intact. Capture the focused Authentic buffer and caret paragraph; show a block suggestion below it and apply only after an explicit Accept against an unchanged buffer.

See the [work plan](milestone-32-paragraph-review-architecture.md) for transport, lifecycle and editor details.

## Completion checklist

- [x] Paragraph Review works without a generated candidate and respects connection, profile, composition and review availability.
- [x] Only the caret paragraph is sent for review; the suggestion appears below it with Accept and Reject at top right.
- [x] Accept uses a normal editor transaction and autosave; Reject leaves text unchanged; changed drafts refuse acceptance.
- [x] Focused regression tests and the repository gate pass.

## Changeability refactor checklist

Owner authorization (2026-11-09): paragraph review is a growing advanced feature; incompatible internal and transport changes are permitted.

- [x] React only presents paragraph-review state and dispatches actions; application code owns eligibility, lifecycle, cancellation and decisions.
- [x] AI transport, polling and prompt construction sit behind an application-owned paragraph-review port, independently replaceable from editor and UI.
- [x] Editor capability is explicit and complete; paragraph selection policy and CodeMirror rendering have separate owners.
- [x] Server conversation ownership and routing are centralized, with named purposes instead of feature booleans.
- [x] Module responsibilities, dependency direction, extension points and future scope are documented and protected by boundary tests.
- [x] Application lifecycle regressions, adapter cancellation tests, existing behavior tests and the full gate pass.

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
