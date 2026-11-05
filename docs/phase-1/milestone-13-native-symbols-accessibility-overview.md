# P1-M13: Native symbols and accessible interaction overview

Status: Planned

Add allowlisted AppKit SF Symbols to the existing icon controls through a `get_symbol` command, and close the accessibility gaps that automated tests can reach (names, order, focus, separators, IME-safe save). The [work plan](milestone-13-native-symbols-accessibility-architecture.md) defines files, interfaces, sequencing and tests.

## Entry conditions

- P1-M11 (Standard Layout, Pane separators) and P1-M12 (action registry, palette, toolbar routing) are Done.
- Follow the [platform symbols route](../architecture/configuration-and-exports.md#version-and-platform-strategy), [input and accessibility](../ux-gui.md#input-and-accessibility) and handoff E9/E12. Read the evidence gaps in [AGENTS.md](../../AGENTS.md).
- Owner decisions (2026-11-06) are in the [work plan](milestone-13-native-symbols-accessibility-architecture.md#owner-decisions): direct macOS-only `objc2`, `objc2-app-kit` and `objc2-foundation` dependencies are approved, and native proof is not required to close this milestone.

## Scope

Included: the `get_symbol` command (allowlist, bounded PNG, in-memory cache, unsupported-symbol result); the React `Symbol` component with a text fallback; symbols on the Create Empty, Command palette, Settings, Generate and Close-tab controls; an accessible-name, tab-order, focus-restore and live-region audit with tests; keyboard resize and IME checks for Pane separators and the save debounce.

Not included: SF Symbols application, template exports, font or raster assets; a new theme or component library; native VoiceOver, WKWebView key delivery, Korean/Japanese IME and packaged resize proof (carried to P1-M28 as a known gap, see below); license clearance for Apple's symbols; new product behavior.

## Completion checklist

Check only with recorded evidence. Planning does not complete these items.

- [ ] S01 — `get_symbol` accepts only allowlisted names and bounded size, scale and appearance values, returns a PNG within the byte cap, and rejects everything else with a typed error; non-macOS builds compile and return `unsupported`.
- [ ] S02 — A missing or failing symbol yields `unsupported`; the UI shows the text label and reports "unsupported symbol" once in Global Messages, never an empty button.
- [ ] S03 — `Symbol` renders decoration only (`aria-hidden`); each icon control keeps its UX-terms accessible name and tooltip, and the shortcut hint, whether or not the symbol loaded.
- [ ] S04 — Symbols follow text color and disabled state, and an in-memory cache serves repeat requests without a second command call.
- [ ] S05 — The `get_symbol` permission is declared in `build.rs` and `capabilities/main.json`; no other capability widens.
- [ ] S06 — Every interactive control has a role and non-empty accessible name; tab order follows the Standard Layout (toolbar, Icon Bar, Explorer, Tabbed Editor, Global Messages); a test fails on an unnamed control.
- [ ] S07 — Dialogs and the palette restore focus to their invoker; Editor tab changes keep focus; snippet focus stays independent of inclusion.
- [ ] S08 — Pane separators resize by arrow keys, clamp to limits, and ignore keys during composition; Global Messages stays a polite log region.
- [ ] S09 — The save debounce does not fire during composition and flushes once after `compositionend` (existing queue behavior pinned by test).
- [ ] S10 — Affected docs, README, handoff (E9/E12), phase plan, roadmap and changelog state actual behavior and the evidence limit: no native VoiceOver, IME, key or resize proof; the dependency notices are updated.

## Plan-review checklist

- [ ] R01 — Compare the plan with the maintained contracts and current code; record findings in the work plan.
- [ ] R02 — Map each finding to an S item and a named test.
- [ ] R03 — Format changed plans, check local links and whitespace, and review the scoped diff.

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

Planned 2026-11-06. No application code or dependencies changed yet; every item above is unchecked until implementation.

Known gap carried to P1-M28: native VoiceOver names and order, Korean/Japanese IME, WKWebView key delivery, Pane separator dragging, and real SF Symbol rendering and appearance. The owner chose automated tests only for closing this milestone, so mocks and unit tests are not native proof.
