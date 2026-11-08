# AGENTS.md

Instructions for contributors and AI coding agents. This is the single source of agent instructions; `CLAUDE.md` imports it with `@AGENTS.md`.

## Project

`narihitos-diary-gui`: Narihito's Diary, the GUI: a personal asset management app on your Mac.

Stack: Tauri 2 (Rust backend), TypeScript + React, CodeMirror 6.

See [README.md](README.md) and [docs/product-behavior.md](docs/product-behavior.md). Setup and contribution rules are in [docs/contribution-guide.md](docs/contribution-guide.md); other docs are indexed at [docs/README.md](docs/README.md).

## Agent workflow

- Work only on a clear task in the active milestone; read its plans and design docs first ([roadmap](docs/roadmap.md), [process](docs/development-process.md)).
- Before implementing, add a verifiable completion checklist to the milestone overview. Check items only with evidence.
- Keep code, tests and docs together. Done = checklist complete and `scripts/gate.sh` passes (fmt, clippy, cargo test, prettier, eslint, vitest, tsc); see [definition of done](docs/development-process.md#definition-of-done).
- Ask before: building on an undecided item, changing a decided one, changing dependencies or build tools, deleting user data, or broad restructuring. Decided items and open questions: [docs/decisions.md](docs/decisions.md).

## Contributing

- Environment, commands, gate and code conventions (including prompt assets): [contribution guide](docs/contribution-guide.md).
- Follow the [commit message rules](docs/contribution-guide.md#commit-messages) in the contribution guide.
- Documentation rules and lifecycle: [development process](docs/development-process.md#documentation-rules).

## Dependencies

Approved dependencies must have licenses compatible with Apache-2.0.
