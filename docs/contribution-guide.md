# Contribution Guide

Status: Active

How to set up `narihitos-diary-gui`, validate a change, write commit messages, and open a pull request.

## Setup and development

Use Apple Silicon/macOS 26.7.1 for the recorded scaffold acceptance environment, Xcode/Command Line Tools, Rust 1.98.0 with rustfmt/Clippy, and Node 26.13.0 LTS/npm 11.20.0. Exact dependency pins are in the manifests/lockfiles; approval and evidence are in the [setup inventory](archive/milestones/p1-m3/milestone-03-setup.md). Other platform versions are not qualified by this scaffold.

```sh
npm ci --ignore-scripts
npm run build
```

The server and client launch separately. The client never supervises the server:

```sh
cargo run --manifest-path src-tauri/Cargo.toml -p diary-server -- --config /absolute/private/config.yaml --development-origin http://localhost:1420
npm run tauri dev -- -- --connect /absolute/private/runtime/WORKSPACE_KEY/discovery.json
```

The client opens every component of the `--connect` path without following symlinks. A symlinked component (for example `~/work` pointing at a volume) fails with `discovery_unavailable`; pass the `realpath`. `run-gui.sh` and `make gui` do this.

The server prints only the discovery path. Do not print or copy discovery contents: they contain a credential. The [configuration contract](architecture/configuration-and-exports.md#configuration-schema-and-validation) defines private permissions and roots. On start the server initializes an empty `paths.entries_root` (private `.diary` with the lock, `state.sqlite3` and `workspace.json`) and reopens an initialized one. A folder that has files but no `.diary`, or unexpected files in `.diary`, is refused or reported `inspection_required` without modification; a lost or damaged database starts in read-only `recovery` mode (`storage_ready` false). Never point `entries_root` at a folder you have not inspected, and never delete `.diary` files to "fix" recovery. Sources, durable saves, agents and exports remain unavailable; the editor is disposable scratch text.

Build/run commands:

```sh
npm run typecheck
npm run build
cargo build --manifest-path src-tauri/Cargo.toml -p diary-server
npm run tauri build -- --debug --bundles app
src-tauri/target/debug/diary-server --config /absolute/private/config.yaml
"src-tauri/target/debug/bundle/macos/Narihito's Diary.app/Contents/MacOS/diary-client" --connect /absolute/private/runtime/WORKSPACE_KEY/discovery.json
```

For the isolated ordinary server smoke, run `python3 scripts/smoke/server.py` after building the server. The [native evidence](archive/milestones/p1-m3/milestone-03-native-evidence.md) records fixture build commands, outcomes and remaining native checks. `scaffold-smoke` is opt-in and changes no command permission; do not use it with real config/source roots. After rebuilding ordinary artifacts, `node scripts/check-fixture-exclusion.mjs` checks fixture-marker exclusion. App packaging requires the frontend build/type check via its configured before-build command.

For the Authentic persistence smoke, run `python3 scripts/smoke/authentic.py`; it builds a separate `fault-injection` binary under `src-tauri/target/fault` (the `DIARY_FAULT` environment variable is read only in that build), creates, saves, restarts and reloads files, and `SIGKILL`s a save between publication and receipt to check recovery.

`python3 scripts/collect-notices.py` refreshes exact resolved dependency notice assets using the local Cargo/npm installation. The [third-party notice record](../third-party-notices.md) identifies current review gaps. Keep upstream legal text byte-for-byte and retain source provenance; do not infer license clearance from package metadata alone.

## Debugging with the redacted trace

Logs live under `<runtime_root>/<workspace-key>/logs/{server,client}` (0700/0600, kept 72 hours) and hold codes, IDs and sizes only: never diary text, credentials, prompts or paths. To debug a failure, from a person or an AI agent:

1. Find a trace ID: the `X-Diary-Trace-Id` response header of the failing request, or `cargo run --manifest-path src-tauri/Cargo.toml -p diary-server -- trace --config PATH --select recent:20` for the last warn/error records.
2. Follow it: `… trace --config PATH --select trace:<32 hex>` (other selectors: `op:<id>`, `job:<id>`, `res:<kind>:<id>`). The output merges server and client records by time, then ends with a summary line that lists the distinct resources and a legend.
3. Read `res`: `rev`, `bytes`, `lines`, `state` and `path` describe the resource; `fp` is a keyed fingerprint, so equal `fp` at two events of one process run means identical content without revealing it. Fingerprints mean nothing after a restart.

The trace command is read-only, bounded to 200 records (`--limit` up to 500) and 256 KiB, and drops any line that fails strict validation, so it is safe to hand to an agent. If a client has no log file (another client holds it, or discovery was never validated), its records are on its stderr only.

For Generate failures, launch the client with `DIARY_LOG=debug` (`make gui` already sets it), reproduce once, then use `diary-server trace --config PATH --select debug:50` for preparation stages or `--select recent:20` for failure warnings. Client `ui.generation_*` stages distinguish a dirty-source choice, Entry settling and submission; server `generation.capture` warnings explain rejected profile/source capture. See the [logging contract](architecture/logging.md#generation-records).

## Required validation

Before opening a pull request, run the full checks below from the repository root, except for the owner-approved docs-only P1-M1/P1-M2 exemption. See [development-process.md](development-process.md#definition-of-done) for that exemption and the required documentation review.

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npx prettier --check .
npx eslint . --max-warnings 0
npx vitest run
```

## Code and test conventions

Do not hard-code milestone or phase numbers (such as `P1-M3`) in source code or test code: not in identifiers, strings, comments, test names, or fixture names. Name things by behavior. Milestone and phase references belong in `docs/`.

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org):
`<type>[(scope)]: <imperative summary>`. Keep the lowercase title at most 50
characters with no period. Use a short body only to explain why.

Keep each commit focused. Example:

    feat(parser): add support for nested lists

    Nested list items were flattened into a single level. Keep the nesting
    so the parsed document matches the source.

## Pull requests

TBD

## Environment

- macOS with Xcode Command Line Tools.
- Rust stable toolchain (rustup), with rustfmt and clippy.
- Node.js (current LTS) and npm.
- Tauri 2 prerequisites; see the Tauri docs for macOS setup.

## Commands

```sh
npm run tauri dev                                                                                                # run the app in development
npm run tauri build                                                                                              # build the app bundle
cargo test --manifest-path src-tauri/Cargo.toml                                                                  # run Rust tests
npx vitest run                                                                                                   # run UI tests
cargo fmt --manifest-path src-tauri/Cargo.toml && npx prettier --write .                                         # format
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings && npx eslint . --max-warnings 0  # lint
scripts/gate.sh                                                                                                  # full gate + tsc typecheck: parallel lanes, quiet, PASS/FAIL per command
```

Smoke tests and the log trace tool: [docs/smoke-tests.md](smoke-tests.md).

## Agent and code conventions

- Format with `cargo fmt` and `prettier`; do not hand-format.
- Keep `cargo clippy -D warnings` and `eslint --max-warnings 0` clean.
- Expose backend features to the UI only through explicit Tauri commands, and grant the minimum Tauri 2 capabilities/permissions each needs.
- Write UI in TypeScript with React function components; keep CodeMirror 6 editor setup in dedicated modules.
- Before adding or changing agent behavior, inspect `assets/prompts/` and reuse or update the relevant files under `assets/prompts/<agent-type>/` (`codex`, `claude`, `qwen`, `apple-foundation-models`). Store all app-authored system/session instructions, task prompts and retry/repair prompts there; never hard-code prompt text or fallback prompts in source code. Source code may load assets, validate schemas and substitute request data. Missing or invalid assets disable the affected action with a visible error. See the [format and existing inventory](../assets/prompts/README.md).
