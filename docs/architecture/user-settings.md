# User settings storage

Status: Active

This contract defines persistent, GUI-editable user settings. They are stored apart from the private server config (`config/global_config.yaml`), which stays read-only from the GUI and holds paths, sources, agents and credentials.

## Separation from config

|           | Server config                                               | User settings                                                                                                                                                                              |
| --------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| File      | `--config PATH`, conventionally `config/global_config.yaml` | `user_settings.json`                                                                                                                                                                       |
| Location  | Chosen by the owner at server launch                        | Tauri app config directory for the app identifier (`~/Library/Application Support/<identifier>/`), never inside `entries_root`, `runtime_root`, sources, exports or the config's directory |
| Owner     | Server, loaded at launch, explicit `reload_settings`        | Tauri client bridge                                                                                                                                                                        |
| Edited by | Owner in a text editor                                      | The user in Settings                                                                                                                                                                       |
| Content   | Paths, sources, profiles, credentials                       | User preferences: key bindings, AI writing language and generation count                                                                                                                   |

User settings never hold credentials, tokens, paths, source or agent profiles, or diary content, and the server neither reads nor writes their file. The chosen writing locale is sent as validated request data for each AI run. Because settings are per-user and per-machine, they are not part of the workspace and are not exported or synced.

## File format and lifecycle

Strict JSON, at most 256 KiB, duplicate and unknown top-level keys rejected:

```json
{
  "schema_version": 1,
  "keybindings": { "palette.open": ["Shift+Command+P"] },
  "writing": { "locale": "en-US" },
  "generation": { "count": 3 }
}
```

Each feature owns one top-level section with its own validation. Key bindings store only deviations from built-in defaults, so new defaults reach existing users. `keybindings` is defined in [keyboard shortcuts](keyboard-shortcuts.md#storage-and-api). `writing.locale` is a validated, canonical BCP-47 language tag chosen in Settings and persisted across sessions; `en-US` above is an example, not a fixed default. If no writing preference exists in a valid/missing settings file, initialize it from the Mac's preferred language and persist it with the same guarded section-write protocol before the first AI run. If the language cannot be resolved or settings cannot safely be initialized, show the actual error and require a valid saved choice; never overwrite an invalid/newer file to install a default. A valid saved choice is not inferred again from diary text, timezone or future system-language changes. Logging retention is fixed and is not a user setting.

- **Startup:** the bridge loads once. A missing file means defaults. Malformed JSON, duplicate keys, unknown top-level keys or excess size makes the whole version-1 file invalid; defaults apply with a visible message. With a valid envelope, an invalid section degrades only that section and retains its raw data for preservation. A newer schema is read-only for the session: defaults and a message apply, and every update is rejected without touching it.
- **Write:** take one section plus `expected_settings_revision` (digest or the explicit missing-file revision), merge against the latest stored snapshot, validate and serialize in Rust. A changed file or stale section edit returns conflict rather than overwriting another window/process's preferences. Preserve untouched valid sections and invalid section payloads; replacing an invalid whole file requires a visible explicit recovery/reset action that preserves the original in a private backup first. Ordinary edits never silently erase unknown/newer data. Open without symlink traversal; write a temporary file in the same directory (0600), sync, atomically rename and sync the directory; the directory is created 0700. In-memory settings change only after confirmed publication. Failure before publication preserves the previous file. A post-rename sync failure is an uncertain write: read back/report it and reload before another save, rather than claiming the previous file necessarily survived. Coordinate participating client writers; uncoordinated external writes have the same race limitation as [managed files](domain-and-persistence.md#startup-and-external-changes).
- **Schema:** version 1 is fixed; no silent migration, and a newer version is read-only. A migration needs a backup copy first.
- **Logs:** log that a load or save happened and any error code, never section contents.

`generation.count` is an integer 1–5, default 3 when absent or invalid. Invalid section contents remain on disk when another section changes; only an explicit generation section update replaces them. This is a schema-1 section, not a schema migration. P1-M22 Stage A implements its storage and bridge snapshot (`generation_count`); the Settings control and use by Generate follow in Stage C.

## Commands

Client-only Tauri commands, not forwarded to the server: `get_user_settings` returns the validated effective snapshot, settings revision, read-only status and load warnings; `update_user_settings` takes one section and its expected revision and returns the new snapshot/revision. The WebView never receives the file path or filesystem access. These are distinct from `get_settings`/`reload_settings`, which concern the redacted server config. The command manifest and capability grants cover exactly these commands. Recovery/reset is an explicit mode of the typed update, never an arbitrary file operation.

Typed update contract: `update_user_settings(section, value, expected_revision, recover)` with sections `writing` (a tag), `writing_init` (the system's preferred tags; fills only an absent choice) `keybindings`, and `generation` (`{count}`); `recover: true` backs the original up (0600, `user_settings.json.bak-<seconds>-<n>`) and installs defaults. Error codes: `conflict`, `read_only`, `file_invalid`, `invalid_input`, `language_unresolved`, `uncertain`, `unavailable`. A settings directory that exists with another mode is `location_unsafe` and read-only. `reload_settings` also reports `restart_required`, `rejected` and `busy` outcomes with coarse area/code issues.

## Required evidence

Synthetic tests with a temporary directory: missing, corrupt, oversized, duplicate-key and newer-schema files fall back with a message and no implicit overwrite; newer-schema updates are rejected; bad sections preserve other data; explicit recovery retains the original backup; interrupted writes before/after rename report the correct outcome; concurrent/stale edits conflict; symlink swaps and modes 0600/0700; location is outside the config and workspace roots; no token or path appears in the snapshot sent to the WebView. Restart persistence of an edited binding and writing language is checked in the packaged app.
