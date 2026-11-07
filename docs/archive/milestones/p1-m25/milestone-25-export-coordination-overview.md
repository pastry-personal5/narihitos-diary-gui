# P1-M25: Export coordination and confirmations overview

Status: Delivered (2026-11-07)

Add the destination-neutral export foundation: an immutable saved-revision snapshot, durable receipts and checkpoints, destination reservations and deduplicated confirmation steps, proven against a fake destination. No real iCloud or Notion contact happens here. The [work plan](milestone-25-export-coordination-architecture.md) defines stages and scenarios; this plan does not certify runtime behavior.

## Entry conditions

P1-M24 is delivered; its [open review findings](../p1-m24/milestone-24-open-review-findings.md) are tracked separately (O4) and none touch export code. Follow [Export input and shared reconciliation](../../../architecture/configuration-and-exports.md#export-input-and-shared-reconciliation), [wire types and preconditions](../../../architecture/api-and-processes.md#wire-types-and-preconditions) (step IDs, checkpoints, operation lookup), [jobs and coordinators](../../../architecture/api-and-processes.md#jobs-events-and-recovery), [UX](../../../ux-gui.md) export status, and [handoff](../../../phase-1/implementation-handoff.md) E7/E8. The source-Trash preview/step/authorization design (P1-M15) is the precedent for token and checkpoint handling.

## Scope

- Stage A: domain/application export model — target, destination key, snapshot, observation, decision table, state machine, confirmation token and step types; `ExportAdapter` port.
- Stage B: infrastructure — additive migration, operation/receipt/checkpoint/authorization store, destination reservations in the ledger, crash-boundary recovery.
- Stage C: fake destination adapter with fault injection (test-only), server routes `POST /exports` and `POST /exports/{id}/confirmation`, typed `export` lookup through `get_operation`, bridge commands `start_export` and `confirm_export` with capability entries, config-reload busy rule.
- Stage D: scenarios, `scripts/smoke/export.py`, full gate, documentation.

Out of scope: export cancellation (O9), real iCloud adapter (P1-M26), Notion adapter (P1-M27), Export UI (no UI calls the commands in this milestone; they are granted to the main window but report the target as disabled until M26 wires a real adapter), clearing or abandoning an uncertain export (O5), packaged proof, new prompts or adapters. Without a registered adapter, `start_export` reports the target as disabled.

## Owner decisions (2026-11-07)

- **O1 — Reach:** backend plus server routes and bridge commands, no UI.
- **O2 — Evidence:** close on automated tests, a synthetic built-server smoke and the full gate. No human exit gate; real-destination proof belongs to P1-M26/M27.
- **O3 — Dependencies and schema:** the owner allows new dependencies, schema changes (with migration) and breaking compatibility. This plan needs no new dependency; one additive migration only.

- **O4 — M24 findings:** the five open P1-M24 review findings are tracked separately and do not block M25.
- **O5 — Uncertain exit and snapshot retention:** reconciliation or abandonment of an `uncertain` export is deferred to P1-M26/M27, where a real read-back adapter exists; until then it keeps its reservation and blocks guarded settings reloads. The snapshot body is deleted in the same transaction as any terminal state; digests, revision and receipt remain, and non-terminal operations keep the body.
- **O6 — Reload scope:** keep the coarse all-or-nothing `Guarded` rule (no per-destination split).
- **O7 — Gate:** no change to `scripts/gate.sh`; the `fault-injection` build is covered by `scripts/smoke/export.py` only.
- **O8 — Progress:** export state changes are published as `job_changed` events (state first, then notification, per the events contract) in addition to `get_operation` polling.
- **O9 — Cancellation:** no export cancel path in M25; deferred to P1-M26 with a real adapter.

## Completion checklist

Check only when evidence exists. Scenario IDs refer to the work plan.

### Plan review

- [x] R01 — Owner answered O1–O3 (2026-11-07); recorded here and in the [changelog](../../../phase-1/changelog.md).
- [x] R02 — Plan checked against current code and contracts; corrections recorded in the [work plan](milestone-25-export-coordination-architecture.md#plan-review-corrections-2026-11-07).
- [x] R03 — Owner answered O4–O5 (2026-11-07).

### Stage A — model

- [x] C01 — Snapshot is the acknowledged saved revision's immutable bytes and digests; unsaved buffers, candidates and sources are rejected as inputs; destination key includes canonical root plus workspace identity, so a changed root never reuses receipts (T01). Evidence: `export::tests::only_saved_revisions_are_exported_and_inputs_are_bound`, `a_changed_root_is_a_new_namespace_and_never_reuses_receipts`.
- [x] C02 — Decision table is total: verified no-op, first create (absent target, no unresolved earlier write), update (remote equals last verified receipt), `needs_confirmation` (changed, deleted or unowned occupant) and `uncertain` (unresolved earlier write reserves the target) (T02). Evidence: `diary-application` `decision_table_is_total` and the infrastructure `create_noop_and_update_follow_the_receipt` / `drift_deletion_and_unowned_targets_need_one_use_confirmation`.

### Stage B — persistence and reservations

- [x] C03 — Intent (revision, digests, renderer/API versions, destination) is persisted before any destination call; receipts, per-chunk checkpoints and remote fingerprint live in SQLite; the snapshot body is bounded to 1 MiB and deleted with any terminal transition (O5); `needs_confirmation` maps to durable `paused` with no CHECK change; migration is additive and tested from the prior schema (T03). Evidence: `sqlite::m25_upgrades_schema_five_keeping_rows_and_adding_export_tables`; intent committed by `admit_export` before `observe`/`apply`; snapshot cleared by `finish_export` (asserted in `create_noop_and_update_follow_the_receipt`). The bound is the rendered file (body bound plus 64 KiB front matter), not the body alone.
- [x] C04 — Destination reservations (`operation_resources`, key `export:<destination key>`) serialize steps; paused and uncertain work keeps a durable reservation across restart; export never reserves the live Authentic resource, so autosave is not blocked by slow remote work (T04). Evidence: `paused_exports_hold_the_destination_across_restart_but_not_authentic_saves`.
- [x] C05 — Confirmation token (60 s, one use) binds operation, session, saved revision, destination, observed fingerprint and proposed actions; the token consume and checkpoint advance commit atomically before the step's side effect; the bearer is never stored (T05). Evidence: `drift_deletion_and_unowned_targets_need_one_use_confirmation` (wrong session, wrong checkpoint, consumed token); only the SHA-256 fingerprint is stored (`export_authorizations`).
- [x] C06 — Duplicate or lost-reply confirmation returns the recorded step state before token expiry or consumption checks and never repeats the step; a stale checkpoint or changed fingerprint requires a new preview (T06). Evidence: same test (duplicate returns the recorded step), `concurrent_confirmations_advance_once`.
- [x] C07 — Crash at every boundary (before intent, after intent, after step admission, after side effect, before receipt) recovers to `uncertain` or a recorded state, never a duplicate create/append (T07). Evidence: `crash_before_intent_records_nothing`, `crash_before_any_side_effect_asks_for_a_fresh_confirmation`, `crash_after_the_side_effect_is_proven_by_read_back_not_replayed`, `unprovable_outcomes_become_uncertain_and_block_the_destination`, `a_lost_reply_is_reconciled_by_read_back_without_a_second_write`.

### Stage C — transport

- [x] C08 — Export state changes publish `job_changed` events after the durable state is written, and replay/resync behaves as for other jobs (O8); `POST /exports` and `POST /exports/{id}/confirmation` and bridge `start_export`/`confirm_export` validate identity, bounds, session and capability grants; a typed `export` object resolves through `get_operation` after restart and a new session; disabled targets return a typed error (T08). Evidence: `diary-api` `exports::tests` (session, bounds, unknown fields, disabled, one `job_changed` per distinct state, no event on a duplicate confirmation, cancel `unsupported`, typed lookup and listing) and `export::tests::lookup_pages_and_disabled_targets`. Lookup after a restart and in a new session is exercised by the smoke and the restart tests.
- [x] C09 — Settings reload with an in-flight, paused or uncertain export on the affected destination or credential returns `busy`, including operations that survived a restart (ledger query, not only session jobs); a changed root creates a new namespace (T09). Evidence: `exports::tests::reload_is_busy_while_a_ledger_export_is_unfinished`, `lookup_pages_and_disabled_targets` (unfinished export visible without an adapter).

### Stage D — proof and docs

- [x] C10 — T01–T09 automated matrix passes against the fake destination, including lost reply, restart, external change, deletion, unowned occupant and concurrent confirmation. `python3 scripts/smoke/export.py` passes on the real server binary. Evidence: `python3 scripts/smoke/export.py` passed on the fault-injection server binary (create, no-op, external change, duplicate confirmation, disabled target, kill after write recovered by read-back, kill before write left `uncertain`).
- [x] C11 — Full gate passes (`scripts/gate.sh`: fmt, Clippy `-D warnings`, cargo test, Prettier, ESLint zero warnings, Vitest, tsc). Evidence: `scripts/gate.sh` G01–G07 all passed on 2026-11-07.
- [x] C12 — README (if user-visible), architecture status notes, handoff E7/E8, phase plan, roadmap, changelog, docs index and AGENTS reflect delivered behavior and the explicit no-real-destination limit. Evidence: README unchanged (not user-visible); architecture status note, handoff, phase plan, roadmap, changelog, docs index and AGENTS.md updated.
