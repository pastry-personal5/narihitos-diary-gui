# P1-M31: AI Agent progress in Global Messages

Status: Done (2026-11-09)

Owner request (2026-11-09): add a per-profile progress toggle in Settings → AI Agents. While enabled, ongoing AI jobs report their current AI Agent turn (request/response) in Global Messages every two seconds. Implementation and the full gate pass; see the [work plan](milestone-31-agent-progress-architecture.md).

## Task and completion checklist

- [x] T02 — Show ongoing AI Agent turn progress in Global Messages, controlled independently for each AI Agent profile.
  - [x] Add an accessible toggle labeled **Show progress using Global Messages** to each profile in Settings → AI Agents.
  - [x] Default the toggle to on for existing and new profiles; save each profile's preference independently and restore it after restart.
  - [x] When enabled for the profile running an AI job, show what is currently in progress, identifying the profile/job and its current AI Agent turn and request/response stage. Refresh the progress in Global Messages every two seconds while the job is ongoing, including when waiting for a response.
  - [x] When disabled, suppress that profile's periodic progress messages. Stop periodic updates when a job completes, fails or is cancelled; do not present stale or disconnected observations as ongoing work. Keep existing completion, warning and error messages available.
  - [x] Cover defaults, independent persistence, enabled/disabled behavior, two-second timing, request/response transitions and terminal/disconnected cleanup with focused regressions. Update affected UX/settings contracts and pass `scripts/gate.sh` before marking implementation complete.

## Implementation preparation and acceptance

Before implementation, record a work plan covering the existing profile-settings owner, AI job/turn observations and Global Messages delivery. Use actual request/response lifecycle observations; do not invent progress or reasoning traces. The progress summary must not copy raw prompts, diary content, responses or credentials into diagnostic logs.

Acceptance evidence must demonstrate an enabled profile reporting its current request/response turn on a two-second cadence, a disabled profile producing no periodic progress, independent preferences surviving restart, and no periodic updates after terminal states. Completion evidence is recorded below.

M31's completed changeability refactoring and text-only snippet support remain complete and archived. This completed follow-up belongs to M31; M32 retains personal release qualification.

## Completion evidence (2026-11-09)

- Accessible checkbox, independent default-on preferences and restart restoration:
  `tests/settings-ui.test.tsx` exercises both profiles across App remounts;
  `tests/adapter.test.ts` verifies default omissions, valid preferences and rejected
  malformed DTOs. Rust `agent_progress_defaults_persist_independently_and_conflict`
  verifies real-disk reopen, unrelated-section preservation and stale-write refusal;
  `agent_progress_validation_and_invalid_section_preservation` verifies bounds,
  invalid-section fallback/preservation and explicit replacement.
- Two-second request/response updates, continuing waits, profile filtering,
  generation candidate/chat turn identity and terminal/stale/disconnected cleanup:
  `tests/agent-progress.test.ts` uses fake clocks and authoritative activity updates.
  `tests/activity-feedback.test.tsx` verifies settings-load gating, changing the
  preference during a run and unmount cleanup; existing completion/error tests pass.
- Delegated `test_runner` passed 53 focused frontend tests, 31 Rust settings tests
  and TypeScript checking. Final `scripts/gate.sh`: G01 fmt, G02 clippy, G03 Rust
  tests, G04 Prettier, G05 ESLint, G06 Vitest and G07 TypeScript all PASS. The first
  sandboxed Rust run could not open existing loopback test sockets; the final
  full gate passed with localhost socket access.

No real AI provider run or native visual inspection was performed. The automated
UI and real-disk storage evidence completes this task; packaged/native release
qualification remains M32. Progress describes observed lifecycle stages and turn
identity, without raw request/response bodies or reasoning traces.
