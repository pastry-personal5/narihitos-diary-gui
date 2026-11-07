# Development process

Status: Active

Work is organized into numbered phases and milestones. General documentation rules are in [AGENTS.md](../AGENTS.md).

## Structure and status

- A **phase** is a product increment with a goal and exit criteria. Status: `Planned`, `Active`, or `Done`.
- A **milestone** is a small, verifiable step that fits one reviewable change or short series. Numbering restarts per phase. Status: `Planned`, `Active`, `Done`, or `Dropped`.
- IDs use `P<phase>-M<milestone>` (for example, `P1-M2`).
- Keep one active phase and follow its milestones in order unless its plan says otherwise.
- Phase and milestone numbers may change. Update IDs, filenames, and links together. Mark a dropped milestone `Dropped` with a one-line reason.

## Definition of done

A milestone is done when:

1. Its completion checklist is checked with acceptance evidence.
2. The gate passes: `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`, `cargo test --manifest-path src-tauri/Cargo.toml`, `npx prettier --check .`, `npx eslint . --max-warnings 0`, and `npx vitest run`.
3. Affected docs and its phase-doc status are updated in the same change.

A phase is done when every milestone is `Done` or `Dropped` and its exit criteria hold.

Owner-approved exception: P1-M1 Research and P1-M2 Architecture produce documentation only and are exempt from the code gate in item 2; see the [Phase 1 changelog](phase-1/changelog.md). Their checklists, evidence, affected-doc updates, and local-link/whitespace review still apply. P1-M3 requires the full gate. This exception does not authorize code or dependency changes in the docs-only milestones.

Owner waiver (2026-11-06): P1-M11 closed without its packaged navigation smoke, which the owner skipped. It applies to P1-M11 only; see the [completion record](archive/milestones/p1-m11/milestone-11-explorer-navigation-overview.md#completion-record).

## Plan files

Create a phase directory when planning that phase.

| File                                                | Purpose                                                                                         |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `docs/roadmap.md`                                   | All phases with ID, title, status, and phase-doc link; the sole authority for the active phase. |
| `docs/phase-N/phase-N.md`                           | Phase goal, exit criteria, and brief milestone entries.                                         |
| `docs/phase-N/milestone-NN-<theme>-overview.md`     | Milestone goal, in/out scope, and completion checklist.                                         |
| `docs/phase-N/milestone-NN-<theme>-architecture.md` | Affected modules, interfaces, approach, and sequencing.                                         |
| `docs/phase-N/changelog.md`                         | Reverse-chronological decisions and plan changes not clear from commits.                        |

`N` is the phase number. `NN` is the zero-padded milestone number matching its ID; `<theme>` is kebab-case. Do not use a generic `phase.md` filename.

Keep phase-doc milestone entries short:

```markdown
# Phase 1: <title>

Status: Active
Goal: <one sentence>

## Exit criteria

- <observable outcome>

## Milestones

### P1-M1: <title>

Status: Done
Goal: <one sentence>
Plan: [overview](milestone-01-<theme>-overview.md), [architecture](milestone-01-<theme>-architecture.md)
Notes: <optional decisions or links>
```

Before implementation, create and link `milestone-NN-<theme>-overview.md` and `milestone-NN-<theme>-architecture.md`. Resolve any related "Undecided" item in `AGENTS.md` first.

Record owner decisions, departures from the architecture plan, decided open questions, and reordered/dropped milestones in `changelog.md`:

```markdown
# Phase 1 changelog

Status: Active

## Entries

- 2026-09-28 — <who> — <what changed and why; optional link>
```

## Document lifecycle

- Other docs use `Draft`, `Proposal`, `Active`, `Superseded by <link>`, or `Archived`. Proposals are not decisions.
- Archive completed phase plans, superseded designs, and concluded research; fix their links in `docs/roadmap.md`.
- Record resolved open questions under "Decided" in `AGENTS.md`.

## Documentation rules

- `README.md` is user-facing; `AGENTS.md` holds contributor rules; `docs/` holds plans and design; `docs/archive/` holds inactive docs (not maintained).
- Lowercase kebab-case filenames; each doc starts with a title, `Status:` line and a one- to three-sentence summary. Add every doc to [docs/README.md](README.md); follow [docs/development-process.md](development-process.md).
- Update affected docs with code, `README.md` for user-visible changes, and [docs/decisions.md](decisions.md) for decisions. Link instead of copying facts. Archive completed docs with `git mv` under `docs/archive/` and fix links.
- Save tokens: skip `node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/gen/`, lockfiles and `docs/archive/`; search with `git grep`/`rg`; start at `docs/README.md`; run affected tests while iterating and the full gate once at the end.
