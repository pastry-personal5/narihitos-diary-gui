# P1-M13: Native symbols and accessible interaction overview

Status: Done

Add allowlisted AppKit SF Symbols to the existing icon controls through a `get_symbol` command, and close the accessibility gaps that automated tests can reach (names, order, focus, separators, IME-safe save). The [work plan](milestone-13-native-symbols-accessibility-architecture.md) defines files, interfaces, sequencing and tests.

## Entry conditions

- P1-M11 (Standard Layout, Pane separators) and P1-M12 (action registry, palette, toolbar routing) are Done.
- Follow the [platform symbols route](../architecture/configuration-and-exports.md#version-and-platform-strategy), [input and accessibility](../ux-gui.md#input-and-accessibility) and handoff E9/E12. Read the evidence gaps in [AGENTS.md](../../AGENTS.md).
- Owner decisions (2026-11-06) are in the [work plan](milestone-13-native-symbols-accessibility-architecture.md#owner-decisions): direct macOS-only `objc2`, `objc2-app-kit` and `objc2-foundation` dependencies are approved, and native proof is not required to close this milestone.

## Scope

Included: the `get_symbol` command (allowlist, bounded PNG, in-memory cache, unsupported-symbol result); the React `Symbol` component with a text fallback; symbols on the Create Empty, Command palette, Settings, Generate, Close-tab and Refresh controls; an accessible-name, tab-order, focus-restore and live-region audit with tests; keyboard resize and IME checks for Pane separators and the save debounce.

Not included: SF Symbols application, template exports, font or raster assets; a new theme or component library; native VoiceOver proof (the owner does not require it); WKWebView key delivery, Korean IME and packaged resize proof (carried to P1-M28 as a known gap, see below); license clearance for Apple's symbols; new product behavior.

## Completion checklist

Check only with recorded evidence. Planning does not complete these items.

- [x] S01 — `get_symbol` accepts only allowlisted names and bounded size and scale values, returns a PNG within the byte cap, and rejects everything else with a typed error; non-macOS builds compile and return `unsupported`.
- [x] S02 — A missing or failing symbol yields `unsupported`; the UI shows the text label and reports "unsupported symbol" once in Global Messages, never an empty button.
- [x] S03 — `Symbol` renders decoration only (`aria-hidden`); each icon control keeps its UX-terms accessible name and tooltip, and the shortcut hint, whether or not the symbol loaded.
- [x] S04 — Symbols follow text color and disabled state, and an in-memory cache serves repeat requests without a second command call.
- [x] S05 — The `get_symbol` permission is declared in `build.rs` and `capabilities/main.json`; no other capability widens.
- [x] S06 — Every interactive control has a role and non-empty accessible name; tab order follows the Standard Layout (toolbar, Icon Bar, Explorer, Tabbed Editor, Global Messages); a test fails on an unnamed control.
- [x] S07 — Dialogs and the palette restore focus to their invoker; Editor tab changes keep focus; snippet focus stays independent of inclusion.
- [x] S08 — Pane separators resize by arrow keys, clamp to limits, and ignore keys during composition; Global Messages stays a polite log region.
- [x] S09 — The save debounce does not fire during composition and flushes once after `compositionend` (existing queue behavior pinned by test).
- [x] S10 — Affected docs, README, handoff (E9/E12), phase plan, roadmap and changelog state actual behavior and the evidence limit: no native key, Korean IME or resize proof, and no VoiceOver proof (not required); the dependency notices are updated.

## Plan-review checklist

- [x] R01 — Compare the plan with the maintained contracts and current code; record findings in the work plan.
- [x] R02 — Map each finding to an S item and a named test.
- [x] R03 — Format changed plans, check local links and whitespace, and review the scoped diff.

## Implementation gate

- [x] G01 — `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [x] G02 — `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [x] G03 — `cargo test --manifest-path src-tauri/Cargo.toml`
- [x] G04 — `npx prettier --check .`
- [x] G05 — `npx eslint . --max-warnings 0`
- [x] G06 — `npx vitest run`
- [x] G07 — `npx tsc --noEmit` (additional project check)

Run affected tests while iterating and `scripts/gate.sh` once at the end. If a command cannot run, leave its item unchecked and record the blocker.

## Evidence record

Planned 2026-11-06; plan review findings are in the [work plan](milestone-13-native-symbols-accessibility-architecture.md#plan-review-findings-and-corrections).

Implemented 2026-11-06. `scripts/gate.sh` passed (G01–G07; 142 vitest tests). Evidence: Rust `symbols` tests (allowlist, size/scale limits, base64, result tags, macOS PNG signature/cap and unknown-symbol, run on this Mac) and `tests/architecture.test.js` (command/permission lists agree); `tests/symbols.test.tsx` (DTO validation, aria-hidden decoration, single shared call, fallback text, unsupported notice once per name, error retry, disabled name/tooltip, no symbol bridge); `tests/accessibility.test.tsx` (every control named, region order). S06 names cover buttons, tabs, separators and inputs. S08 and S09 are pinned by existing tests (`navigation-components` separator bounds, `navigation-layout` composition and polite log, `saveQueue` composition). Notices: the three crates were already in `assets/third-party/inventory.json` through Tauri, so no inventory change was needed.

S04: the cache is pinned by `shares one native call between simultaneous and repeat mounts`; following text and disabled color is pinned only as a CSS rule (`symbol styling` test: `background-color: currentColor` under a mask), because jsdom cannot compute a masked color, so the visible effect is unproven until P1-M28. S01/S02 also reject an all-transparent render as unsupported, but the real rendering of the six symbols is unverified.\n\nS07: dialog and palette focus restore is the P1-M12 palette test; `resources.test.ts` `moves focus into the restored editor on a tab change and keeps it there` pins Editor tab focus (jsdom, so native WebView focus remains unproven); `accessibility.test.tsx` `keeps focus on the Include checkbox when it is toggled` pins snippet focus independence.

Not required by the owner (2026-11-06): native VoiceOver proof. Accessible names and order are covered by automated role/name tests only.

Known gap carried to P1-M28: native WebView focus, WKWebView key delivery, Korean IME, Pane separator dragging, and real SF Symbol rendering. The owner chose automated tests only for closing this milestone, so mocks and unit tests are not native proof.
