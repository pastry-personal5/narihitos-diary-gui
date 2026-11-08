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

- **T13–T16 (owner, 2026-11-07):** The Notion export shows each day as a Heading 2 titled `YYYY-MM-DD (WED)` followed by ordinary paragraph blocks, with no daily toggle. Old toggle days are not migrated: a leftover dated toggle is a structural conflict repaired by hand. Spec: [heading days](../superpowers/specs/2026-11-07-notion-heading-days-design.md).

- **T17–T19 (owner, 2026-11-07):** The Notion export reorders out-of-order day headings automatically, with no prompt; originals are deleted only after the copy is verified. Spec: [reorder days](../superpowers/specs/2026-11-07-notion-reorder-days-design.md).

- **T26 (owner, 2026-11-08):** Group settings by Server, Sources and AI Agents; move both profile selectors into AI Agents cards and allow Sources to reveal the active server configuration folder in Finder.

- **T28 (owner, 2026-11-08):** Move Trash recovery controls from Global Messages to a spacious Trash Recovery dialog opened from the Main Toolbar Strip, with a top-right close button and detailed status/action guidance.

- **T30 (owner, 2026-11-08):** Move AI Activities into a spacious modal opened from the Main Toolbar Strip, with a top-right close button, plain-language guidance, bitmap info buttons opening explanation overlays, and a brief current-activity summary in the bottom status line. This supersedes T08’s widget placement.

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

- [x] T13 `[behavior]` - Notion day title and body headings: `day_title` gives `YYYY-MM-DD (WED)`; every projected Markdown heading becomes `heading_3`. Evidence: `diary-domain` `notion::tests`. Passes: `cargo test -p diary-domain` (34): `day_title_has_iso_date_and_english_weekday`, `body_headings_never_project_as_heading_two`.

- [x] T14 `[behavior]` - Notion read side: a day is a dated `heading_2` plus its following siblings; ascending order, duplicates, legacy toggles and foreign blocks. Evidence: `export/notion/tests.rs`, `wire.rs` unit tests. Passes: `wire::tests` (`partition_splits_on_dated_heading_two_only`, `day_of_ignores_the_weekday_suffix`) and `export::notion::tests` (`duplicate_and_disordered_hierarchy_is_a_structural_conflict`, `a_legacy_dated_toggle_is_a_structural_conflict`, `a_foreign_block_in_a_day_is_a_structural_conflict_with_no_write`, `observing_an_absent_date_never_writes`, `pagination_reads_every_page_of_a_long_month`).

- [x] T15 `[behavior]` - Notion write side: create, backdate and replace as heading ranges with the O10 order and crash/lost-reply cases. Evidence: `export/notion/tests.rs`. Passes: `export::notion::tests` (create, ascending insertion, replace, crash and lost-reply cases, preamble, empty body, body headings, wrong weekday, hand-typed heading); `cargo test --no-fail-fast`: 577 pass, 0 fail.

- [x] T16 `[docs]` - Smoke scripts, architecture text, P1-M27 wording and changelog updated for heading days. Evidence: `scripts/smoke/export.py` fake leg, docs diff. Evidence: `scripts/smoke/export.py` prints `PASS export smoke`; `notion-live.py` compiles; architecture, P1-M27 and changelog updated.
  - [ ] T16.1 - Owner-run live round trip (`scripts/smoke/notion-live.py`) records Q1 and Q2 of the spec.

- [x] T17 `[behavior]` - Notion move planner and plain-text flag: `plan_moves` keeps the longest ascending run; `Remote.plain` marks formatted text. Evidence: `wire.rs` unit tests. Passes: `wire::tests` (`plan_moves_*`, `parse_page_marks_formatted_text_not_plain`). Tie rule: the earliest-positioned day stays.

- [x] T18 `[behavior]` - Notion reorder: `look` reports disorder instead of failing; `apply` copies, verifies and deletes out-of-place days, repairs identical twins, and refuses formatted, foreign, duplicate and legacy days without writing. Evidence: `export/notion/tests.rs`. Passes: `export::notion::tests` (`one_misplaced_day_moves_alone_and_the_requested_day_is_written`, `a_reversed_page_moves_the_minimum`, `the_requested_day_may_itself_be_the_moved_one`, crash/lost-reply cases, `a_formatted_day_is_not_moved`, `a_foreign_block_in_a_moved_day_refuses`, `two_different_texts_for_one_day_refuse`, nested items, small API pages, preamble); `cargo test --no-fail-fast`: 665 pass, 0 fail.

- [x] T19 `[docs]` - Architecture text, P1-M27 O8 note, changelog and a live reorder leg. Evidence: docs diff, `notion-live.py` compiles. Evidence: architecture text, P1-M27 note and changelog updated; `notion-live.py` has a reorder leg (compiles).
  - [ ] T19.1 - Owner-run live round trip records the reorder leg result (`Q3`).

- [x] T20 `[gui]` - Icon Bar: icons half as big (64 → 32 px), column narrower (80 → 48 px); idle icons use the darker `--muted` color and brighten to `--text` on hover (pressed keeps the accent fill). Evidence: `src/components/App.tsx`, `src/styles.css`. Not done: visual check in the running app.

- [x] T21 `[gui]` - Move connection status text and the Connect button from the toolbar into a new Settings > Server category.
  - [x] T21.1 - Server card preserves disconnected, connecting, connected and storage-unavailable states; Connect is disabled while connecting. Evidence: affected UI suites pass (45 tests), including Settings and client connection regressions.
  - [x] T21.2 - UI regression coverage and the full `scripts/gate.sh` pass (2026-11-08, G01–G07; local TCP fixtures require execution outside the sandbox); affected README, user guide, UX docs and changelog updated.

- [ ] T22 `[gui]` `[bug]` - Fix the Main Toolbar Strip layout, make custom bitmap icons render consistently as tinted masks, and provide hover tooltips for toolbar actions.
  - [ ] T22.1 - Toolbar title, actions and save status remain distinct and usable at the minimum window width.
  - [ ] T22.2 - Custom bitmap masks use alpha consistently and retain disabled/hover tinting.
  - [ ] T22.3 - Toolbar and icon-strip controls expose visible hover tooltips and accessible names.

- [x] T23 `[bug]` - Generate must work with a closed Entry tab and report preparation/admission failures in Global Messages.
  - [x] T23.1 - Regression coverage proves generation proceeds with the Entry tab open or closed without creating or saving it. Evidence: `tests/generation-ui.test.tsx` (6 pass).
  - [x] T23.2 - Failures appear once in Global Messages with actionable text. Evidence: `tests/activity-feedback.test.tsx`; `scripts/gate.sh` G01–G07 pass (2026-11-08; TCP fixtures run outside the sandbox). The original reported attempt could not be attributed to a specific failure from the retained logs.

- [x] T24 `[bug]` - Add typed generation preparation/admission diagnostics with stage, count and closed error tags. Evidence: controller tests (25 pass) verify stages, failure tags, redaction and nonblocking diagnostic rejection; native diagnostic tests validate allowlists and reject private fields. Logging/contribution docs updated; full `scripts/gate.sh` G01–G07 pass (2026-11-08; local TCP fixtures outside the sandbox).

- [x] T25 `[gui]` - Remove the active view heading (Date tab, Snippet tab, Draft tab and Entry tab) below the Explorer icons; retain icon tooltips, accessible names and selected state. Evidence: `src/components/Explorer.tsx` diff; gate G01, G02 and G04–G07 pass; G03 passes on rerun outside the sandbox for local TCP fixtures (2026-11-08).

- [x] T26 `[gui]` `[behavior]` - Settings categories and profile cards (owner, 2026-11-08).
  - [x] T26.1 - Server configuration/reload card is in Server; source status is in Sources; Agent profiles and separate Chat profile / Generation profile cards are in AI Agents, removed from Main. Evidence: Settings/profile UI suites, including independent profile choices retained when Settings closes and reopens.
  - [x] T26.2 - Sources reveals the active server configuration folder in Finder through an authenticated native action, with pending and error feedback. Evidence: API test verifies session/token enforcement, configured/unavailable folder and redacted ordinary settings; Settings UI test verifies disconnected, pending and sanitized error states. Native Finder interaction has not been manually checked.
  - [x] T26.3 - Affected README, user guide, UX, architecture, decisions and changelog updated. Evidence: `scripts/gate.sh` G01–G07 pass (2026-11-08; run outside the sandbox for local TCP fixtures); 363 frontend tests pass.

- [x] T27 `[docs]` - Reconcile `docs/ux*` with the agreed Settings categories and profile-card moves.
  - [x] T27.1 - GUI behavior, containment diagram and canonical terms agree on category/card placement, Finder reveal and profile choices. Evidence: reviewed all three `docs/ux*` files against T26 and Settings components; stale placement assertions and local-link checks pass.
  - [x] T27.2 - Markdown formatting, local links and whitespace checks pass. Evidence: `scripts/gate.sh` G01–G07 pass (2026-11-08; outside the sandbox for local TCP fixtures).

- [x] T28 `[gui]` `[behavior]` - Dedicated Trash Recovery dialog.
  - [x] T28.1 - Toolbar opens a spacious, responsive dialog with a top-right close button, keyboard dismissal and focus restoration; Global Messages no longer contains recovery controls. Evidence: `tests/client.test.tsx`; installed headless Chrome visual/layout smoke at 1280 × 900, 780 × 600 and 500 × 500 confirms a 960 × 740 desktop dialog, viewport containment, scrollable content, visible close button and restored toolbar focus.
  - [x] T28.2 - Explain current connection, discovery and operation states and available actions; retain inspection, explicit retry confirmation and completed dismissal. Evidence: `TrashRecovery.tsx`, `tests/trash-ui.test.tsx` covers connected/checking/error/read-only guidance and recovery → result → continuation → recovery, with no implicit confirmation. Existing partial/uncertain and completed-result tests pass.
  - [x] T28.3 - Regression tests, affected docs and full `scripts/gate.sh` pass. Evidence: G01–G07 pass (2026-11-08; outside sandbox for local TCP fixtures), 365 frontend tests pass; README, user guide, UX contracts, decisions, work plan and changelog updated.

- [x] T29 `[docs]` - Reconcile Trash Recovery across UX behavior, containment and canonical terms; use AI Agents as the canonical name for configured AI providers and the Settings category.
  - [x] T29.1 - Toolbar, dedicated recovery dialog, status/actions, return/close behavior and absence of recovery controls in Global Messages agree across all three UX documents. Evidence: dedicated Trash Recovery section and outcome/action table in `ux-gui.md`, modal flow in `ux-information-architecture.md`, control vocabulary in `ux-terms.md`; terminology and local link/anchor checks pass.
  - [x] T29.2 - AI Agents and singular AI Agent are defined explicitly, with existing AI Chat, AI Activity and profile-card terms retained; terminology, local links, formatting and full gate pass. Evidence: canonical component rows and AI terminology rules; local links/anchors and whitespace checks pass; `scripts/gate.sh` G01–G07 pass (2026-11-08, outside sandbox for local TCP fixtures), 365 frontend tests pass.

- [x] T30 `[gui]` `[behavior]` - Dedicated AI Activities window and brief bottom status.
  - [x] T30.1 - Toolbar opens a spacious viewport-bounded modal; close/Escape/focus work, and controls are removed from above AI Chat. Evidence: `tests/client.test.tsx`; headless Chrome layout/focus smoke at 1280 × 900, 780 × 600 and 500 × 500 (960 × 740 desktop, no horizontal overflow, body scrolling on the smallest window).
  - [x] T30.2 - Friendly status, result and cancellation guidance; bitmap info buttons open dismissible explanation overlays. Existing cancel, view, dismiss, discard and reconnect actions remain available. Evidence: `tests/activity-details.test.tsx` and `tests/client.test.tsx` verify actions, sanitized failure feedback, stale-state disabling, both info overlays and focus return.
  - [x] T30.3 - Status line reflects submission, running, waiting, stopping and disconnected observations; completion feedback remains active with the modal closed. Evidence: `tests/activity-status.test.tsx`, existing `tests/activity-feedback.test.tsx`; `ActivityFeedback` stays mounted in App independently of the dialog.
  - [x] T30.4 - Regression tests, layout smoke, affected docs and full gate pass. Evidence: `scripts/gate.sh` G01–G07 pass (2026-11-08; local TCP fixtures outside the sandbox), 368 frontend tests; browser smoke and maintained UX, architecture, README, user guide, decisions and changelog updates.

### Proof and docs

- [x] Z01 - Full gate (`scripts/gate.sh`) passes (2026-11-07): G01–G07 all exit 0. Fixes made on the way: `cargo fmt --all` on `diary-logging` `codec.rs`/`codec/tests.rs` (formatting only; the architecture test checks all workspace members) and the T11 default in `tests/accessibility.test.tsx`.
- [ ] Z02 - Smokes for touched areas pass.
- [ ] Z03 - Affected docs, phase plan, roadmap and changelog are updated. Done so far: `ux-gui.md`, `ux-terms.md`, `ux-information-architecture.md` (T01–T10), `sources-and-dates.md` and `ux-gui.md` for T11/T12, and a changelog entry for T01–T12. Open: tick this box once the owner finishes the visual checks.
