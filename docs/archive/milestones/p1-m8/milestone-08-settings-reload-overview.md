# P1-M8: Settings and configuration reload overview

Status: Done

Add the client-owned `user_settings.json` store with guarded, section-scoped writes and recovery; persist the Writing Language; and give Settings a redacted server-config status with an explicit, validated, atomic `reload_settings`. The work plan is [milestone-08-settings-reload-architecture.md](milestone-08-settings-reload-architecture.md). Contracts: [user settings](../../../architecture/user-settings.md), [configuration schema and reload](../../../architecture/configuration-and-exports.md#configuration-schema-and-validation), [API table](../../../architecture/api-and-processes.md) and [Settings UX](../../../ux-gui.md).

## Entry conditions

- P1-M7 is Done. Read handoff rows E12 and P1-M8.
- [Owner decisions](milestone-08-settings-reload-architecture.md#owner-decisions) approve `sha2 =0.11.1` for `diary-client` and fix the language source. No other dependency or feature flag is approved; stop and ask if one is needed.

## Scope

| Included in P1-M8                                                                                                                                                                                     | Deferred to                                                       |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Client store for `user_settings.json` (strict JSON, 256 KiB, duplicate/unknown top-level keys rejected, section-level degrade, newer schema read-only, private atomic write)                          | Key-binding section contents and Settings table: P1-M12           |
| Commands `get_user_settings`, `update_user_settings` (one section + expected revision; explicit `recover` mode with private backup); no path or filesystem access in the WebView                      | AI runs that consume the locale: AI milestones                    |
| `writing.locale`: canonical BCP-47 validation, first-run initialisation from the Mac's preferred language via the guarded write, visible error when unresolved                                        | Packaged-app restart persistence: P1-M13/P1-M28                   |
| Server `reload_settings` (`POST /settings/reload`): full decode and validation first, keep the last config on failure, redacted field paths/codes, config generation, restart-required and busy rules | Source/agent/export users of the new generation: later milestones |
| Settings view: sidebar with Status (redacted, with the last reload result) and Writing Language; warnings and read-only banner                                                                        | SF Symbols, final styling: P1-M13                                 |
| Docs, handoff, changelog                                                                                                                                                                              |                                                                   |

## Completion checklist

Check an item only with a linked result naming the tested revision, command, observed outcome and limitations. Rows use the IDs of the [acceptance matrix](milestone-08-settings-reload-architecture.md#acceptance-matrix).

- [x] **S01 — Load fallbacks:** Missing file gives defaults; corrupt, oversized, duplicate-key and unknown-top-level-key files give defaults plus a message; a bad section degrades only itself and its raw data is preserved; a newer schema is read-only and every update to it is rejected untouched.
- [x] **S02 — Guarded write:** An update takes one section and `expected_settings_revision`; a stale revision or changed file returns `conflict` without overwriting; untouched valid sections and invalid section payloads survive; in-memory state changes only after confirmed publication.
- [x] **S03 — Durable publish:** Directory 0700, file 0600, no symlink traversal, same-directory temp file, sync, atomic rename, directory sync; a failure before rename keeps the old file; a post-rename sync failure reports uncertain and forces a reload before the next save.
- [x] **S04 — Recovery:** Explicit `recover` mode backs up the original privately before replacing an invalid or newer-schema file with defaults; ordinary updates never do.
- [x] **S05 — Location and secrecy:** The store only uses Tauri's app config directory (never a server-config, workspace, runtime, source or export path); the WebView snapshot carries no path, token or credential; logs record load/save and error codes only.
- [x] **S06 — Writing Language:** First load with no preference stores the Mac's preferred language through the guarded write; a valid saved choice is never re-inferred; an unresolved language shows the actual error and the `en-US`/`ko-KR` shortcuts and requires a saved choice; an unsafe file shows the error and never overwriting an invalid/newer file; invalid tags are rejected.
- [x] **S07 — Server reload:** A valid change applies atomically and bumps the config generation; any failure keeps the last config and returns redacted field paths/codes; workspace, runtime, port and source type/root changes are reported restart-required; profile/export changes return `busy` while an operation uses them; no token, credential or private path is returned.
- [x] **S08 — Settings UI:** Status shows availability and the last reload result with a Reload action; Writing Language is editable and persists across a new App on the same store; warnings and read-only state are visible; errors are shown, not swallowed.
- [x] **S09 — Bridge and capabilities:** `get_user_settings`, `update_user_settings` and `reload_settings` are the only additions; the command manifest and capability file agree; inputs are validated before any write or request.
- [x] **S10 — Handoff:** Update API table, user-settings status, handoff rows E12/P1-M8, phase plan, roadmap, docs index and changelog; update `AGENTS.md` only if commands change.
- [x] **G01:** Pass `cargo fmt --manifest-path src-tauri/Cargo.toml --check`.
- [x] **G02:** Pass `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [x] **G03:** Pass `cargo test --manifest-path src-tauri/Cargo.toml`.
- [x] **G04:** Pass `npx prettier --check .`.
- [x] **G05:** Pass `npx eslint . --max-warnings 0`.
- [x] **G06:** Pass `npx vitest run`.

## Evidence record

Working tree on top of `82c25f5`, branch `main`, Apple Silicon macOS, 2026-11-05. Test runs went through the test-runner. Departures are in the [work plan](milestone-08-settings-reload-architecture.md#departures-made-during-implementation).

S01–S04 | `src-tauri/src/user_settings/tests.rs` (13): missing/corrupt/duplicate-key/unknown-key/wrong-version/oversized files fall back with `file_invalid` and stay byte-identical; newer schema is read-only and rejects updates untouched; a bad section degrades alone and survives an unrelated update; 0700 directory, 0600 file, no temp left; stale and externally changed revisions conflict without overwrite; fault before rename keeps the old file and is retryable; fault after rename is `uncertain` until `snapshot()` reloads; `Recover` writes one 0600 backup with the original bytes, then defaults | pass | Limit: faults are injected by a test hook, not a real disk failure; `fsync` is std `sync_all`.

S05 | Same file: snapshot JSON carries no path; symlinked directory and a 0644 file are `location_unsafe` and never followed or written; `settings.loaded/saved/failed` log only counts and error tags | pass | Limit: the app config directory is not compared with the server's roots at runtime (the client does not know them); the store is only ever constructed from `app_config_dir()`.

S06 | `canonical_tag` table (11 cases, extensions and private use rejected); first run stores the first valid tag once and never re-infers; unresolved gives `language_unresolved` and writes nothing; an invalid saved `writing` section is not overwritten. UI: `tests/settings-ui.test.tsx` first-run, restart on the same fake store, unresolved error with `en-US`/`ko-KR`, typed-tag rejection | pass | Limit: `navigator.languages` is mocked; real WKWebView values and packaged-app restart persistence remain for P1-M13/P1-M28.

S07 | `diary-infrastructure` `config::tests::reload_tests` (2): profile change applies, is `busy` while an operation runs, then applies; port change is `restart_required`; invalid file, `unknown` key and 0644 permissions are `rejected` with codes only and no secret text, and nothing is adopted. `diary-api` (4): no reloader is 503; applied bumps `config_generation` and swaps availability; rejected keeps generation 1 and returns only codes; an open session is required | pass | Limit: field paths are coarse areas, not individual keys; `busy` means any non-terminal job (there are no export or AI users yet).

S08 | `tests/settings-ui.test.tsx` (7): language flows above, conflict reload, recover only after confirmation, read-only banner disables saving, status list plus reload result and no token or path text | pass.

S09 | `tests/architecture.test.js` (manifest, handler, capability and `#[tauri::command]` lists agree: `reload_settings`, `get_user_settings`, `update_user_settings`); `tests/adapter.test.ts` (3): camelCase arguments, extra fields dropped, raw error text never forwarded | pass.

S10 | API contract note, user-settings contract, handoff rows E12/P1-M8, phase plan, roadmap, docs index and changelog updated in this change.

G01–G06 | `scripts/gate.sh` on the final working tree, 2026-11-05: all seven lanes PASS in one run (including the `tsc` check); an earlier G03 flake in `stalled_diagnostics` was diagnosed and fixed in the test, see the [work plan](milestone-08-settings-reload-architecture.md#departures-made-during-implementation) for details | session output, not retained in the repo. Earlier runs failed on an unused constant, a `react-hooks/set-state-in-effect` lint, a `useRef` type error and an ambiguous test query, plus a missing test import; all fixed and the gate rerun.
