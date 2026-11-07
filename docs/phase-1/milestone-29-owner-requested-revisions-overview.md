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

- [ ] T03 `[gui]` - Define one dark-theme color palette (named tokens, roles and contrast targets) and use it across the GUI; no hard-coded colors outside the token block.
  - [x] T03.1 - Owner approves the palette (design in chat, 2026-11-07).
  - [x] T03.2 - Tokens defined once in `src/styles.css` and documented in `docs/ux-gui.md`; text roles meet WCAG AA (4.5:1) on every surface; selected-state text uses an on-accent token (white on the mint accent is 1.9:1 today).
  - [x] T03.3 - A test checks that `src/styles.css` has no color literals outside the token block.

### Proof and docs

- [ ] Z01 - Full gate (`scripts/gate.sh`) passes.
- [ ] Z02 - Smokes for touched areas pass.
- [ ] Z03 - Affected docs, phase plan, roadmap and changelog are updated.
