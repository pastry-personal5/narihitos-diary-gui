# AGENTS.md

Instructions for contributors and AI coding agents. This is the single source of agent instructions; `CLAUDE.md` imports it with `@AGENTS.md`.

## Project

`narihitos-diary-gui`: Narihito's Diary, the GUI: a personal asset management app on your Mac.

Stack: Tauri 2 (Rust backend), TypeScript + React, CodeMirror 6.

See [README.md](README.md) for the overview and [docs/product-behavior.md](docs/product-behavior.md) for product scope. Setup and contribution rules are in [docs/contribution-guide.md](docs/contribution-guide.md); other docs are indexed at [docs/README.md](docs/README.md).

## Agent workflow

- Work only on a clear task in the active milestone. Read its plans, relevant design docs, and applicable "Undecided" items first.
- Before implementation, add a verifiable completion checklist to the milestone overview. Check items only when evidence exists.
- Keep implementation, tests, and affected docs together. A milestone is done only when its checklist is complete and the gate passes: `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`, `cargo test --manifest-path src-tauri/Cargo.toml`, `npx prettier --check .`, `npx eslint . --max-warnings 0`, and `npx vitest run`. If a command cannot run, leave its item unchecked and report the blocker.
- Ask before building on an undecided item, changing dependencies or build tools, deleting user data, or broadly restructuring the repository. Other work within the approved task and milestone may proceed.
- Follow the active phase and milestone order in `docs/roadmap.md` unless the phase plan says otherwise. Create the roadmap when planning the first phase.

## Decided — ask before changing

- **Technical stack:** Tauri 2 (Rust backend), TypeScript + React, CodeMirror 6.
- **Product scope:** [product-behavior.md](docs/product-behavior.md).
- **Interaction model:** [ux-gui.md](docs/ux-gui.md). Component vocabulary is in [ux-terms.md](docs/ux-terms.md) and the layout containment model in [ux-information-architecture.md](docs/ux-information-architecture.md).
- **License:** Apache-2.0 for all project code.
- **Process:** numbered phases contain numbered milestones; milestone numbering restarts per phase. IDs use `P1-M2`. See [docs/development-process.md](docs/development-process.md).

## Undecided — ask before inventing

Nothing yet. Add open product and architecture questions here.

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
```

## Code conventions

- Format with `cargo fmt` and `prettier`; do not hand-format.
- Keep `cargo clippy -D warnings` and `eslint --max-warnings 0` clean.
- Expose backend features to the UI only through explicit Tauri commands, and grant the minimum Tauri 2 capabilities/permissions each needs.
- Write UI in TypeScript with React function components; keep CodeMirror 6 editor setup in dedicated modules.

## Dependencies

Approved dependencies must have licenses compatible with Apache-2.0.

## Documentation

- `README.md` is user-facing; `AGENTS.md` holds contributor rules; `docs/` holds plans and design; `docs/archive/` holds inactive docs.
- Use lowercase kebab-case filenames. Start each doc with a title, `Status:` line, and one- to three-sentence summary.
- Add every doc to [docs/README.md](docs/README.md). Follow [docs/development-process.md](docs/development-process.md) for phase and milestone plans.
- Update affected docs with code. Update `README.md` for user-visible changes and this file for tooling, commands, or decisions.
- Link instead of copying facts.
- Archive completed or superseded docs: update status, `git mv` them under `docs/archive/` (completed phase directories go under `docs/archive/phases/`), and fix links. Archived docs are not maintained.

## Token efficiency

- Skip build/dependency output (`node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/gen/`), lockfiles, `LICENSE`, and `docs/archive/` unless needed. Search with `git grep` or `rg`.
- Start at `docs/README.md` and read only relevant docs or sections.
- Keep logs short with quiet flags or filtering. Run affected tests while iterating and the full gate once at the end.
- Consult only the needed dependency source/module for API details.
- Make targeted edits; inspect `git diff --stat` before a full diff.
- Reply with concise summaries, not raw logs.
