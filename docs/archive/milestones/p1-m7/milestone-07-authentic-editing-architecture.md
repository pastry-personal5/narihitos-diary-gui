# P1-M7: Authentic editing and autosave work plan

Status: Done

Implementation plan for the [P1-M7 overview](milestone-07-authentic-editing-overview.md). It follows the [autosave protocol](../../../architecture/domain-and-persistence.md#autosave-protocol), the [API contract](../../../architecture/api-and-processes.md) and the inward dependency rule of [architecture](../../../architecture.md). Each task ends with passing tests and one commit. Run affected tests while iterating and the full gate once at the end.

## Placement

| Layer             | Location                                     | Adds                                                                                              |
| ----------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Server process    | `diary-server`, `diary-api`                  | Start the P1-M6 watcher; publish `authentic_changed` on the existing SSE stream                   |
| Tauri bridge      | `src-tauri/src/bridge.rs`, `main.rs`         | Six Authentic commands, DTO structs, error-code passthrough                                       |
| TS port + adapter | `src/application/bridge.ts`, `src/adapters/` | `Bridge` methods, `authenticDto` validators                                                       |
| Client state      | `src/application/authentic/`                 | Pure save-queue reducer (`saveQueue.ts`), operation-recovery helper, no React or Tauri imports    |
| Editor            | `src/editor/`                                | Buffer registry: one `EditorState` per date, composition tracking, change notifications           |
| UI                | `src/components/`                            | Draft/Entry editor tab, toolbar Save status and Create Empty, minimal date list, transition guard |

## Owner decisions

Answered 2026-11-05 and recorded in the [changelog](../../../phase-1/changelog.md).

1. **Watcher event:** add payload-free `authentic_changed` and start the P1-M6 watcher now.
2. **Own writes:** the client ignores a reload that returns the revision it already acknowledged; no server suppression.
3. **Date navigation:** date input, Today and a plain `list_authentic` list until the P1-M11 Explorer.
4. **Errors:** Authentic commands return `AuthenticError { code, current_revision?, retryable }`.

## Departures made during implementation

This list overrides the tasks below.

- **No buffer registry.** One `CodeMirror` view is mounted for the component's lifetime; Draft and Entry are labels over it, and `view.setState` replaces the state (resetting undo) on load, reload and discard. Nothing keeps a state per date. `src/editor/authentic-editor.ts` replaces the planned `authentic-buffers.ts`.
- **Error shape.** `AuthenticError` is `{code, retryable}`: the P1-M6 error body has no `current_revision`, so after `revision_conflict`/`external_change` the client reloads and `keep_mine` waits for that reload.
- **Lost reply.** A `not_found` answer to `get_operation` resends the original operation ID with the stored body (the server deduplicates) instead of minting a new ID.
- **Close guard needs two more commands.** `watch_close` (registers a channel) and `finish_close` (release lease and destroy) let the WebView flush before Rust closes the window. Without a registered channel the window closes at once, as in the scaffold. A WebView that registers and then never answers cannot be closed from the window button.
- **Session glue.** `AuthenticSession` (`src/application/authentic/session.ts`) runs the reducer's effects; `useAuthentic` owns the editor, the session and date/close transitions. `ScratchEditor.tsx` is removed.
- **Watcher scope.** The watcher starts only if the Authentic tree exists when the server starts, outside recovery mode.
- **Own writes.** The client ignores a reload that returns the revision it already holds, and holds an invalidation that arrives during a save until the save settles (owner decision 2).

## Tasks

### Task 1: Server hint

Files: `diary-server/src/main.rs`, `diary-api/src/lib.rs` (event kind), tests in `diary-api/src/tests.rs`.

Start `watch` on the Authentic tree after recovery classification, outside recovery mode; on each coalesced signal publish `authentic_changed` through the existing event hub. Tests: a burst of three writes yields one event with no path or content; recovery mode starts no watcher; the event passes the SSE size and envelope checks. Extend `scripts/smoke/authentic.py` to assert the event after an external edit.

### Task 2: Tauri commands

Files: `src-tauri/src/bridge.rs`, `src-tauri/src/main.rs` (`generate_handler!`), `src-tauri/src/bridge/tests.rs`.

```rust
pub struct AuthenticRevision { sequence: u64, sha256: String }
pub struct AuthenticFile { date: String, exists: bool, revision: Option<AuthenticRevision>, body: String }
pub struct SaveAck { operation_id: String, revision: AuthenticRevision, buffer_generation: u64 }
pub struct AuthenticError { code: String, current_revision: Option<AuthenticRevision>, retryable: bool }
// commands: list_authentic(cursor), load_authentic(date), save_authentic(date, operation_id, expected_revision, body, buffer_generation),
// create_empty_authentic(date, operation_id), refresh_authentic(), get_operation(operation_id)
```

Validate date (`YYYY-MM-DD`), UUID operation ID, 1 MiB body and cursor before any request; build the fixed route yourself (no URL or method from the WebView). Parse the server error body into the allowlist; map unknown codes to `unavailable`. Grant the commands in the Tauri capability file with nothing broader. Tests with the existing fake server: each command's request shape; invalid input rejected before the network; `stale_revision` keeps `current_revision`; a body with a path or text in the error is not forwarded; timeout leaves the operation ID usable for `get_operation`.

### Task 3: TypeScript port and validators

Files: `src/application/bridge.ts`, `src/adapters/tauri-bridge.ts`, `src/adapters/validation.ts` (+ `tests/validation.test.ts`, `tests/adapter.test.ts`).

Add the six methods to `Bridge` and `authenticDto`/`saveAckDto`/`authenticErrorDto` that rebuild allowlisted objects and reject extra or malformed fields (sequence safe integer ≥ 1, sha256 64 hex, date shape). Tests: golden accept, each rejection, additive private fields dropped.

### Task 4: Save queue (pure)

Files: `src/application/authentic/saveQueue.ts` (+ `tests/saveQueue.test.ts`).

```ts
type SaveState =
  "clean" | "dirty" | "saving" | "save_failed" | "external_conflict";
interface QueueState {
  state: SaveState;
  generation: number;
  sentGeneration: number | null;
  acknowledged: Revision | null;
  composing: boolean;
  operationId: string | null;
  error?: AuthenticError;
}
type Action =
  | Edit
  | Tick /* debounce elapsed */
  | CompositionStart
  | CompositionEnd
  | Ack
  | Failure
  | Invalidated
  | Flush
  | Retry
  | Discard
  | Loaded;
function step(
  s: QueueState,
  a: Action,
): { state: QueueState; effects: Effect[] }; // Effect = send | getOperation | reload | scheduleTick
```

Reducer is pure; timers and I/O are effects run by the caller. Tests, each with exact assertions: 20 edits within 300 ms produce one `send`; edits while saving coalesce and the next send uses the acknowledged revision; ack for generation 3 with current generation 5 leaves `dirty`; old/duplicate ack ignored; `Tick` during composition sends nothing and `CompositionEnd` schedules it; `Flush` sends immediately and not while one is in flight (it queues); timeout emits `getOperation` with the same ID and never a new one; `stale_revision` while clean emits `reload`, while dirty becomes `external_conflict`; `Discard` after failure reloads; `Retry` after conflict is refused until the user has reconciled (a `Reconciled` action).

### Task 5: Buffer registry and editor tab

Files: `src/editor/authentic-buffers.ts`, `src/components/AuthenticEditor.tsx`, replacing `ScratchEditor` use (+ `tests/client.test.tsx`).

`getBuffer(date)` returns the single `EditorState` for a date; the Draft and Entry tabs both render it in the one active `EditorView`. An update listener sends `Edit`, and composition events send `CompositionStart/End` (CodeMirror `view.composing`). Replacing the body on reload resets history. Tests: both tabs show the same text and undo stack; typing in Draft appears in Entry; tab switch keeps selection; unmount destroys the view.

### Task 6: Toolbar, Create Empty, date navigation and guards

Files: `src/components/App.tsx`, `src/components/Toolbar.tsx`, `src/application/authentic/transition.ts` (+ tests).

Save status text from `QueueState`; Create Empty enabled only when the loaded date has `exists: false` and no creation is in flight, with a disabled reason; its handler calls `create_empty_authentic`, and on `exists` loads the file. `guardTransition(queue)` returns `proceed | wait | blocked`; date change and `onCloseRequested` await it (flush, wait for the in-flight save, block on `save_failed`/`external_conflict` with a retry/discard prompt). Minimal date input and list per decision 3. Tests (fake bridge): Create Empty then type persists; button disabled for existing; date change during a slow save waits and does not retarget the save to the new date; failed save blocks close and date change until retry succeeds or discard; clean close is immediate; restart (new fake server state) reloads the empty file.

### Task 7: Docs and handoff

Update the API table, handoff rows E2/P1-M7, phase plan status, roadmap, docs index, changelog, then fill the evidence record in the overview, run `scripts/gate.sh` and the smoke script, and check the overview boxes.

## Acceptance matrix

| ID  | Evidence                                                                                            | Task |
| --- | --------------------------------------------------------------------------------------------------- | ---- |
| B01 | Bridge tests (validation before network, error allowlist, `current_revision`), capability file diff | 2, 3 |
| B02 | Buffer registry and client tests (shared state and undo)                                            | 5    |
| B03 | `saveQueue` tests (coalescing, ordering, generation-limited ack, duplicate ack)                     | 4    |
| B04 | `saveQueue` composition tests; editor test dispatching composition events                           | 4, 5 |
| B05 | Toolbar tests: slow ack does not mark newer text saved                                              | 6    |
| B06 | Create Empty client tests and the smoke restart-reload                                              | 6, 1 |
| B07 | `saveQueue` timeout test; bridge test that the original ID is reused                                | 2, 4 |
| B08 | Guard and failure tests (retry, discard warning, blocked transition)                                | 4, 6 |
| B09 | `saveQueue` invalidation tests; server event test; smoke external edit                              | 1, 4 |
| B10 | Transition tests                                                                                    | 6    |
| B11 | Server burst test and smoke                                                                         | 1    |
| B12 | Smoke run output; Vitest scripted client                                                            | 1, 6 |
| B13 | Doc diff and link check                                                                             | 7    |

## Known limits to record

- Mocked IME events are not native WKWebView composition evidence; that is P1-M13/P1-M28.
- A debounce or fake bridge is not proof of durability; durable-save evidence remains the P1-M6 matrix plus the smoke script.
- Client-only edits are lost on forced termination.
