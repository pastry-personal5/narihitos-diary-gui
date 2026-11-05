# Contribution Guide

Status: Active

How to set up `narihitos-diary-gui`, validate a change, write commit messages, and open a pull request.

## Setup and development

- macOS with Xcode Command Line Tools.
- Rust stable toolchain (rustup), with rustfmt and clippy.
- Node.js (current LTS) and npm.
- Tauri 2 prerequisites; see the Tauri docs for macOS setup.

```sh
npm run tauri dev                                                                                                # run the app in development
npm run tauri build                                                                                              # build the app bundle
cargo test --manifest-path src-tauri/Cargo.toml                                                                  # run Rust tests
npx vitest run                                                                                                   # run UI tests
cargo fmt --manifest-path src-tauri/Cargo.toml && npx prettier --write .                                         # format
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings && npx eslint . --max-warnings 0  # lint
```

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
