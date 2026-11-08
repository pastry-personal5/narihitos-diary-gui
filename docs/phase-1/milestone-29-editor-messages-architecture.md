# P1-M29 follow-up: Editor messages work plan

Status: Done (2026-11-08)

Implementation plan for the [editor messages checklist](milestone-29-editor-messages-overview.md).

Remove only the visible heading from `AuthenticPane`, retaining the region label and Draft/Entry controls. Pass the app-owned `Messages` store into the editor and generation controls. Effects publish passive messages on meaningful input changes, with consecutive-message deduplication. Generation errors already flow through `ActivityFeedback`; avoid a second error announcement in integrated controls. Preserve actionable save-conflict explanations, confirmations, retry, cancel and review controls. Route candidate lifecycle/stale/error notices to Global Messages while retaining candidate review/discard controls. Subscribe the log directly to its store so notices emitted during initial mount are immediately visible.

Verify that generation configuration changes and blockers appear in the Global Messages log, never in the editor controls; check that the editor remains accessible and recovery actions remain available. Run the full gate. No dependency, backend, prompt or build-tool changes.
