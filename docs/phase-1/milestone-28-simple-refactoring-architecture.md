# P1-M28: Simple refactoring work plan

Status: Done (2026-11-07)

Approach, module boundaries and sequencing for the [P1-M28 overview](milestone-28-simple-refactoring-overview.md). Behavior-preserving only: every step compiles and passes its crate tests before the next starts.

## Rules

- Move first, then clean. A move commit changes no logic; fixes (suppressions, duplicates) go in separate commits.
- Keep the layer rules in [architecture](../architecture.md): inward dependencies stay inward. A split never adds a cross-crate dependency.
- Keep error handling and validation in moved code. No new dependency (`Cargo.toml` and `package.json` unchanged).
- Tests move with the code they cover; test names do not change.

## Proposed boundaries

**`diary-infrastructure/src/export.rs`** (`Service`, `Inner`): `export.rs` keeps the public `Service`, the `Exports` impl and wiring. The `Inner` methods move into sibling modules under `export/`, grouped by the lifecycle they serve:

- `snapshot` / `preview`: `snapshot`, `issue_preview`, `repreview`, `publication`, `expected`, `row_expected`.
- `start`: `start`, `step`, `park`, `holder_blocks`, `adapter`, `key`.
- `confirm`: `confirm`, `succeed`, `why`.
- `run`: `spawn`, `run`, `cancel`.
- `recovery`: `abandon`, `abandon_confirm`, `recover`.
- `util` (hash, monotonic clock, uuid, store-error mapping) stays small.

**`diary-api/src/lib.rs`**: keep `ServerState`, `router` and the module list. Move `authenticate`/`trace`/error helpers to `middleware`, session routes to `session`, settings to `settings`, and jobs plus `events` to `jobs`.

**`diary-application/src/agent.rs`**: split along its existing seams into `agent/kind` (`AgentKind`, `KindSpec`), `agent/status` (`ProfileStatus`, `Reason`, `ProfileCheck`), `agent/error`, `agent/packet` (locale, style, generation and chat packets), `agent/run` (events, cancel token, `Run`, runner trait) and `agent/capability`. Constants stay in `agent.rs`.

**`src/components/App.tsx`** (one 900-line component): extract by concern, per open item Q1. Candidate hooks: connection lifecycle (lines ~204-290), shell layout and pane sizes, derived reasons (`createReason`, `generationReason`, `exportReason`), and the `handlers` table. `App` becomes composition and JSX.

**Test files**: `diary-api/src/tests.rs` splits into one module per feature route family (session, jobs, settings, and so on), with shared fixtures in `tests/common.rs`. `export/tests.rs` splits into create, confirmation, cancel/abandon and recovery.

## Suppressions

| Suppression                  | Where                                                    | Planned fix                                           |
| ---------------------------- | -------------------------------------------------------- | ----------------------------------------------------- |
| `result_large_err` (8 files) | `diary-api/src/*.rs`                                     | one boxed or aliased error type in `diary-api` (Q2)   |
| `too_many_arguments`         | `diary-application/src/lib.rs:257`, `source/scan.rs:103` | parameter struct                                      |
| `unused_self`                | `source/write.rs:184`                                    | make the method an associated function, or use `self` |

## Duplication

Stage A greps adapters, API routes, smoke scripts and UI controllers for helpers repeated three or more times, and lists them here before any merge. A merged helper lives in the lowest crate or module that all callers already depend on.

## Sequencing

1. Stage A: baseline and inventory (no code changes).
2. Stage B: `agent.rs`, then `export.rs`, then `diary-api/src/lib.rs`, then the two test files, then suppressions, then Rust duplicates. Run `cargo test -p <crate>` after each.
3. Stage C: `App.tsx`, then UI duplicates. Run `npx vitest run` after each.
4. Stage D: parity check, smokes, full gate, docs.

## Risks

- Large moves hide accidental logic edits. Mitigation: move-only commits and `git diff --color-moved` review.
- Renames (O5) touch many callers and doc links. Mitigation: rename in its own commit and grep docs for old paths.
- `result_large_err` may ripple through signatures; stop and report if it does (Q2).
