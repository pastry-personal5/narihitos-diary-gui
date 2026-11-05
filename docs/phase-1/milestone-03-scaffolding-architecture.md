# P1-M3: Scaffolding work plan

Status: Planned

Assemble the [scaffolding checklist](milestone-03-scaffolding-overview.md) from the completed [P1-M2 design](../archive/milestones/p1-m2/milestone-02-architecture-overview.md). This required milestone work-plan file describes implementation sequencing; maintained architecture contracts define the interfaces and behavior.

## Approach

1. Read the approved contracts and inventory concrete required dependencies, versions, licenses, native features and build commands. Obtain package/build-tool approval before setup; document-level recommendations do not authorize installation.
2. Create the Cargo workspace under `src-tauri` with the specified client/server/inward crates. Verify the unchanged manifest-path gate commands cover all default members, and that domain/application code cannot import adapters.
3. Wire startup configuration, workspace lock, loopback discovery/auth/session/status and bounded fake-job HTTP/SSE. Validate the specified port fallback and tagged CLI/native profile schema without starting an agent. Exercise admission, cancellation, duplicate requests, stale discovery, denied auth/origin/Host, expiry and event replay gaps using synthetic state.
4. Assemble the narrow Rust Tauri bridge and explicit main-window command permissions. Add the typed frontend bridge/reducers, dedicated CodeMirror module and minimal text-labeled Standard Layout. Use fake status/jobs to verify IPC and subscription cleanup; deny generic inputs and secondary-window commands. Full settings, logging, shortcut and AI activity features are outside this skeleton.
5. Add release-version consistency, protocol compatibility and test infrastructure using approved tools. Keep private configuration ignored, examples synthetic and unimplemented capabilities explicitly unavailable. No source save/delete, durable content recovery, model invocation or service export runs at scaffold startup.
6. Run affected checks, then the full six-command gate and a packaged synthetic two-command launch/permission smoke test. Record observed results and update run/contribution instructions. Leave every unavailable acceptance item unchecked.

## Handoff

E1/E10 and E11's profile-schema preparation in the [evidence register](implementation-handoff.md#capability-evidence-register) determine scaffold acceptance. [P1-M2](../archive/milestones/p1-m2/milestone-02-architecture-overview.md#architecture-clarifications-and-review) owns configuration/prompt clarifications, architecture review, interviews and background-AI design. Their contracts are inputs, not new P1-M3 design tasks.

Durable Authentic operations and Create Empty belong to P1-M4. Complete source/GUI/settings/logging/shortcut/activity features belong to P1-M5 or their later consuming milestone; real agent/review/progress acceptance belongs to P1-M7, exports to P1-M8 and final personal-release qualification to P1-M9. Create each feature milestone's overview/checklist and work plan before implementation, following the [handoff order](implementation-handoff.md#delivery-order-and-scope).
