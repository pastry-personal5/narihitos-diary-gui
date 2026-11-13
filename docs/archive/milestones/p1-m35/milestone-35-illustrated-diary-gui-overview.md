# P1-M35: Illustrated diary GUI overview

Status: Done

Put the Illustrated diary in the app: create it from a day that has an Authentic diary, watch progress, read it with its photos, replace or delete it, and see engine status in Settings. Requires [P1-M34](../p1-m34/milestone-34-illustrated-diary-overview.md).

## Entry conditions

P1-M33 and P1-M34 are Done; the kind's name (P1-M34 Q1) is fixed.

## Scope

- Stage A: application controller and TypeScript adapters for the existing P1-M34 native commands; add missing image-engine discovery/check bridges; a dedicated Illustrated Explorer tab lists available dates.
- Stage B: **Create Illustrated Diary** action (toolbar and palette), enabled only when the selected day has a saved Authentic diary; progress and cancel in AI Activities; silent replacement.
- Stage C: read-only viewer tab: rendered Markdown with photos; delete action with confirmation.
- Stage D: Settings card listing image engines with availability and a Check button; UX docs, user guide, native smoke, full gate.

Out of scope: editing the version; re-rendering one photo; choosing or editing photo prompts; picking the engine per creation in the GUI (config only); export; a gallery or lightbox; by-human tests as an exit gate (same policy as P1-M32).

## Open items

The interview is complete. Answers below are owner decisions; implementation qualification and acceptance evidence remain outstanding. The [Atomic Editor research](../../../research/atomic-editor.md) supplies qualification risks, not runtime acceptance.

- **Q1 - Where it opens (resolved, owner 2026-11-13):** a separate read-only tab in the existing tabbed editor area, labelled with date and kind, beside the day's Authentic tab.
- **Q2 - Markdown rendering (resolved, owner 2026-11-13):** Atomic Editor rendered reading view; dependency qualification approved. Inspect and pin the actual package and required direct peers, verify resolved licenses/notices and prove strict immutability and local photo rendering before adoption. This does not approve editable preview or build-tool changes. If qualification fails, return with findings rather than silently changing the viewer.
- **Q3 - Photo transport (resolved, owner 2026-11-13):** reuse the existing authenticated `read_illustrated_photo` command and display temporary object URLs; add `blob:` narrowly to CSP `img-src` with packaged proof. Filesystem or remote-image access is permitted if needed for transport, but M35 images remain limited to generated diary photos; no support for additional local or remote image content is included. `Bitmap` is a CSS mask component, not a photo loader.
- **Q4 - Agent profile (resolved, owner 2026-11-13):** reuse the Generation profile; no independent Illustrated profile selector. Capture its identity with the date and writing locale at submission, and validate capability for the Illustrated action.
- **Q5 - Explorer access (resolved, owner 2026-11-13):** add a dedicated Illustrated Explorer tab with an independent catalog, including entries whose Authentic file is absent.
- **Q6 - Completion and source age (resolved, owner 2026-11-13):** open the result if still on the originating day; otherwise notify in Global Messages without changing the working date or focus. Update an already-open viewer after successful replacement. When the saved Authentic changes from the captured source revision, show a quiet source-changed notice in Global Messages. Deduplicate notices; do not regenerate automatically.

The M34 decisions remain binding: settle pending Authentic saves before capture, silent successful replacement, confirmed deletion and all-or-nothing publication. The owner then requested M35 completion. Implementation and observed acceptance are recorded in the [evidence](milestone-35-illustrated-diary-gui-evidence.md); status/checkmarks follow that evidence.

## Completion checklist

Check only when evidence exists.

### Plan review

- [x] R01 - Owner answered Q1-Q6; recorded here, in decisions and the changelog.
- [x] R02 - Work plan re-read against current code (action registry, Explorer tables, tabbed editor resources, AI Activities, Settings cards, UX terms); corrections and inspected paths recorded in the [work plan](milestone-35-illustrated-diary-gui-architecture.md#code-review-findings-2026-11-13).
- [x] R03 - Interview choices translated into explicit task acceptance, including profile selection, Explorer access and completion/focus behavior; remaining implementation questions resolved before affected work starts.

### Stage A - controller and Explorer

- [x] C01 - Bridge adapters validate every response from the P1-M34 routes; malformed data is a typed error. Evidence: vitest.
- [x] C02 - The dedicated Illustrated Explorer tab lists available dates and opens their read-only resources; the list refreshes after create, replace and delete. Evidence: vitest.

### Stage B - create

- [x] C03 - The action is disabled with a visible reason when the day has no saved Authentic diary, the effective configured engine is not ready, the selected Generation profile cannot run Illustrated, storage is read-only, the locale is unresolved or admission is busy. Evidence: action-decision tests.
- [x] C04 - A running creation appears in AI Activities with step progress (text, photo k of N) and Cancel; editing and autosave of the Authentic diary keep working meanwhile. Evidence: vitest plus synthetic smoke.
- [x] C05 - Creating over an existing version silently replaces it only after successful publication; failed creation preserves the previous version. Evidence: vitest.
- [x] C06 - Failure and pre-publication cancellation show a Global Message and retain any previous readable version; cancellation after publication reconciles to the authoritative successful version. Evidence: vitest.

### Stage C - view and delete

- [x] C07 - The viewer shows text and photos in order, is read-only, and never offers save, review or export actions. Evidence: vitest.
- [x] C08 - Photos have alt text, load lazily and release their object URLs when the tab closes. Evidence: vitest.
- [x] C09 - Delete asks for confirmation and removes the Markdown and photo directory. Evidence: vitest plus smoke.

### Stage D - settings, docs and proof

- [x] C10 - Settings lists image engines with availability, reason and Check. Evidence: vitest.
- [x] C11 - Keyboard access and reduced-motion behaviour match the existing dialogs and tabs; new terms are in `docs/ux-terms.md`. Evidence: tests and doc diff.
- [x] C12 - `docs/product-behavior.md`, `docs/ux-gui.md`, `docs/ux-information-architecture.md`, `docs/user-guide.md`, `README.md`, decisions, changelog and docs index updated.
- [x] C13 - Native smoke: create with fake agent and fake engine, view, replace, delete. Evidence: smoke output.
- [x] C14 - `scripts/gate.sh` passes.
- [x] C15 - Creation settles pending Authentic saves, captures the requested date/profile/locale once, prevents duplicate submissions and handles uncertain admission without starting duplicate jobs. Save failure prevents submission. Evidence: controller/race tests.
- [x] C16 - Illustrated progress survives native/TypeScript validation, paginated snapshot/replay and reconnect alongside Chat and Generation; malformed progress is rejected without misattributing a job. Evidence: bridge, event and activity tests.
- [x] C17 - Photo loads are version-bound, concurrency/memory bounded and deferred until needed; close, replacement, disconnect and stale completion release owned allocations/URLs. Failed photos have accessible retry/fallback. Evidence: adapter tests plus packaged CSP smoke.
- [x] C18 - Version-bound delete conflicts and cancellation after publication reconcile with server state; an old success/failure never overwrites a newer view or deletes a newer version. Evidence: race tests and synthetic smoke.
- [x] C19 - Existing Chat context, Generate, Paragraph Review, save/export actions and status-line activity behave deliberately with an Illustrated tab active. Evidence: action/context and activity tests.
- [x] C20 - Source revision changes produce one quiet Global Message per observed source/version change; background completion opens only when still on the originating day and otherwise posts a Global Message. Evidence: controller and focus tests.

Completion evidence: [acceptance record](milestone-35-illustrated-diary-gui-evidence.md).

## Completion record

Completed 2026-11-13. All C01–C20 acceptance checks are supported by the [evidence record](milestone-35-illustrated-diary-gui-evidence.md). Final G01–G07 PASS (`run.9Sko0I`), 500 frontend tests across 68 files, 15 Python harness tests, isolated synthetic/packaged native proof and refreshed ordinary release artifact/process/agent checks. M32 remains Active.
