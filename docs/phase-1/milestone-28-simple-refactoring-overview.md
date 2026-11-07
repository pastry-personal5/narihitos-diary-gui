# P1-M28: Simple refactoring overview

Status: Done (2026-11-07)

Split the oversized modules into focused ones and remove two kinds of repeated friction (lint suppressions and copy-pasted helpers), without changing behavior, contracts or dependencies. The owner interview on 2026-11-07 fixed the scope below.

## Entry conditions

P1-M25–P1-M27 are Delivered. No feature work is in flight, so file moves cannot conflict with other changes.

## Owner decisions (2026-11-07)

- **O1 - Split threshold:** split production modules over 900 lines. Smaller cohesive files stay as they are.
- **O2 - Also in scope:** fix the root cause of lint suppressions (`too_many_arguments`, `unused_self`, `result_large_err`) and merge duplicated helpers (three or more copies). Dead-code removal and `shortcut:` debt are out of scope.
- **O3 - Test files:** split `diary-api/src/tests.rs` and `diary-infrastructure/src/export/tests.rs` by feature, keeping every test name and assertion.
- **O4 - Evidence:** the full gate, plus before/after test-count and test-name parity for Rust and Vitest, plus the synthetic smokes for touched areas (`export.py`, `agent.py`, `review.py`).
- **O5 - Names:** public items and module paths may be renamed where it improves clarity; update all callers, docs and links in the same change.

## Scope

In scope, with current sizes:

| Module                                     | Lines | Direction                                                  |
| ------------------------------------------ | ----: | ---------------------------------------------------------- |
| `diary-infrastructure/src/export.rs`       |  1132 | split the 870-line `impl Inner` by responsibility          |
| `diary-api/src/lib.rs`                     |  1087 | move handlers, auth and job/event routes into submodules   |
| `diary-application/src/agent.rs`           |   932 | split by concept: kinds/status, packets, run/cancel, views |
| `src/components/App.tsx`                   |   957 | extract hooks and the handler table from the 900-line body |
| `diary-api/src/tests.rs`                   |  1273 | per-feature test modules                                   |
| `diary-infrastructure/src/export/tests.rs` |  1014 | per-feature test modules                                   |

Plus the suppressions and duplicated helpers found by the stage A inventory. Details are in the [work plan](milestone-28-simple-refactoring-architecture.md).

Out of scope: behavior, schema, API, prompt or dependency changes; dead-code removal; files under 900 lines; UI visuals; `docs/archive`.

## Open items

- **Q1 - App.tsx style:** recommended default is extracting custom hooks (`useConnection`, `useShellLayout`, `useHandlers`) and keeping `App` as composition. Alternative: child components. Confirm before stage C.
- **Q2 - `result_large_err`:** recommended default is boxing or aliasing the error once in `diary-api` so the eight file-level allows go away. If that changes a public signature widely, report it first.

## Completion checklist

### Plan review

- [x] R01 - Owner confirms Q1 and Q2 or accepts the defaults. Owner asked to implement; the recommended defaults for Q1 (hooks) and Q2 were applied, and Q2 fell back as recorded in B05.

### Stage A - inventory and baseline

- [x] A01 - Record the Rust test count and names (`cargo test -- --list`) and the Vitest count and names before any move. Baseline: 748 Rust tests, 333 Vitest tests.
- [x] A02 - List duplicated helpers with three or more copies and each suppression with its root cause; record them in the work plan. Inventory: bridge `invalid`/`uuid`/`digest`/`date_shape` copies, a second `valid_uuid`, two identical Entry `flush` closures plus a third in generation navigation.

### Stage B - Rust splits

- [x] B01 - Split `export.rs` into modules with one responsibility each; no module over 600 lines. `export.rs` 1132 → 392 lines; `snapshot`, `start`, `confirm`, `run`, `recovery` modules (largest 258).
- [x] B02 - Split `diary-api/src/lib.rs`; the router and `ServerState` stay in `lib.rs`. `lib.rs` 1087 → 419 lines; `guard`, `sessions`, `settings`, `jobs` modules.
- [x] B03 - Split `agent.rs` by concept; re-exports keep callers compiling or are updated. `agent.rs` 932 → 31 lines; `kind`, `status`, `error`, `packet`, `run`, `capability`; `diary_application::agent::*` paths unchanged via re-exports.
- [x] B04 - Split the two Rust test files by feature; test names are unchanged. `diary-api` tests: 1273 → 527 lines plus `authentic_routes`, `diagnostics`, `reload`, `replay`; export tests: 1014 → 131 lines plus `start`, `recovery`, `concurrency`, `cancel_abandon`, `notion`. Leaf test names unchanged.
- [x] B05 - Remove the `too_many_arguments`, `unused_self` and `result_large_err` suppressions by fixing their causes, or record why one must stay. `too_many_arguments` fixed with `Admission` and `Bounds` structs; `unused_self` allow removed. The eight `result_large_err` allows became one documented crate-level allow: boxing the axum `Response` touches about 100 call sites for no behavior gain (Q2 fallback).
- [x] B06 - Merge the Rust duplicated helpers found in A02. Bridge `validate.rs` replaces 7 `invalid`, 3 `uuid`, 3 `digest` and 3 `date_shape` copies; `diagnostics::valid_uuid` re-exports the domain function. `stamp::valid_uuid` is stricter (v4) and stays.

### Stage C - UI

- [x] C01 - Split `App.tsx` per Q1; no UI file in the split over 600 lines. `App.tsx` 957 → 512 lines; hooks `useAppControllers`, `useConnection`, `useCloseRequest`, `useAppActions`. `NavigationModel.guardDateChange` replaces a direct property write the React lint rejects.
- [x] C02 - Merge the UI duplicated helpers found in A02. The three Entry-settle closures are now one `settleAuthentic` in `generation/navigation.ts`.

### Stage D - proof and docs

- [x] D01 - Test counts and names match the A01 baseline. 748 Rust and 333 Vitest tests before and after; leaf names identical (33 export tests changed module path only).
- [x] D02 - `python3 scripts/smoke/export.py`, `agent.py` and `review.py` pass. `export.py`, `agent.py` and `review.py` passed.
- [x] D03 - The full gate passes (`scripts/gate.sh`). `scripts/gate.sh` passed all seven commands.
- [x] D04 - Docs that cite moved paths are updated; P1-M28 status, phase plan, roadmap and changelog are updated. No doc cited a moved path; phase plan, changelog and this record are updated.
