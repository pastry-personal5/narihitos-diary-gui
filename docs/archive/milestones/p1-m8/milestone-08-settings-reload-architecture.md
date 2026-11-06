# P1-M8: Settings and configuration reload work plan

Status: Done

Implementation plan for the [P1-M8 overview](milestone-08-settings-reload-overview.md). It follows [user settings](../../../architecture/user-settings.md), the [configuration contract](../../../architecture/configuration-and-exports.md#configuration-schema-and-validation) and the inward dependency rule of [architecture](../../../architecture.md). Each task ends with passing tests and one commit. Run affected tests while iterating and the full gate once at the end.

## Placement

| Layer             | Location                                     | Adds                                                                                                           |
| ----------------- | -------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Application       | `diary-application`                          | `ReloadOutcome`, `ReloadIssue { path, code }`, `ConfigSource` port (load a validated snapshot)                 |
| Infrastructure    | `diary-infrastructure::config`               | Reload diff: classify restart-required vs. reloadable vs. busy-guarded; redacted issue paths                   |
| Server            | `diary-api`, `diary-server`                  | `ServerState` owns the current snapshot and generation; `POST /api/v1/settings/reload`; richer `GET /settings` |
| Tauri client      | `src-tauri/src/user_settings/` (new module)  | Store, revision, section merge, atomic publish, recovery; two client-only commands                             |
| Tauri bridge      | `src-tauri/src/bridge.rs`, `main.rs`         | `reload_settings` command; manifest and capability entries                                                     |
| TS port + adapter | `src/application/bridge.ts`, `src/adapters/` | `Bridge` methods and DTO validators                                                                            |
| UI                | `src/components/SettingsView.tsx`, `App.tsx` | Sidebar (Status, Writing Language), warnings, read-only banner                                                 |

## Owner decisions

Answered 2026-11-05 and recorded in the [changelog](../../../phase-1/changelog.md).

1. **Digest:** add the already-pinned `sha2 =0.11.1` to `diary-client` (approved; already in the notices as a `diary-infrastructure` dependency).
2. **Preferred language:** the WebView sends `navigator.languages`; Rust validates it as untrusted data. No `objc2` change.
3. **Unresolved language:** show the actual error and offer a two-entry list, `en-US` and `ko-KR`, as one-click choices. Nothing is saved until the user picks one or types a valid tag; the list is a convenience, not a default.
4. **Input:** free text validated as canonical BCP-47, plus the two shortcuts above.

## Tasks

### Task 1: Settings file model and loader

Files: `src-tauri/src/user_settings/{mod.rs,model.rs,load.rs,tests.rs}`.

```rust
pub struct Snapshot { pub revision: String, pub read_only: bool, pub warnings: Vec<Warning>, pub writing: Option<WritingLocale>, pub keybindings: serde_json::Value }
pub struct Warning { pub section: &'static str, pub code: &'static str }
pub fn load(dir: &Path) -> Loaded;           // never fails; degrades with warnings
pub fn revision_of(bytes: Option<&[u8]>) -> String; // "missing" for no file, else hex digest
```

Strict parse with a duplicate-key-detecting reader, 256 KiB cap, unknown top-level key rejection, `schema_version` 1 only (greater means read-only). Invalid section keeps its raw value for preservation. Tests: the S01 cases, each with a temp dir; a bad `writing` leaves `keybindings` intact.

### Task 2: Atomic guarded write and recovery

Files: `user_settings/{write.rs,tests.rs}`. Adds `sha2` to `src-tauri/Cargo.toml` (decision 1); regenerate notices.

```rust
pub enum Update { Section { name: Section, value: serde_json::Value }, Recover }
pub fn update(dir: &Path, expected: &str, change: Update) -> Result<Snapshot, UpdateError>;
pub enum UpdateError { Conflict, ReadOnly, Invalid, Uncertain, Failed }
```

Re-read and re-hash the file under the check, compare with `expected`, merge into the latest stored snapshot, serialize, write a 0600 temp file in the same directory, fsync, rename, fsync the directory. Open without following symlinks; create the directory 0700. `Recover` copies the original to a 0600 backup first. Tests: S02–S04 including stale revision, fault before rename (old file kept), fault after rename (`Uncertain`, next update requires reload), symlinked file/directory refused, modes.

### Task 3: Writing Language

Files: `user_settings/locale.rs`, tests.

`pub fn canonical_tag(input: &str) -> Option<String>` accepting a bounded BCP-47 shape (language, optional script/region/variants, no private-use or extensions beyond the contract); `pub fn init_writing(dir, preferred: &[String]) -> Result<Snapshot, InitError>` stores the first valid tag through Task 2's write, only when the section is missing and the file is valid and writable. Tests: S06, including `en_US` canonicalising to `en-US`, garbage rejected, an invalid or newer file untouched, a saved choice never replaced.

### Task 4: Client commands and bridge

Files: `src-tauri/src/main.rs` (`generate_handler!`), `src-tauri/src/bridge.rs`, capability file, `src-tauri/src/bridge/tests.rs`, `tests/architecture.test.js`.

`get_user_settings() -> SettingsDto`, `update_user_settings(section, value, expected_revision, recover: bool) -> SettingsDto | SettingsError { code }`, `reload_settings() -> ReloadDto`. DTOs are allowlisted and carry no path. Validate section name and value size before any write. Tests: S05, S09.

### Task 5: Server reload

Files: `diary-application` (types), `diary-infrastructure/src/config.rs` and `config/tests.rs`, `diary-api/src/lib.rs`, `tests.rs`, `diary-server/src/main.rs`.

```rust
pub struct ReloadIssue { pub path: String, pub code: &'static str }
pub enum ReloadOutcome { Applied { generation: u64 }, RestartRequired(Vec<ReloadIssue>), Busy, Rejected(Vec<ReloadIssue>) }
```

`ServerState` keeps the config path and the current snapshot behind a lock; reload re-runs `config::load`, diffs against the current snapshot, and swaps only on success. Immutable fields (workspace, runtime, port, source type/root) differing gives `RestartRequired`. Profile/export differences while the coordinator reports an affected in-flight operation give `Busy`. `GET /settings` gains `config_generation` and the last reload result. Tests: S07 with temp configs (valid change, broken YAML, root overlap, port change, credential change leaves no value in the response).

### Task 6: Settings UI

Files: `src/components/SettingsView.tsx`, `App.tsx`, `src/application/settings.ts`, `src/adapters/settingsDto.ts`, `tests/settings-ui.test.tsx`.

Sidebar with Status and Writing Language; Status shows source/profile availability, generation and the last reload result with a Reload button; Writing Language is a validated text input with the effective tag, plus `en-US`/`ko-KR` buttons shown when no language is resolved (decision 3); warnings and read-only banners; failures shown by code. Call `get_user_settings` on open; on first load without a preference, send the preferred languages (decision 2) to initialise. Tests: S08 with a fake bridge.

### Task 7: Docs and gate

Update the docs listed in S10, tick the checklist with evidence, run `scripts/gate.sh`, record departures below.

## Acceptance matrix

| ID  | Tests                                                 |
| --- | ----------------------------------------------------- |
| S01 | Task 1 loader tests                                   |
| S02 | Task 2 stale/merge/preservation tests                 |
| S03 | Task 2 fault-injection and mode/symlink tests         |
| S04 | Task 2 recovery tests                                 |
| S05 | Tasks 1, 4: location, snapshot and log content checks |
| S06 | Task 3, Task 6 first-run test                         |
| S07 | Task 5 reload tests                                   |
| S08 | Task 6 UI tests                                       |
| S09 | Task 4 bridge tests and `tests/architecture.test.js`  |
| S10 | Task 7 doc review                                     |

## Review focus

- Two Settings windows or processes saving different sections: one gets `conflict`, neither loses the other's data.
- A hand-edited file with an unknown key inside a section: preserved, not erased by an unrelated update.
- Reload while an export or job uses a profile: `busy`, config unchanged.
- Reload with a config file that became world-readable or a symlink: rejected, redacted, last config kept.
- Preferred language list that is empty or not a valid tag: error shown, no default invented.

## Departures made during implementation

This list overrides the tasks above.

- **Command surface.** The planned three commands are `reload_settings`, `get_user_settings` and `update_user_settings`; `get_settings` (already present) now also returns `config_generation` and `last_reload`. First-run initialisation is `update_user_settings` with section `writing_init` (value: preferred tags) and recovery is its `recover: true` flag, so no fourth command was needed.
- **Placement.** The store is `src-tauri/src/user_settings.rs` (+ `locale.rs`, `tests.rs`), reusing `diary_logging::private_fs::PrivateDir` for no-follow, owner-only access. The reload lives in `diary-infrastructure::config::ConfigHandle`, behind a `ConfigReloader` port in `diary-application`.
- **Reload shape.** Issues are `{path, code}` with `path` a coarse config area (`config`, `server`, `paths`, `sources`...), because `ConfigError` has no field path and parse errors cannot name a key without risking its value. `busy` means any non-terminal job. The server logs nothing for reload (no new server event codes).
- **Logging.** Three client event codes were added: `settings.loaded`, `settings.saved`, `settings.failed`.
- **Directory mode.** An existing settings directory that is not 0700 owned by the user is reported `location_unsafe` and read-only; it is never chmodded.
- **Dependency.** `sha2 =0.11.1` added to `diary-client` (owner decision 1).
- **Not done here.** No key-binding editor (P1-M12); packaged-app restart persistence and real WKWebView language values are unverified (P1-M13/P1-M28).
- **Gate evidence.** Final tree, one full `scripts/gate.sh` run on 2026-11-05: G01–G07 all pass; Vitest 65 tests in 10 files; the runner reported 287 Rust tests (its totals varied between runs and are not otherwise recorded). An earlier gate run failed G03 once on `diary-server/tests/stalled_diagnostics.rs` with `ConnectionReset`. Cause, reproduced under CPU load (1 of 300 runs): the test's wrong-token `POST` is rejected with a complete `401` before the server reads the body, and closing the socket with unread data can reset the connection while the client is still reading. This is independent of the settings work; the test client now accepts a reset that follows a complete response (headers plus `content-length` bytes). After the change: 0 failures in 600 runs under load. The server still closes without draining a rejected request body, which a real client could also observe as a reset.
