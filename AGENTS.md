# AGENTS.md

Instructions for contributors and AI coding agents. This is the single source of agent instructions; `CLAUDE.md` imports it with `@AGENTS.md`.

## Project

`narihitos-diary-gui`: Narihito's Diary, the GUI: a personal asset management app on your Mac.

Stack: Tauri 2 (Rust backend), TypeScript + React, CodeMirror 6.

See [README.md](README.md) for the overview and [docs/product-behavior.md](docs/product-behavior.md) for product scope. Setup and contribution rules are in [docs/contribution-guide.md](docs/contribution-guide.md); other docs are indexed at [docs/README.md](docs/README.md).

## Agent workflow

- Work only on a clear task in the active milestone. Read its plans, relevant design docs, and applicable "Undecided" items first.
- Before implementation, add a verifiable completion checklist to the milestone overview. Check items only when evidence exists.
- Keep implementation, tests, and affected docs together. A milestone is done only when its checklist is complete and the gate passes: `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`, `cargo test --manifest-path src-tauri/Cargo.toml`, `npx prettier --check .`, `npx eslint . --max-warnings 0`, and `npx vitest run`. The owner-approved docs-only P1-M1/P1-M2 exception is recorded in [development-process.md](docs/development-process.md#definition-of-done). Otherwise, if a command cannot run, leave its item unchecked and report the blocker.
- Ask before building on an undecided item, changing dependencies or build tools, deleting user data, or broadly restructuring the repository. Other work within the approved task and milestone may proceed.
- Follow the active phase and milestone order in `docs/roadmap.md` unless the phase plan says otherwise. Create the roadmap when planning the first phase.

## Decided — ask before changing

- **Technical stack:** Tauri 2 (Rust backend), TypeScript + React, CodeMirror 6.
- **Product scope:** [product-behavior.md](docs/product-behavior.md).
- **Interaction model:** [ux-gui.md](docs/ux-gui.md). Component vocabulary is in [ux-terms.md](docs/ux-terms.md) and the layout containment model in [ux-information-architecture.md](docs/ux-information-architecture.md).
- **License:** Apache-2.0 for all project code.
- **Process:** numbered phases contain numbered milestones; milestone numbering restarts per phase. IDs use `P1-M2`. See [docs/development-process.md](docs/development-process.md).
- **Milestone ownership (owner update, 2026-11-05):** P1-M2 owns architecture design, clarifications, review and interviews; P1-M3 is scaffolding only. Keep documentation evidence in the [P1-M2 completion record](docs/archive/milestones/p1-m2/milestone-02-architecture-overview.md), and sequence full feature implementation in the [handoff](docs/phase-1/implementation-handoff.md).
- **Owner research clarifications (2026-11-05):** Phase 1 delivers Diary v1.0.0; the Authentic draft and Authentic Entry are one autosaved file (candidates remain transient). Users explicitly edit/save source snippets and delete a snippet plus related audio from the Explorer tree with deletion confirmation. Imports capture the system timezone; saved Entry dates remain fixed. See [Phase 1 changelog](docs/phase-1/changelog.md).
- **P1-M2 owner decisions (2026-11-05):** [Interview record](docs/phase-1/milestone-02-owner-decisions.md) D01–D17 is authoritative for separate shell-launched local server/client, HTTP/SSE and local token, one workspace/client, Markdown plus SQLite, deferred history, paragraph accept/reject, path-based source views, Trash deletion, trusted optional CLIs, private YAML credentials, export conflict confirmation, original GUI layout, SF Symbols, and delegated internal design. The detailed contracts and evidence limits are recorded there.
- **Internal architecture (2026-11-05, D17):** [Clean Architecture](docs/architecture.md) and its themed contracts define inward dependencies, loopback discovery/auth, typed Tauri commands, schemas/recovery, source/date rules, review/CLI lifecycle, config/exports, versioning and native SF Symbols rendering. Follow these contracts; package/tool candidates remain recommendations requiring approval before adoption. Keep delivery sequencing in planning docs, outside the architecture documents.
- **Server ports (owner update, 2026-11-05):** Start at 8781 and try up to eight subsequent ports, ending at 8789 by default. `server.port_fallback_count` defaults to 8 and is bounded to 0–8; stop at the first successful bind. See the [launch contract](docs/architecture/api-and-processes.md#launch-and-discovery).
- **Configuration paths and agents (owner update, 2026-11-05):** `paths.entries_root` and `paths.runtime_root` require absolute paths. CLI agent profiles use `profile_home`; `executable` is an optional override, defaulting to `codex`, `claude` or `qwen` on the server's launch `PATH` by profile type. Apple Foundation Models profiles use neither field. See the [configuration contract](docs/architecture/configuration-and-exports.md#configuration-schema-and-validation).
- **Apple Foundation Models and prompts (owner update, 2026-11-05):** Add optional `apple-foundation-models` alongside Codex, Claude Code and Qwen Code through the same agent port. Use Apple's on-device system model with explicit availability checks; native integration and runtime evidence remain pending. Every app-authored prompt belongs in `assets/prompts/<agent-type>/`, following the [prompt asset contract](assets/prompts/README.md).
- **Architecture review decisions (owner, 2026-11-05):** Choose AI writing language in Settings, initially the Mac's preferred language, and persist it across sessions. Disable Generate until at least one eligible source snippet is selected; date changes keep generation running in the background for its original date. **Create Empty** immediately persists an empty Authentic file for the selected date without snippets and shows it after restart. See the [review interview](docs/phase-1/milestone-02-architecture-review.md#owner-interview).
- **AI delay feedback (owner, 2026-11-05):** Keep ordinary editing/navigation available during AI work, with visible progress and cancellation. Completion feedback stays in the app; no macOS notifications. If the saved Authentic changes during generation, require regeneration before candidate review. The [background AI contract](docs/architecture/ai-job-experience.md) defines delegated progress/streaming/attention details and acceptance evidence.

- **P1-M9 approvals (owner, 2026-11-05):** Approved the reviewed source-reading plan, pinned `unicode-normalization` with license-notice review, stable source IDs across reloads, body-less scans and library-only exposure until P1-M10. See the [completion record](docs/archive/milestones/p1-m9/milestone-09-source-reading-overview.md).

- **P1-M10 approvals (owner, 2026-11-05):** Approved pinned `jiff` (direct dependency in `diary-infrastructure`, with license-notice review), server routes plus bridge commands without UI, and the system timezone database `+VERSION` as the recorded version. See the [work plan](docs/archive/milestones/p1-m10/milestone-10-civil-date-imports-architecture.md#owner-decisions).

- **Import snapshots (owner approval, 2026-11-05):** Persist immutable, body-less scan snapshots per completed `import_id`, atomically with completion. Pagination survives source changes and restart. Resolve only the selected source with a fresh observation; a new import refreshes listings and applies choices. Legacy contexts without snapshots require re-import. See the [source/date contract](docs/architecture/sources-and-dates.md#implementation-status).

- **P1-M13 approvals (owner, 2026-11-06):** Approved direct macOS-only `objc2`, `objc2-app-kit` and `objc2-foundation` dependencies pinned to the locked versions, with license-notice review, and closing on automated tests plus the full gate. Native key, Korean IME and resize proof moves to P1-M28; native VoiceOver and Japanese IME proof are not required. See the [work plan](docs/phase-1/milestone-13-native-symbols-accessibility-architecture.md#owner-decisions).

- **P1-M14 approvals (owner, 2026-11-06):** Narihito mobile schema 1 body saves copy `title` and `title_origin` unchanged; Ayumi saves stay disabled. Approved the `scripts/smoke/producer.py` evidence harness (a Swift program built with the installed `swiftc` from the pinned mobile codec files, run outside the gate, no dependency) and enabling `write_body` once it passes, with phone-app and live-iCloud proof carried to P1-M28. Unresolved iCloud conflict versions are a recorded known gap (no `NSFileVersion` feature). See the [work plan](docs/phase-1/milestone-14-producer-compatible-source-saves-architecture.md#owner-decisions).

- **P1-M15 approvals (owner, 2026-11-06):** Approved enabling only the `NSFileManager` feature on the existing pinned macOS `objc2-foundation` 0.3.2 dependency in `diary-infrastructure`, with license-notice review. P1-M15 acceptance and enablement require automated tests/harnesses plus the full gate only; no human testing or human sign-off is an exit gate, and P1-M15 manual proof must not become a deferred P1-M28 requirement. Native Trash and producer catalog/reopen coverage remain automated requirements; phone/live-iCloud behavior remains an evidence limit. See the [owner decisions](docs/archive/milestones/p1-m15/milestone-15-source-audio-trash-architecture.md#owner-decisions).

- **P1-M11 smoke waiver (owner, 2026-11-06):** The owner skipped the packaged synthetic navigation smoke and closed P1-M11 on automated tests plus the full gate. This waives only that smoke clause; later milestones still need their own evidence. See the [completion record](docs/archive/milestones/p1-m11/milestone-11-explorer-navigation-overview.md#completion-record).

## Undecided — ask before inventing

The [P1-M2 owner interview](docs/phase-1/milestone-02-owner-decisions.md) resolves the earlier product choices. The maintained [architecture](docs/architecture.md) resolves delegated internal design work; do not ask again about already approved choices. Ask before changing those choices or inventing new product behavior outside the delegation.

- **Internal contracts resolved:** the former design gaps are specified in [architecture](docs/architecture.md); their requirement/evidence mapping is in the [implementation handoff](docs/phase-1/implementation-handoff.md). No owner product question remains blocking scaffolding. New behavior outside the approved scope still requires asking; design does not authorize dependency/build-tool changes.
- **Evidence gaps:** producer-compatible source saves/deletes, packaged macOS/iCloud access, process supervision, and service recovery remain unverified. Record the blocked feature and required acceptance evidence; do not turn an interview preference into a runtime capability claim.
- **Agents/privacy:** the owner chose trusted installed CLIs, superseding the proposed confinement prerequisite. Do not infer selected-text confinement or CLI/provider retention guarantees. Account terms and actual CLI capabilities are not verified by that trust choice.
- **Delivery/assets:** personal Apple Silicon/shell-launch scope and SF Symbols are chosen. The [platform design](docs/architecture/configuration-and-exports.md#version-and-platform-strategy) selects the owner's macOS research baseline as an acceptance target and native AppKit runtime symbols; actual support/rendering remain unverified. The owner's instruction to proceed despite licensing concerns is not evidence of license clearance. No new dependency/build tool is approved by a research recommendation or a documentation-level architecture choice.

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
python3 scripts/smoke/imports.py                                                                                   # Civil-date import/resolution/restart smoke on the real server binary
python3 scripts/smoke/trash.py --enabled-server                                                                  # Synthetic native Trash faults + pinned producer catalog + real-server integration
python3 scripts/smoke/authentic.py                                                                                 # Authentic create/save/restart/kill-recovery smoke on the real server binary
cargo run --manifest-path src-tauri/Cargo.toml -p diary-server -- trace --config PATH --select recent:20        # redacted debugging trace of the logs
```

## Code conventions

- Format with `cargo fmt` and `prettier`; do not hand-format.
- Keep `cargo clippy -D warnings` and `eslint --max-warnings 0` clean.
- Expose backend features to the UI only through explicit Tauri commands, and grant the minimum Tauri 2 capabilities/permissions each needs.
- Write UI in TypeScript with React function components; keep CodeMirror 6 editor setup in dedicated modules.
- Before adding or changing agent behavior, inspect `assets/prompts/` and reuse or update the relevant files under `assets/prompts/<agent-type>/` (`codex`, `claude`, `qwen`, `apple-foundation-models`). Store all app-authored system/session instructions, task prompts and retry/repair prompts there; never hard-code prompt text or fallback prompts in source code. Source code may load assets, validate schemas and substitute request data. Missing or invalid assets disable the affected action with a visible error. See the [format and existing inventory](assets/prompts/README.md).

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
