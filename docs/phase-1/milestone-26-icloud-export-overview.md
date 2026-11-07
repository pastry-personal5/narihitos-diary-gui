# P1-M26: iCloud export overview

Status: Delivered (2026-11-07)

Register a real macOS File Provider adapter behind the P1-M25 `ExportAdapter` port, add the deferred recovery paths (export cancel, clear or abandon an `uncertain` export) and a minimal Export UI. The [work plan](milestone-26-icloud-export-architecture.md) defines stages and scenarios. This plan does not certify runtime behavior; real iCloud Drive proof is carried to P1-M28.

## Entry conditions

P1-M25 is delivered ([overview](../archive/milestones/p1-m25/milestone-25-export-coordination-overview.md)). Follow [iCloud projection](../architecture/configuration-and-exports.md#icloud-projection), [Export input and shared reconciliation](../architecture/configuration-and-exports.md#export-input-and-shared-reconciliation), [UX](../ux-gui.md) export status, and [handoff](implementation-handoff.md) E7.

## Owner decisions (2026-11-07)

- **O1 - Reach:** real adapter and production wiring, recovery paths (cancel, clear or abandon `uncertain`) and a minimal Export UI.
- **O2 - Evidence:** close on automated tests, a temp-directory smoke and the full gate. Real iCloud Drive proof moves to P1-M28; no live iCloud test and no human gate.

- **O3 - Abandon (Q2):** abandoning an `uncertain` export reads back first. A read-back equal to the saved revision resolves it as Succeeded with a receipt; otherwise a preview and one-use confirmation record "abandoned", delete the snapshot, release the reservation and reload block, leave the remote file untouched and add a Global Message.
- **O4 - Export UI:** toolbar button plus command-palette action for the selected date. It flushes the Authentic buffer first, then previews and confirms. Status appears in the toolbar and Global Messages.
- **O5 - Providers (Q3):** a root without ubiquity state is always available. A macOS privacy (TCC) denial is a distinct typed reason (`permission_denied`), never `cloud_unavailable`.
- **O6 - Cancel:** add `cancelled` to the export state (it mapped to `failed`). A pending confirmation is declined with Cancel; a running export cancels before `apply`, otherwise it finishes by read-back and reports normally.
- **Q1 - Dependencies (owner, 2026-11-07):** the owner approved adding dependencies, but none were needed: provider state comes from `st_flags` (`SF_DATALESS`) and the legacy `.name.icloud` placeholder, and no-clobber publication uses the existing `libc` `renamex_np`. If the ubiquity download-status key needs an `objc2-foundation` feature beyond the pinned set, stop and ask.

## Scope

- Stage A: macOS adapter `diary-infrastructure/src/export/icloud.rs`: path `<root>/YYYY/MM/YYYY-MM-DD.md`, coordinated availability and read checks, no-clobber create, expected-fingerprint replace via staged same-directory file, read-back, `cloud_unavailable` for non-materialized or unreadable states. Registered when `paths.icloud_entries_root` is set; `exports_available` becomes true.
- Stage B: recovery - `POST /jobs/{id}/cancel` for exports (no remote rollback), clear/abandon of an `uncertain` export (read-back first, then explicit confirmation), releasing the reservation and the reload block.
- Stage C: Export action, preview and confirm dialog, and status in the toolbar and Global Messages; UI uses the existing `start_export`/`confirm_export` commands plus new bridge commands for stage B.
- Stage D: scenarios, `scripts/smoke/export.py` extended for the real adapter on a temp directory, full gate, docs.

Out of scope: Notion (P1-M27), packaged and live iCloud proof (P1-M28), automatic download of evicted files, any claim that a local write means upload completed, new prompts or adapters.

## Open items

- **Q1:** `NSFileVersion` conflict versions would need a new feature; they stay a recorded gap, as in P1-M14.
- **Q4 - Staging files in iCloud:** a staged temp file in the destination directory may be synced by iCloud before it is renamed. Unverified; the plan removes it on every outcome and carries the question to P1-M28 live proof.

## Completion checklist

Check only when evidence exists.

### Plan review

- [x] R01 - Owner answered O1-O6 and Q1-Q3 (recorded above); changelog and AGENTS.md entries still to add. Evidence: Owner answered O1-O6 and Q1-Q3 (recorded above and in the [changelog](changelog.md)); AGENTS.md entry added.
- [x] R02 - Plan checked against current code (2026-11-07); corrections are in the [work plan](milestone-26-icloud-export-architecture.md#plan-review-corrections-2026-11-07).

### Stage A - adapter

- [x] C01 - Writes byte-identical Markdown to `YYYY/MM/YYYY-MM-DD.md` with no SQLite or private state (T01). Evidence: `icloud::tests::creates_exact_bytes_under_year_and_month`, `symlinked_parents_are_rejected`; smoke leg 5 compares the exported file with the workspace file byte for byte.
- [x] C02 - Create is no-clobber; replace requires the observed fingerprint; both stage in the same directory and verify by read-back (T02). Evidence: `create_is_no_clobber_and_replace_is_guarded`, `exclusive_rename_refuses_existing_target`; staged same-directory file removed on every outcome.
- [x] C03 - Download-needed, evicted or unreadable states are `cloud_unavailable`, never absence; an absent target after a receipt needs confirmation (T03). Evidence: `download_needed_is_unavailable_not_absent`, `legacy_placeholder_means_download_needed`; absent-after-receipt is `deleted` in the M25 decision table.
- [x] C04 - Concurrent external write is detected as divergence, staged input preserved, outcome `uncertain` or conflict (T04). Evidence: `late_accessor_never_writes_and_leftovers_are_cleaned`; replace re-checks the authorized observation inside the coordinated write (C12). Divergence ends `Failed` or `uncertain` by read-back, never `Succeeded`.
- [x] C05 - Production registers the adapter only when the root is set and valid; `exports_available` follows (T05). Evidence: `diary-server` registers `Icloud` only when `paths.icloud_entries_root` canonicalizes to a directory (`a_missing_root_registers_nothing`); `exports_available` now follows `Exports::available`.

- [x] C12 - `apply` receives the expected fingerprint and refuses a changed target atomically inside the coordinated write (T04). Evidence: `ExportAdapter::apply` takes the expected `Observation`; `create_is_no_clobber_and_replace_is_guarded`.
- [x] C13 - Unavailability carries a typed reason (`download_needed`, `unreadable`, `permission_denied`, `coordination_failed`); a timed-out coordination never writes late (T03). Evidence: `Unavailable` enum (`download_needed|unreadable|permission_denied|coordination_failed`) on `Observation`, `ExportError::CloudUnavailable(reason)` and the API `reason` field; `permission_denied_is_distinct`. Accessors run on a worker with a deadline and a late accessor never writes.

### Stage B - recovery

- [x] C06 - Export cancel (new `cancelled` state) stops before `apply`, never rolls back remote state, and a post-apply cancel finishes by read-back (T06). Evidence: `export::tests::cancel_declines_a_pending_confirmation_and_releases_the_destination`, `cancel_before_apply_stops_without_writing`, `uncertain_exports_are_not_cancellable`; `POST /jobs/{id}/cancel` dispatches exports; smoke leg 5.
- [x] C07 - An `uncertain` export resolves by read-back match (Succeeded with receipt) or confirmed abandon; reservation and reload block are released (T07). Evidence: `abandon_resolves_by_read_back_when_the_write_landed`, `abandon_needs_a_one_use_confirmation_and_releases_the_block`; routes `POST /exports/{id}/abandon` and `/abandon/confirmation`; smoke leg 5 (kill, restart, abandon).

### Stage C - UI

- [x] C08 - Export action, preview with confirmation, pending/succeeded/conflict/uncertain status and `cloud_unavailable` message, with accessible names and keyboard path (T08). Evidence: `tests/export.test.ts` (flush then acknowledged revision, confirmation, decline, typed failures, uncertain/abandon); `ExportDialog` uses `ModalDialog` (focus starts on Cancel), toolbar button and `export.start` palette action with a reason. Unavailable reasons share one message per code (the reason is in the API body).

### Stage D - proof and docs

- [x] C09 - T01-T08 pass; `python3 scripts/smoke/export.py` passes on a temp directory. Evidence: `python3 scripts/smoke/export.py` PASS (fake legs 0-4, real adapter leg 5 on a temp directory).
- [x] C10 - Full gate passes (`scripts/gate.sh`). Evidence: `scripts/gate.sh` G01-G07 all passed on 2026-11-07.
- [x] C11 - Architecture status note, handoff E7, phase plan, roadmap, changelog, docs index, README (user-visible) and AGENTS reflect delivered behavior and the no-live-iCloud limit. Evidence: Architecture status note, handoff E7, phase plan, roadmap, changelog, README and AGENTS.md updated.

## Implementation review follow-up (2026-11-07)

- [x] V01 - Review adapter path, staging and timeout safety; add regression coverage for confirmed defects. Evidence: native regression tests listed below passed in the full gate.
- [x] V02 - Review cancellation/recovery and UI outcome reporting; add regression coverage for confirmed defects. Evidence: controller, dialog keyboard/focus and App dirty-save integration tests passed (340 frontend tests in the full gate).
- [x] V03 - Run the temp-directory export smoke and full gate after fixes; record evidence and remaining limits. Evidence: strengthened `python3 scripts/smoke/export.py` PASS and final `scripts/gate.sh` G01-G07 all PASS on 2026-11-07; live iCloud and packaged evidence remain P1-M28.

### Review findings and corrections

- **File safety:** exclusive, owner-only staging prevents a pre-existing symlink or hard link from truncating another file. Remote reads reject symlinks and non-regular files and stop at the snapshot size bound. Parent permission errors retain `permission_denied`.
- **Deadline and concurrency:** observation cannot delete an active stage; publication rechecks abandonment and the remote fingerprint after staging. At most one native worker per adapter can remain blocked after a deadline. Tests: `staging_symlink_never_truncates_another_file`, `target_symlinks_are_unreadable_even_when_their_contents_match`, `oversized_remote_files_are_unreadable`, `observation_does_not_clean_up_a_stage_while_publication_holds_access`, `timeout_or_drift_during_staging_prevents_publication`, `timed_out_workers_bound_subsequent_native_calls`.
- **Cancellation:** a cancellation arriving during observation takes precedence over parking for drift, and finished workers remove pending cancellation flags. The UI waits for the authoritative operation outcome instead of announcing a cancellation after a failed request or a racing successful export.
- **UI recovery:** dirty buffers can invoke Export and are flushed before the saved revision is loaded. Running exports remain tracked beyond one minute. Concurrent drift and retained reservations after reload re-preview the original operation and revision. Recovery dialogs identify the original entry date. Tests: `tests/export.test.ts`, the keyboard/focus test in `tests/export-ui.test.tsx`, and the dirty-buffer export integration in `tests/authentic-ui.test.tsx`.
- **Built-server smoke:** the real adapter now explicitly verifies no-op without republishing, confirmed recreation after external deletion and exact exported bytes. Kill/restart must produce `uncertain`; the recovery assertions can no longer silently skip that path.
- **Documentation:** the maintained architecture no longer claims production exports are disabled or cancellation is absent. No dependency, prompt, live provider access or product decision changed.

Native workers can remain blocked until Foundation returns; deadlines bound the caller and repeated calls cannot accumulate workers. Coordination remains cooperative, so arbitrary non-coordinating writes racing the final rename cannot be guaranteed lossless. Live File Provider behavior and packaged proof remain P1-M28 evidence limits.
