# P1-M35: Illustrated diary GUI overview

Status: Planned

Put the Illustrated diary in the app: create it from a day that has an Authentic diary, watch progress, read it with its photos, replace or delete it, and see engine status in Settings. Requires [P1-M34](milestone-34-illustrated-diary-overview.md).

## Entry conditions

P1-M33 and P1-M34 are Done; the kind's name (P1-M34 Q1) is fixed.

## Scope

- Stage A: application controller and bridge adapters for the P1-M34 routes; Explorer shows which days have an Illustrated diary.
- Stage B: **Create Illustrated Diary** action (toolbar and palette), enabled only when the selected day has a saved Authentic diary; progress and cancel in AI Activities; replace confirmation.
- Stage C: read-only viewer tab: rendered Markdown with photos; delete action with confirmation.
- Stage D: Settings card listing image engines with availability and a Check button; UX docs, user guide, native smoke, full gate.

Out of scope: editing the version; re-rendering one photo; choosing or editing photo prompts; picking the engine per creation in the GUI (config only); export; a gallery or lightbox; by-human tests as an exit gate (same policy as P1-M32).

## Open items

The [Atomic Editor research](../research/atomic-editor.md) assesses an additional option for Q2, including photo transport and strict read-only gaps. It is a proposal, not a dependency approval or a change to the editing scope.

- **Q1 - Where it opens:** proposal: a tab in the existing tabbed editor area, read-only, labelled with the date and kind, next to the day's Authentic tab. Alternative: a separate pane.
- **Q2 - Markdown rendering:** the app has a CodeMirror editor but may have no Markdown-to-HTML renderer. Proposal: reuse whatever the app already renders with; if none exists, show the text as read-only CodeMirror with photos as block widgets, which needs no new dependency. A renderer dependency needs the owner's approval.
- **Q3 - Photo transport:** proposal: fetch PNG bytes through a bridge command and show them as object URLs (same pattern as `Bitmap`/`get_symbol`), so the webview needs no file-system or asset-protocol scope.

## Completion checklist

Check only when evidence exists.

### Plan review

- [ ] R01 - Owner answered Q1-Q3; recorded here, in decisions and the changelog.
- [ ] R02 - Work plan re-read against current code (action registry, Explorer tables, tabbed editor resources, AI Activities, Settings cards, UX terms); corrections recorded in the [work plan](milestone-35-illustrated-diary-gui-architecture.md).

### Stage A - controller and Explorer

- [ ] C01 - Bridge adapters validate every response from the P1-M34 routes; malformed data is a typed error. Evidence: vitest.
- [ ] C02 - Explorer marks days that have an Illustrated diary and opens it; the list refreshes after create, replace and delete. Evidence: vitest.

### Stage B - create

- [ ] C03 - The action is disabled with a visible reason when the day has no saved Authentic diary, no engine is available or no agent profile is usable. Evidence: action-decision tests.
- [ ] C04 - A running creation appears in AI Activities with step progress (text, photo k of N) and Cancel; editing and autosave of the Authentic diary keep working meanwhile. Evidence: vitest plus synthetic smoke.
- [ ] C05 - Creating over an existing version asks for confirmation first; declining changes nothing. Evidence: vitest.
- [ ] C06 - Failure and cancel show a Global Message and leave the previous version on screen. Evidence: vitest.

### Stage C - view and delete

- [ ] C07 - The viewer shows text and photos in order, is read-only, and never offers save, review or export actions. Evidence: vitest.
- [ ] C08 - Photos have alt text, load lazily and release their object URLs when the tab closes. Evidence: vitest.
- [ ] C09 - Delete asks for confirmation and removes the Markdown and photo directory. Evidence: vitest plus smoke.

### Stage D - settings, docs and proof

- [ ] C10 - Settings lists image engines with availability, reason and Check. Evidence: vitest.
- [ ] C11 - Keyboard access and reduced-motion behaviour match the existing dialogs and tabs; new terms are in `docs/ux-terms.md`. Evidence: tests and doc diff.
- [ ] C12 - `docs/product-behavior.md`, `docs/ux-gui.md`, `docs/ux-information-architecture.md`, `docs/user-guide.md`, `README.md`, decisions, changelog and docs index updated.
- [ ] C13 - Native smoke: create with fake agent and fake engine, view, replace, delete. Evidence: smoke output.
- [ ] C14 - `scripts/gate.sh` passes.
