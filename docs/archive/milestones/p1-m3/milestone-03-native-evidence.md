# P1-M3 native evidence

Status: Done (archived)

This records bounded packaged scaffold observations. S09 remains incomplete; IPC evidence alone does not establish native folder access, editor interaction or complete transport acceptance.

## Packaged IPC observation — 2026-11-05

Revision: working tree based on `7f3692c`. Environment: Apple Silicon arm64, macOS 26.7.1 (25G241), Rust/Cargo 1.98.0 and temporary Node 26.13.0/npm 11.20.0. Client bundle ID: `com.narihito.diary.gui`. Client executable: `src-tauri/target/debug/bundle/macos/Narihito's Diary.app/Contents/MacOS/diary-client`; independent server: `src-tauri/target/debug/diary-server`.

Commands:

```sh
npm run tauri build -- --debug --bundles app --features scaffold-smoke
python3 scripts/smoke/server.py "src-tauri/target/debug/bundle/macos/Narihito's Diary.app/Contents/MacOS/diary-client"
```

The fixture feature builds a hidden secondary window and evaluates fixed IPC checks in the actual bundled WebViews. It adds no command, server endpoint, permission or capability. Normal code excludes the fixture module. The command manifest/handler/capability inventory stays the same. The main window's ordinary metadata request also runs from React.

Observed allowlisted results:

- Main-window app metadata invocation succeeded.
- Main-window discovery handshake succeeded through the Rust bridge.
- Main-window session open/settings/read/release sequence reached successful release.
- The secondary window's metadata invocation was rejected by the actual packaged IPC boundary.
- The independent server smoke passed unauthorized denial, status/instance match, client exclusion, OS-lock exclusion, lease release with server remaining live, SIGTERM/SIGINT cleanup and restart credential rotation.

The smoke script launches both executable identities separately with test-owned private config/workspace/runtime folders and invented data; it removes only those folders/processes. No private config, real profile/source folder, credential or discovery content was printed. The fixture client was terminated after collecting IPC results; this run does not prove the normal main-window close-request path.

SHA-256 identities at observation:

| Executable               | SHA-256                                                            |
| ------------------------ | ------------------------------------------------------------------ |
| Packaged fixture client  | `e99f19801b66f9183b21963a028a5ba773049c7a00d45018871b8c054a9f5351` |
| Independent debug server | `e4e433e952da45e4168de898209062bef25cecd081c1952d94c67983a480a2d4` |

## Remaining S09 evidence

Record packaged Origin/CSP enforcement explicitly, allowed/denied synthetic folder outcomes from the server identity, fake-job channel/cancel round trip, actual editor keyboard/focus and cleanup, competing packaged client/reconnect and normal close behavior. Rebuild and inspect the ordinary binary for fixture exclusion. This run used a debug `.app` fixture and a standalone debug server; release/fixture packaging and all remaining acceptance must not be inferred from it.

## Packaged fake-job channel observation — 2026-11-05

Same base revision/environment/bundle identity, with newer working-tree sources. Commands:

```sh
npm run tauri build -- --debug --bundles app --features scaffold-smoke
cargo build --manifest-path src-tauri/Cargo.toml -p diary-server --features scaffold-smoke
python3 scripts/smoke/server.py "src-tauri/target/debug/bundle/macos/Narihito's Diary.app/Contents/MacOS/diary-client"
```

The server's opt-in API composition seeds one queued/running in-memory fake job through the application coordinator after session admission. No fake-admission HTTP endpoint, Tauri command or extra permission exists. The ordinary build excludes that module. The client fixture requests the snapshot and channel, acknowledges delivery counters, cancels the job through the normal command, waits for the cancelled SSE/channel event, queries authoritative cancelled state and only then releases the lease.

Observed: `main_metadata_allowed`, `main_handshake_allowed`, `main_delivery_acknowledged`, `main_cancelled_job_read`, `main_session_released` and `secondary_command_denied`. The smoke also passed client/server exclusion, signal cleanup, forced crash/stale-record liveness failure and restart rotation. All six native marker assertions passed, and fixture-feature workspace Clippy passed.

| Executable                | SHA-256                                                            |
| ------------------------- | ------------------------------------------------------------------ |
| Packaged fixture client   | `8968bc864acc0c9cefb30f731992560c272bdcd4f4d501eb5395ad8e3000ff21` |
| Standalone fixture server | `0c1609f7ed37f64a8c2fa2fe49afde532e7973670d379ae86c0b0aded80f0046` |

The runtime check used only invented temporary data and printed no credentials. It establishes the native fake-job transport round trip; it does not establish source/model/durable capabilities or all remaining S09 items. An ordinary bundle rebuild was started afterward to check binary fixture exclusion; that separate result must be recorded before claiming exclusion.

Ordinary-build follow-up: `npm run tauri build -- --debug --bundles app` succeeded without fixture features, then `node scripts/check-fixture-exclusion.mjs` confirmed fixture markers absent from the ordinary standalone client/server binaries and packaged client executable. The opt-in manifest/module inventory test also passed as part of all 23 Vitest tests. This establishes exclusion for the rebuilt artifacts, not a new native runtime observation of the ordinary app.

## Packaged CSP and synthetic folder observation — 2026-11-05

The same packaged fixture commands passed against the newer working tree. Before continuing its native fake-job sequence, the client asserts `window.location.origin === 'tauri://localhost'` and waits for a real `securitypolicyviolation` event identifying `connect-src` for a direct HTTP fetch. A failed socket/CORS promise alone does not satisfy that assertion. The session-release marker occurs only after the origin/CSP, folder and cancelled-channel assertions have all completed.

The script prepares two sibling test-owned source roots. Each contains only `sentinel.txt` with invented fixed content; one root is mode 0700 and the denied root is mode 000 until cleanup. The fixture server opens/reads the fixed sentinel names itself, verifies the readable bytes and captures the denied read's OS error. It exposes only alias/outcome pairs through fixture Settings, never source paths or sentinel bytes. Both sources remain unavailable for product operations.

Observed server results: `server_folder_readable`, `server_folder_denied errno=13` (EACCES), `server_folder_denied`. Client assertions verified both bounded outcomes. All six native IPC/channel markers and lifecycle/crash assertions again passed. The script restored permissions and removed only its own temporary folders/processes. Fixture-feature workspace Clippy passed before the errno-only diagnostic addition.

| Executable                | SHA-256                                                            |
| ------------------------- | ------------------------------------------------------------------ |
| Packaged fixture client   | `9ae53cd17092874c65de5e285eeca171c1afb73191280aa0b71be5b575c2e646` |
| Standalone fixture server | `42b28c3e5e6094049229faa50b29840ed55265dce63f39e07624f70cd00031ce` |

This proves ordinary filesystem permission denial for these synthetic local folders under the recorded server identity. It does not establish TCC/iCloud/File Provider access, producer compatibility, native model availability or durable storage. Editor keyboard/focus, normal main-window close, competing packaged-client/reconnect and final ordinary-build exclusion for these newer artifacts remain pending; S09 stays open. The current bundle/server build outputs are fixture artifacts until the next ordinary rebuild.

## Final S09 observation — 2026-11-05

Base `9553a7f` plus the uncommitted working tree; Apple Silicon, macOS 26.7.1. Fixture build `npm run tauri build -- --debug --bundles app --features scaffold-smoke`, then `python3 scripts/smoke/server.py <packaged diary-client>` passed. Reported markers: native target activation with one window, `editor_probe` (focused, active editor, sentinel matched), metadata/handshake allowed, delivery acknowledged, running and cancelled job reads, session released, `main_window_close_released`, `main_window_destroyed`, `secondary_command_denied`, `server_folder_readable`, `server_folder_denied errno=13`. The script itself asserts `SMOKE client_busy_rejected` from a competing packaged client, plus lifecycle exclusion, SIGTERM/SIGINT cleanup, crash/stale rejection and restart rotation, so its pass includes them. No stray processes remained. This corrects the earlier failed focus diagnosis: the later run activated and focused the window. The marker list comes from the run report, not a retained log.

Ordinary follow-up: `npm run tauri build -- --debug --bundles app` and `cargo build -p diary-server` succeeded without fixture features; `node scripts/check-fixture-exclusion.mjs` confirmed fixture markers absent from the ordinary packaged client and standalone server; `npm run check:versions` printed `Version v1.0.0`.

| Executable (ordinary) | SHA-256                                                            |
| --------------------- | ------------------------------------------------------------------ |
| Packaged client       | `76fc775814cda364c1e1e40695dd8dac43bf95f2c814648334078d5632ec1093` |
| Standalone server     | `36248fa6f73a0a6791a269ece5f303174c19b60dbbc295a8eedd539e6e08cec4` |

Limits: debug, ad hoc, shell-launched bundles with invented data; no TCC/iCloud, release signing or notarization claim.
