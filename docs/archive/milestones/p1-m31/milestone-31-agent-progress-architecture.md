# P1-M31: AI Agent progress work plan

Status: Done (2026-11-09)

Implement the [progress checklist](milestone-31-agent-progress-overview.md) using client-owned preferences and authoritative AI Activities observations. No dependency or build-tool changes are needed.

1. Extend the guarded Rust user-settings store and bridge DTO with an `agent_progress` section: a bounded map of profile aliases to booleans. Missing aliases default to enabled. Validate both disk reads and updates, preserve other sections, and retain revision/conflict/recovery behavior.
2. Add a labeled checkbox for each row in Settings → AI Agents. Save through `useSettings`; disable edits while unavailable, read-only, recovering or saving. Keep each preference independent of Chat/Generation profile selection.
3. Add an application progress reporter reading `AIActivitiesStore` every two seconds while enabled jobs are active. It uses current profile/job/request identity, generation candidate index and allowlisted phases to describe request preparation, waiting for a response, receiving/validating a response and cancellation. It never reads transcript or candidate payloads. Repeated observations must not reset the cadence.
4. Integrate the reporter into the existing AI feedback lifetime. Stop its timer on stale/disconnected state, terminal jobs, disabled preferences and unmount. Keep completion/warning/error feedback independent.
5. Add persistence/validation, accessible UI and fake-clock lifecycle regressions. Delegate focused checks and the full seven-lane gate to `test_runner`; record evidence before closing the checklist. Update maintained settings and AI UX contracts, then archive completed plans and fix links.

Implemented as planned. Focused and full-gate evidence is in the [completion record](milestone-31-agent-progress-overview.md#completion-evidence-2026-11-09). Packaged/native qualification remains M32.
