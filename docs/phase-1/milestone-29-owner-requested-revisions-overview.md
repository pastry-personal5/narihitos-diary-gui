# P1-M29: Owner-requested revisions overview

Status: Active

Collect and deliver the owner's requested bug fixes, UX fixes, GUI fixes, behavior changes, refactoring and architecture changes. Each request becomes a task with a checkbox below as it arrives; nothing is added without a request.

## Entry conditions

P1-M28 is Done ([overview](milestone-28-simple-refactoring-overview.md)). Decided items in [AGENTS.md](../../AGENTS.md) need explicit owner approval before a request changes them.

## How requests are recorded

- One task per request, with a stable ID (`T01`, `T02`, ...) and a kind tag: `[bug]`, `[ux]`, `[gui]`, `[behavior]`, `[refactor]` or `[arch]`.
- A task with parts gets sub-tasks (`T01.1`, `T01.2`). Check a box only when evidence exists (test, smoke or recorded result) and note it on the line.
- Behavior and architecture requests also get an owner-decision line (date and the choice) before implementation.
- Closing needs every box checked, the full gate (`scripts/gate.sh`) passing and affected docs updated.

## Owner decisions

- **T02 (owner, 2026-11-07):** The GUI is dark theme only for now; it no longer follows the system light/dark setting. A light or switchable theme is a later request.
- **T03 (owner, 2026-11-07):** Neutral gray surfaces with no green tint, referenced from the owner's VS Code dark theme screenshot (blue accent). Palette approved in chat and recorded in [ux-gui.md](../ux-gui.md).
- **T05 (owner, 2026-11-07):** Settings panes use cards. Agent profiles are checked in the background from launch (first at launch, then 15 s doubling, capped at 1 h, per profile, for the whole session) and shown in a table with SF Symbols; an Advanced card lists what blocks Chat and Generate. The owner approved the background polling.

## Scope

In scope: the requests listed in the checklist. Out of scope: anything not requested, new dependencies or build tools without approval, and release qualification (P1-M32).

## Open items

None yet.

## Completion checklist

### Requests

- [x] T01 `[gui]` - Remove the visible "Icon Bar" text from the left strip (`aria-label` kept). Evidence: `src/components/App.tsx`; gate pending (Z01).
- [x] T02 `[gui]` - Dark theme only.
  - [x] T02.1 - Dark `:root` tokens plus `color-scheme: dark` in `src/styles.css`; hard-coded light colors replaced; the `prefers-color-scheme` block removed.
  - [x] T02.2 - CodeMirror gutter and caret styled for dark.
  - [ ] T02.3 - Visual check in the running app (not yet done).

- [x] T03 `[gui]` - Define one dark-theme color palette (named tokens, roles and contrast targets) and use it across the GUI; no hard-coded colors outside the token block.
  - [x] T03.1 - Owner approves the palette (design in chat, 2026-11-07).
  - [x] T03.2 - Tokens defined once in `src/styles.css` and documented in `docs/ux-gui.md`; text roles meet WCAG AA (4.5:1) on every surface; selected-state text uses an on-accent token (white on the mint accent is 1.9:1 today).
  - [x] T03.3 - A test checks that `src/styles.css` has no color literals outside the token block.

- [x] T04 `[gui]` - Replace the app icon with a very simple, abstract, modern mark and a transparent background outside the shape (no green tile).
  - [x] T04.1 - `assets/icons/app.svg` redrawn (accent-blue page, no background tile); `src-tauri/icons/icon.png` and `icon.icns` regenerated from it. Evidence: corner pixel of `icon.png` is fully transparent.
  - [x] T04.2 - Decoded `icon.icns` (1024 px): corner fully transparent, shape fill #2a7fa8. A Dock/Finder look at a packaged build was not done.

- [x] T05 `[gui]` - Settings: replace the "Close Settings" button with a "Go back" SF Symbol button (`chevron.backward`, icon only with `aria-label`; "←" glyph if the symbol is unavailable) above "Status", left-aligned with the section buttons. Evidence: `src/components/SettingsView.tsx`; tests updated; gate pending (Z01).
  - [ ] T05.1 - Visual check in the running app (not yet done).

- [x] T05 `[gui]` `[behavior]` - Settings cards, background profile checks with exponential backoff, profile table, Advanced card. Evidence: `src/components/{useProfiles.ts,ProfileTable.tsx,advanced.ts,SettingsView.tsx}`; `tests/profiles-ui.test.tsx` passes. Not done: visual check in the running app, `scripts/gate.sh`.

- [x] T06 `[gui]` - Main sidebar (Icon Bar): icons four times as big (16 → 64 px), sidebar column narrower (90 → 80 px, the least that fits the 64 px icon), button shrinks to fit its icon; icons are flat (transparent background and border, matching the bar) with a hover highlight, and the pressed state keeps the accent fill. Evidence: `src/components/App.tsx`, `src/styles.css`. Not done: visual check in the running app.

- [x] T07 `[gui]` - Remove the "Version v1.0.0" text from the toolbar title block. Evidence: `src/components/App.tsx`, `tests/client.test.tsx`.

- [x] T08 `[gui]` - AI activity moves out of the toolbar into its own "AI Activity" widget above AI Chat (Chat gets shorter). AI Chat: bitmap Send button (`src/assets/send.png`), large flat message box in a bottom-anchored composer. Evidence: `src/components/{App.tsx,ChatPane.tsx}`, `src/styles.css`. Not done: visual check in the running app.

- [x] T09 `[gui]` `[bug]` - Chat and generation profile pickers move to the top of the AI Activity widget; both profiles are checked automatically in the background (15 s doubling, capped at 1 h) and the manual "Check generation profile" button is gone. Fix: the chat and generation controllers kept their own stale profile lists (status only changed on a manual check), so Generate stayed blocked as "not ready"; both now take the app-wide list from `useProfiles` via `applyProfiles`. Evidence: `src/components/{ProfilePicker.tsx,App.tsx}`, `src/application/{chat.ts,generation/controller.ts}`. Not done: test updates run, visual check.

- [x] T10 `[ux]` `[behavior]` - Candidates open as read-only Editor tabs (one per slot) at Generate without changing the active tab; choosing a candidate in the Explorer Draft tab activates and focuses it (owner, 2026-11-07); the candidate card list is removed and a banner holds status, Review and Discard. Evidence: `src/components/{candidateTabs.ts,GenerationPane.tsx,Explorer.tsx}`, `src/application/navigation/resources.ts`, `tests/generation-ui.test.tsx` (5 pass), eslint clean, `tsc` clean. Full vitest rerun and `scripts/gate.sh` pass (Z01; the `diary-logging` formatting was fixed with `cargo fmt --all`). Not done: visual check in the running app.

- [x] T11 `[behavior]` - Snippet tab default selection: eligible snippets whose local date is the selected day D start checked; D-1 and D+1 snippets start unchecked (owner, 2026-11-07). Evidence: `src/application/navigation/imports.ts`, `tests/navigation.test.ts` (new test; navigation suites 29 pass before the new test was added). Full vitest rerun passes (Z01); `tests/accessibility.test.tsx` now dates its snippet fixtures on the selected day. Not done: visual check.

- [x] T12 `[gui]` - Snippet tab: "Move to Trash" becomes a bitmap icon button (`src/assets/trash.png`, masked like Chat Send; `aria-label` and tooltip kept). Evidence: `src/components/Explorer.tsx`, `src/styles.css`. Full vitest rerun passes (Z01). Not done: visual check.

### Proof and docs

- [x] Z01 - Full gate (`scripts/gate.sh`) passes (2026-11-07): G01–G07 all exit 0. Fixes made on the way: `cargo fmt --all` on `diary-logging` `codec.rs`/`codec/tests.rs` (formatting only; the architecture test checks all workspace members) and the T11 default in `tests/accessibility.test.tsx`.
- [ ] Z02 - Smokes for touched areas pass.
- [ ] Z03 - Affected docs, phase plan, roadmap and changelog are updated. Done so far: `ux-gui.md`, `ux-terms.md`, `ux-information-architecture.md` (T01–T10), `sources-and-dates.md` and `ux-gui.md` for T11/T12, and a changelog entry for T01–T12. Open: tick this box once the owner finishes the visual checks.
