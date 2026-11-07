# P1-M25: Export coordination and confirmations work plan

Status: Delivered (2026-11-07)

Implementation stages and scenarios for the [overview](milestone-25-export-coordination-overview.md). The behavioral contract is [Export input and shared reconciliation](../../../architecture/configuration-and-exports.md#export-input-and-shared-reconciliation); this plan only sequences it and fixes module boundaries. Owner decisions O1–O3 are in the overview.

## Existing code to reuse

- `diary-application::operations`: `Identity`, `NewOperation`, `NewStep`, `OperationState`, `ResourceKey`, `MAX_RESOURCES`; business-input hashing (`hash_input`) and `operation_mismatch` classification.
- `diary-application::source_trash` and `diary-infrastructure` source trash (`0005_source_trash.sql`): the preview/continuation payload, step-ID plus expected-checkpoint model, one-use authorization fingerprints, unfinished-operation index and typed `get_operation` lookup object. Export mirrors these shapes rather than inventing new ones.
- `diary-application::authentic::AuthenticStore::load` for the acknowledged revision and bytes; `Revision{sequence, sha256}`.
- Durable reservations are `operation_resources` rows owned by the ledger (`diary-infrastructure/src/ledger.rs`: conflict lookup by `resource_key`, rows kept while an operation is `running|paused|uncertain`), as Trash uses them. The in-memory `Coordinator` in `diary-application` is only the session/job registry; do not reserve through it.
- `diary_domain::authentic::MAX_BODY_BYTES` (1 MiB) for the snapshot bound; the `fault-injection` cargo feature (diary-infrastructure, forwarded by diary-server) for the smoke binary.
- `diary-api`: per-feature route modules and session admission; `src-tauri/src/bridge/*` command and capability pattern; config handle for `icloud_entries_root` and `notion` presence.

## Module plan

| Layer          | New or changed file                                                                       | Responsibility                                                                                                                                                                        |
| -------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| application    | `diary-application/src/export.rs` (+ `export/tests.rs`)                                   | `Target`, `DestinationKey`, `Snapshot`, `Observation`, `Decision`, `ExportService`, `ExportAdapter` port, error mapping                                                               |
| infrastructure | `diary-infrastructure/src/export.rs`, `sqlite/0006_export.sql`                            | Operation/receipt/checkpoint/authorization store; atomic step admission; recovery scan                                                                                                |
| infrastructure | `diary-infrastructure/src/export/fake.rs` (`cfg(any(test, feature = "fault-injection"))`) | In-memory destination, wired into the server only under the existing `fault-injection` feature, with scripted drift, deletion, unowned occupant, lost reply, failure at each boundary |
| api            | `diary-api/src/exports.rs` (+ tests)                                                      | Two routes, body limits, error mapping, `export` object added to the typed operation dispatch (Stage C)                                                                               |
| bridge         | `src-tauri/src/bridge/export.rs`                                                          | `start_export`, `confirm_export` with one main-window permission and capability entry each                                                                                            |
| scripts        | `scripts/smoke/export.py`                                                                 | Built-server synthetic smoke on the fault-injection binary, like `review.py`                                                                                                          |

## Interfaces (decided here so stages agree)

- `Target` is `icloud | notion`. `DestinationKey` is `sha256` over length-prefixed `target, canonical root or root page ID, workspace id`. A changed root yields a different key; receipts are keyed by `(key, date)` and never matched across keys.
- `Snapshot{date, revision, sha256, bytes}` is read from `AuthenticStore::load` after the client's flush-before-admission (the generation/review path). Absent Authentic is an error (`nothing_to_export`). The snapshot bytes are stored privately for the operation so a retry always exports that revision (contract: a newer revision is a new action).
- `ExportAdapter` (port, injected): `observe(key, date) -> Observation{state: Absent|Present, ids, fingerprint, normalized_digest, availability}`; `apply(step) -> StepOutcome{Done|Failed|Unknown}`; `read_back(step) -> Observation`. Production wires no adapter in M25; `Target` with no adapter is `disabled`.
- `start(session, {operation_id, date, target, expected_revision}) -> Started|NeedsConfirmation(preview)|Succeeded(noop)|Disabled|Error`. `start` runs load, observe and the decision on a blocking worker under a `source_slots`-style permit (Trash precedent) and returns the outcome in the response; only an admitted `confirm` step runs `apply` in the background, observed through `get_operation`. A stale `expected_revision` is `stale_revision` (409). Operation hash covers kind, date, target, destination key, revision and snapshot digest, excluding session and token bytes.
- `confirm(session, {operation_id, step_id, expected_checkpoint, token}) -> Admitted{operation_id, step_id, admitted_checkpoint}`. Preview carries `{observed fingerprint, proposed actions, expires_in_ms, token}`; token TTL 60 s, one use, fingerprint stored (not the bearer). The bearer exists only in the preview response and transient server memory, as for Trash. After a restart, expiry or new session, calling `start` again with the same `operation_id` and identical inputs re-observes and issues a fresh preview for the same paused operation (hash match); changed inputs are `operation_mismatch`. Any successful settings reload invalidates outstanding previews (config generation).
- Decision table (C02): receipt equals snapshot and remote digest equals receipt → verified no-op; no receipt, absent target, no unresolved earlier write on the key → create; receipt exists and remote fingerprint equals last verified → update; remote changed, deleted, or unowned occupant → `needs_confirmation`; unresolved earlier write on the key → `uncertain`/blocked. Confirmation authorizes only the previewed replacement or recreation.
- State mapping: the durable `operations.state` CHECK allows only `running|paused|uncertain|succeeded|failed|cancelled` and is not changed (a CHECK change would rebuild the table). Job-level `needs_confirmation` is stored as durable `paused`; `queued` exists only in the in-memory job layer. `paused` and `uncertain` keep the durable destination reservation: resource key `export:<64-hex destination key>` (fits `ResourceKey`'s 256-byte printable-ASCII rule); conflicts are the ledger's existing lookup by `resource_key`. The unfinished-export index covers `kind = 'export' AND state IN ('running','paused','uncertain')`, like Trash's.
- Migration `0006_export.sql` (STRICT): `export_operations(operation_id PK→operations, target, destination_key, date, revision_sequence, revision_sha, snapshot_digest, snapshot BLOB CHECK (snapshot IS NULL OR length(snapshot) <= 1048576), renderer_version, api_version)`, `export_receipts(destination_key, authentic_id, date, revision_sha, projection_digest, remote_ids, remote_fingerprint, PRIMARY KEY(destination_key,authentic_id))` (keyed by `authentic_id` as the [persistence table](../../../architecture/domain-and-persistence.md#sqlite-responsibilities) specifies), `export_checkpoints(operation_id, chunk, digest, status, remote_id, checkpoint)`, `export_authorizations(fingerprint PK, operation_id, step_id, UNIQUE(operation_id, step_id))`, plus an unfinished-export index. No body is logged. The snapshot column is nullable and bounded to `MAX_BODY_BYTES` by a CHECK; it is set to NULL in the same transaction as any terminal transition (owner O5), so only `paused`/`uncertain`/`running` operations hold diary text. Digests, revision and the receipt remain.

## Stages and tasks

Each task is test-first: write the named scenario, see it fail, implement, see it pass.

### Stage A — application model (T01–T02)

1. T01 snapshot and key: only saved bytes accepted; absent Authentic; changed root gives a new key; same inputs give the same operation hash; mismatched inputs give `operation_mismatch`.
2. T02 decision table: every row above with the fake adapter, including no-op, create blocked by an unresolved earlier write, and update versus confirm.

### Stage B — persistence (T03–T07)

3. T03 migration and intent: upgrade from schema 0005, intent committed before the first adapter call, rollback at each DB boundary.
4. T04 reservations: two exports to one key serialize; paused and uncertain survive restart and block conflicting starts; an Authentic autosave proceeds during a slow adapter call.
5. T05 token and step admission: consume plus checkpoint atomically before the side effect; expired, reused and mismatched tokens refused; fingerprint, not bearer, stored.
6. T06 duplicate and lost reply: repeat confirmation after restart returns the recorded step; stale checkpoint and changed fingerprint need a new preview; concurrent confirmations advance once.
7. T07 crash matrix: kill before intent, after intent, after admission, after side effect, before receipt; recovery yields a recorded state or `uncertain` with read-back, never a second create/append.

### Stage C — transport (T08–T09)

8. T08 routes and bridge: schema, bounds, session mismatch, forged IDs, disabled target, capability grants, typed `export` lookup after restart and in a new session; no generic HTTP. Lookup plugs into the existing typed operation path (`source_trash::operation()` and the unfinished listing `GET /api/v1/operations`): add an export branch dispatching by `operations.kind` and include unfinished exports in that listing, reusing its `valid_query` bounds; response bounded to 4 KiB like Trash's.
9. T09 reload guard: `reload_settings` currently derives `busy` only from this session's non-terminal in-memory jobs, so a `paused`/`uncertain` export surviving a restart is invisible to it. Extend `busy` with a ledger query for unfinished `export` operations. `Guarded` stays all-or-nothing: the contract counts paused/uncertain work as in-flight for affected roots, credentials and destinations, and the plan keeps the coarse rule (any guarded change is `busy` while any export is unfinished; no `Guarded` split). Root change while idle starts a new namespace. Because clearing an uncertain export is deferred (O5), an uncertain fake-destination operation blocks guarded reloads until M26/M27 provide reconciliation; record this in the architecture status note.

### Stage D — proof and docs

10. Add `scripts/smoke/export.py` (create, noop, external change confirmation, duplicate confirmation, kill and restart recovery) on the fault-injection server binary. Run the full gate once. Update docs per C12.

## Plan review corrections (2026-11-07)

Checked against current code (R02). Recorded corrections:

1. **States:** durable states are `running|paused|uncertain|succeeded|failed|cancelled`; `needs_confirmation` maps to `paused`, `queued` is job-only. The migration stays additive.
2. **Reservations:** held by ledger `operation_resources` rows (`export:<destination key>`), not the in-memory `Coordinator`.
3. **Reload guard:** `busy` must include ledger-unfinished exports; the coarse all-or-nothing `Guarded` rule is kept.
4. **Fake destination:** `cfg(test)` alone is invisible to the built binary; use `cfg(any(test, feature = "fault-injection"))` and the existing `fault-injection` feature, built by the smoke like `review.py`. The gate (`clippy --all-targets`, `cargo test`) compiles the test cfg only; the feature build is exercised by the smoke. Adding the feature to the gate is a build-tool change needing owner approval, so it is not planned.
5. **Token delivery:** the bearer is returned only in the `start` response; a fresh preview for a paused operation is a same-ID `start`.
6. **Typed lookup:** extends the existing operation dispatch and unfinished listing (T08).
7. **Snapshot:** bounded by `MAX_BODY_BYTES` with a CHECK; body dropped at terminal state (O5). SQLite is the established private store for plans (`trash_plans`); no separate payload file.
8. **Receipt key:** `(destination_key, authentic_id)` per the persistence contract.
9. **Reachability:** `start_export`/`confirm_export` hold main-window permissions, so they are reachable from the WebView and return `disabled` while no adapter is registered; no UI calls them in M25.

## Owner follow-up decisions (2026-11-07)

- Coarse reload guard kept (O6); gate unchanged (O7).
- Events (O8): after each durable export state change, publish `job_changed` through the existing event path (state before notification). T08 adds a scenario: event follows the durable state, none on a rejected or duplicate confirmation, and `get_operation` resolves the same state after a missed event.
- No cancel route for exports (O9); `POST /jobs/{id}/cancel` on an export reports unsupported.

## Risks and notes

- The fake adapter defines the port; M26/M27 may need small port changes. Keep the port minimal and record changes in their plans.
- Real File Provider and Notion behavior is unverified here by design (E7/E8); do not claim destination support in docs.
- Crash recovery and token handling are the highest-risk logic; reuse the Trash patterns instead of new ones.
