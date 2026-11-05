# P1-M7: Authentic editing and autosave overview

Status: Done

Connect one shared Draft/Entry CodeMirror buffer to the P1-M6 Authentic routes through typed Tauri commands, with an ordered, IME-aware autosave queue, Create Empty, acknowledged save feedback, external-change handling and guarded date/close transitions. The work plan is [milestone-07-authentic-editing-architecture.md](milestone-07-authentic-editing-architecture.md). Contracts: [editor and client state](../architecture/editing-and-agents.md#editor-and-client-state), [autosave protocol](../architecture/domain-and-persistence.md#autosave-protocol), [startup and external changes](../architecture/domain-and-persistence.md#startup-and-external-changes) and [save feedback](../ux-gui.md).

## Entry conditions

- P1-M6 is Done. Nothing in `AGENTS.md` "Undecided" blocks this milestone. No new dependency or build tool is planned; stop and ask if one turns out to be needed.
- Read handoff rows E2 and P1-M7.

## Scope

| Included in P1-M7                                                                                                                                                                                       | Deferred to                                                                        |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Tauri commands `list_authentic`, `load_authentic`, `save_authentic`, `create_empty_authentic`, `refresh_authentic`, `get_operation`, with allowlisted DTOs and error codes that keep `current_revision` | Source, import, Trash, export commands: later milestones                           |
| Client save state machine (`clean`, `dirty`, `saving`, `save_failed`, `external_conflict`), buffer generation, one request in flight, 300 ms debounce, ordered acknowledgements                         | Review/paragraph-accept saves: later milestones                                    |
| One CodeMirror `EditorState` per date shared by the Draft and Entry tabs; IME composition guard; Cmd+S flush                                                                                            | Command palette, shortcut registry: P1-M12                                         |
| Create Empty button (disabled for an existing file or in-flight creation); the saved empty file reloads after restart                                                                                   | Explorer tree and saved-date listing UI: P1-M11 (a minimal date list is used here) |
| Timeout retry via `get_operation` with the original ID; retry, explicit discard and external-conflict reconcile                                                                                         | SF Symbols / native rendering: P1-M13                                              |
| Date change and window close wait for pending saves and are blocked by failures                                                                                                                         | Cancelling AI runs on close: with AI milestones                                    |
| Server invalidation hint: start the P1-M6 watcher and emit a path-free `authentic_changed` SSE event; client rescans through `refresh_authentic`/`load_authentic`                                       | Explorer reaction: P1-M11                                                          |
| Smoke: extend `scripts/smoke/authentic.py` for the event; docs and handoff updates                                                                                                                      | Packaged-app, iCloud, native WKWebView IME evidence: P1-M13/P1-M28                 |

## Completion checklist

Check an item only with a linked result naming the tested revision, command, observed outcome and limitations. Rows use the same IDs as the [acceptance matrix](milestone-07-authentic-editing-architecture.md#acceptance-matrix).

- [x] **B01 — Bridge commands:** The six Authentic commands validate dates and operation IDs, rebuild allowlisted DTOs, and return stable error codes (`stale_revision`, `external_change`, `exists`, `invalid_input`, `unavailable`) (P1-M6 conflicts carry no `current_revision`, so the client reloads); no body text, path or raw error reaches logs or the WebView error.
- [x] **B02 — Shared buffer:** Draft and Entry for one date are two labels over the one mounted editor, undo history and save queue; switching them preserves text and pending edits; there is only ever one editor.
- [x] **B03 — Ordered autosave:** Debounce 300 ms; at most one request in flight; unsent generations coalesce; each save carries the preceding acknowledged revision; an acknowledgement clears dirty only up to its echoed generation; an old or duplicate acknowledgement is ignored.
- [x] **B04 — IME and flush:** No save starts during an unfinished composition and one starts after `compositionend`; Cmd+S, date change and close flush immediately.
- [x] **B05 — Save status:** The toolbar shows clean, dirty, saving, failed and conflict from acknowledged revisions only; a slow acknowledgement never marks newer text saved.
- [x] **B06 — Create Empty:** Creates revision 1 with no snippet or agent, opens the editable buffer, is disabled for an existing file or in-flight creation, never clears text, and an `exists` result loads the file instead; after a server restart the empty file loads.
- [x] **B07 — Lost reply:** A timeout queries `get_operation` for the original ID and adopts the recorded revision; no second operation ID is created; an unknown ID (`not_found`) resends the same ID with the stored body (the server deduplicates).
- [x] **B08 — Failures:** `save_failed` retains the editor, offers retry and explicit discard (warns, reloads the last observed body) and blocks date change and close until resolved.
- [x] **B09 — External change:** An invalidation or `external_change`/`stale_revision` result reloads a clean buffer and resets undo; a dirty buffer is kept and shown as `external_conflict`; retry uses the current revision only after the user reconciles; discard reloads the file.
- [x] **B10 — Transitions:** Date change and close wait for in-flight saves, flush dirty text first, and stay put on failure; close is not delayed when clean.
- [x] **B11 — Server hint:** The watcher is started by `diary-server`; a burst of file changes produces one `authentic_changed` SSE event with no path or content; own saves do not cause a spurious conflict.
- [x] **B12 — Smoke:** `python3 scripts/smoke/authentic.py` still passes and covers the event; a scripted client run (Vitest with a fake bridge) covers create, type, restart-reload and conflict.
- [x] **B13 — Handoff:** Update API table (remove "HTTP only until P1-M7"), handoff rows E2/P1-M7, phase plan, roadmap, docs index, changelog, `AGENTS.md` only if commands change.
- [x] **G01:** Pass `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
- [x] **G02:** Pass `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [x] **G03:** Pass `cargo test --manifest-path src-tauri/Cargo.toml`.
- [x] **G04:** Pass `npx prettier --check .`.
- [x] **G05:** Pass `npx eslint . --max-warnings 0`.
- [x] **G06:** Pass `npx vitest run`.

## Evidence record

Working tree on top of `2d0ba9f`, branch `main`, Apple Silicon macOS, 2026-11-05. Test runs went through the test-runner. Departures from the plan are in the [work plan](milestone-07-authentic-editing-architecture.md#departures-made-during-implementation).

B01 | `src-tauri/src/bridge/authentic/tests.rs` (3): inputs rejected before any connection (bad date/cursor/operation ID, 1 MiB + 1 body, bad revision); fixed `PUT /api/v1/authentic/{date}` with the original operation ID; `revision_conflict` and `outcome_uncertain` mapped, unknown code becomes `unavailable`, server text never forwarded. `tests/adapter.test.ts` (4) and `tests/architecture.test.js` (command list, capability file and manifest agree: 8 new commands) | pass | Limit: `current_revision` is not in the P1-M6 error body; the client reloads.

B02 | `tests/authentic-ui.test.tsx` "Draft and Entry show the same single editor" | pass | Limit: no per-date state is retained across date changes.

B03–B04 | `tests/saveQueue.test.ts` (14): 20 edits send once; edits during a save coalesce and the next send builds on the acknowledged revision; old, duplicate and unknown acknowledgements ignored; no tick during composition, send after `composition_end`; flush waits for composition and for the in-flight save | pass | Limit: composition is simulated by actions, not native WKWebView/IME (P1-M13/P1-M28).

B05–B06 | `tests/authentic-ui.test.tsx`: Create Empty writes revision 1, then the button is disabled with its reason, then one debounced autosave gives revision 2 and "Saved"; a saved file reloads after a new App on the same fake server and cannot be re-created. Server side: `python3 scripts/smoke/authentic.py` (create, 409 on repeat, save, restart, reload) | pass.

B07 | `saveQueue.test.ts`: timeout asks `get_operation` for the original ID, adopts `<sequence>:<sha256>`, mints no second ID; `not_found` resends the same ID; five unknown outcomes end in `save_failed`, and Retry asks about the same operation | pass.

B08–B10 | `authentic-ui.test.tsx`: a failed save keeps the text, blocks date change and close, Retry then succeeds and close proceeds; a slow save finishes against the original date before the new date loads; a dirty close flushes first; an external change reloads a clean buffer, conflicts a dirty one, and Discard (after confirmation) reloads the file | pass | Limit: the real window close path (`watch_close`/`finish_close`) compiles and is covered by the fake only; no packaged-app evidence.

B11 | `diary-api` `watcher_hint_is_one_pathless_event_per_open_stream`; `events.rs` decoder test; smoke script: an external touch of a real file produces `authentic_changed` with `data: {}` and no path or date | pass | The first smoke run caught the watcher being dropped at once (disjoint closure capture); fixed by moving the whole watcher into its thread.

B12 | `python3 scripts/smoke/authentic.py`: PASS authentic native smoke; Vitest scripted client above | pass.

B13 | API contract (new commands, event, error allowlist), phase plan, roadmap, docs index, handoff and changelog updated in this change.

G01–G06 | `scripts/gate.sh` on the final working tree, 2026-11-05: all six PASS; Vitest 55 tests; `python3 scripts/smoke/authentic.py` PASS | session output, not retained in the repo. Earlier runs failed: the architecture test (new command list), clippy `collapsible_if`, and a compile error (`Option<Channel>` is not a command argument); all fixed and the gate rerun. The Rust test total was reported inconsistently by the runner (251 and 223) and is not recorded here.
