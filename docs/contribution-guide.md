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

The server prints only the discovery path. Do not print or copy discovery contents: they contain a credential. The [configuration contract](architecture/configuration-and-exports.md#configuration-schema-and-validation) defines private permissions and roots. Storage initialization is deferred to P1-M5; the scaffold requires an existing private workspace lock tree prepared by the synthetic smoke script. Do not create a partial workspace in a real diary directory. Sources, durable saves, agents and exports remain unavailable; the editor is disposable scratch text.

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

`python3 scripts/collect-notices.py` refreshes exact resolved dependency notice assets using the local Cargo/npm installation. The [third-party notice record](../third-party-notices.md) identifies current review gaps. Keep upstream legal text byte-for-byte and retain source provenance; do not infer license clearance from package metadata alone.

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
