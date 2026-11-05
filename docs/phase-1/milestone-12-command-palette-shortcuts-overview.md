# P1-M12: Command palette and keyboard shortcuts overview

Status: Planned

Route every GUI action through one registry, and add the command palette and editable, persistent, scope-aware keyboard shortcuts with conflict and IME protection. No new product behavior, dependency or build tool is proposed. The [work plan](milestone-12-command-palette-shortcuts-architecture.md) defines files, interfaces, sequencing and tests.

## Entry conditions

- P1-M8 (user settings, `keybindings` section) and P1-M11 (Standard Layout, resource model, Global Messages) are Done.
- Follow [keyboard shortcuts](../architecture/keyboard-shortcuts.md), [user settings](../architecture/user-settings.md), [UX GUI](../ux-gui.md#input-and-accessibility) and handoff E12. Read the evidence gaps in [AGENTS.md](../../AGENTS.md).
- Owner decisions from the 2026-11-06 interview are in the [work plan](milestone-12-command-palette-shortcuts-architecture.md#owner-decisions); no owner question remains open.

## Scope

Included: the 12-action registry (data shared by Rust and TypeScript); the dispatcher (`app` and `snippetTree` scopes, IME guard, suspension while dialogs or capture are active); the command palette; toolbar buttons routed through the same handlers; the Settings **Keyboard Shortcuts** table (capture, conflict Reassign, Reset, Reset all); Rust validation, per-entry inactive handling and effective-binding output in the user-settings snapshot.

Not included: handlers for `source.trash`, `generate.start`, `ai.*` and `export.start` (registered, disabled with a reason until P1-M15/P1-M21–P1-M24/P1-M25 and later); native SF Symbols and VoiceOver/IME qualification (P1-M13); the optional table search field; macOS menu-bar accelerators; actual WKWebView key delivery evidence (P1-M13/P1-M28 per E12 — mocks are not native proof).

## Completion checklist

Check only with recorded evidence. Planning does not complete these items.

- [ ] K01 — One registry file lists all 12 actions with id, label, info, scope and defaults; Rust and TypeScript both read it, and a parity test fails if either side's copy of ids, scopes, defaults or the reserved list diverges.
- [ ] K02 — Shortcut strings canonicalize identically in Rust and TypeScript from shared fixtures (modifier order Control, Option, Shift, Command; `Shift+Command+P` equals `Command+Shift+P`); the factory defaults validate cleanly under the shipped validator.
- [ ] K03 — Effective bindings merge defaults with overrides (new actions and defaults reach users with overrides); unknown ids, invalid strings and conflicts are inactive and reported, never fatal; a newly added default never steals an existing override.
- [ ] K04 — `update_user_settings` for `keybindings` validates against registry and reserved keys in Rust, keeps unknown/invalid entries on disk through ordinary one-action edits, and replaces them only through Reset all; the snapshot returns effective bindings and per-entry issues. No new Tauri command or capability.
- [ ] K05 — The dispatcher resolves a keydown by canonical shortcut and scope, never during IME composition (`isComposing` or `keyCode 229`), never for reserved text/clipboard/system keys, and `snippetTree` shortcuts fire only with focus in the Snippet tab so editor deletion is unaffected.
- [ ] K06 — The command palette opens from `palette.open` and the toolbar, filters by label, shows current shortcuts and disabled reasons, runs the enabled action, and restores focus to its invoker; Enter during composition does not run an action.
- [ ] K07 — Create Empty, Save/flush, Refresh, Settings and the palette are reachable from the toolbar or palette when unbound, and toolbar buttons call the registry handlers. `editor.save` on a source never reports success while source writers are disabled.
- [ ] K08 — Registered-but-unavailable actions are listed with a reason; a bound shortcut for one is consumed and posts the reason to Global Messages. `source.trash` opens nothing.
- [ ] K09 — The Settings table shows Action, Original, Current ("Modified"), Info; capture, Escape, add/remove chip, Edit, Reset, confirmed Reset all, conflict naming with explicit Reassign, reserved-key messages, text names for VoiceOver. Dispatch pauses during capture and while the source-guard, recovery or Reset-all dialogs are open.
- [ ] K10 — Affected docs, README, handoff, phase plan, roadmap and changelog reflect actual behavior and the evidence limit (no native key proof).

## Plan-review checklist

- [x] R01 — Compare the plan with the maintained contracts and current code; record findings and corrections in the [work plan](milestone-12-command-palette-shortcuts-architecture.md#plan-review-findings-and-corrections).
- [x] R02 — Map each finding to a K item and a named test.
- [x] R03 — Format changed plans, check local links and whitespace, and review the scoped diff; record validation below.

## Implementation gate

- [ ] G01 — `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [ ] G02 — `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [ ] G03 — `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] G04 — `npx prettier --check .`
- [ ] G05 — `npx eslint . --max-warnings 0`
- [ ] G06 — `npx vitest run`
- [ ] G07 — `npx tsc --noEmit` (additional project check)

Run affected tests while iterating and `scripts/gate.sh` once at the end. If a command cannot run, leave its item unchecked and record the blocker.

## Evidence record

Planned 2026-11-06. Plan review completed 2026-11-06; eight findings (F1–F8) and their corrections are in the [work plan](milestone-12-command-palette-shortcuts-architecture.md#plan-review-findings-and-corrections). Planning validation passed: `npx prettier --check docs`, a local-link check over both plans, the README and the roadmap (no broken targets), and `git diff --check`. No application code or dependencies changed; the gate was not run, so K01–K10 and G01–G07 stay unchecked until implementation.
