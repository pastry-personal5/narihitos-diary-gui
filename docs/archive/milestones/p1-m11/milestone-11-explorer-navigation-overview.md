# P1-M11: Explorer and editor navigation overview

Status: Done (archived)

Connect date imports and bounded source views to the Standard Layout, and give each open resource its own editor state. This milestone delivers navigation and selection without enabling producer writes, Trash or AI. The [work plan](milestone-11-explorer-navigation-architecture.md) defines placement, sequencing and evidence.

## Entry conditions

- P1-M10 and its [snapshot hardening follow-up](../p1-m10/milestone-10-import-hardening-overview.md) are Done. Preserve immutable completed listings and fresh selected-source validation.
- Follow [Standard Layout](../../../ux-information-architecture.md), [navigation and transitions](../../../ux-gui.md), [source/date contracts](../../../architecture/sources-and-dates.md) and handoff E3/E9. Read the applicable evidence gaps in [AGENTS.md](../../../../AGENTS.md).
- No new dependency, build tool or owner product decision is proposed. Existing React, CodeMirror, Rust date types and approved Jiff suffice. Seek approval if implementation discovers otherwise.

## Scope

Included: resizable Explorer/editor/AI Chat columns; editor-column Global Messages; calendar and Today; grouped paged imports and inclusion checkboxes; Metadata warnings and explicit timestamp choices; saved-date navigation; resource-owned Editor tabs; guarded reads, refresh, disconnect and transitions. Keep Settings and Authentic autosave working.

P1-M12 owns the shared action registry, command palette and shortcut customization. P1-M13 owns SF Symbols and comprehensive packaged VoiceOver/IME qualification; this milestone still implements and smoke-checks basic keyboard, focus and resize behavior. P1-M14/P1-M15 own producer-compatible source Save and Trash. P1-M21–P1-M24 own chat, candidates, background AI and review. Those unavailable controls explain their state; selection alone does not enable Generate. No persistent imported body, tab history, source writer or speculative AI result is added.

## Completion checklist

Check only with recorded evidence. Planning does not complete these implementation items.

- [x] N01 — Standard Layout containment matches the UX contract; pointer/keyboard separators clamp to usable pane sizes, including Global Messages height and small-window behavior.
- [x] N02 — An authenticated server calendar supplies validated civil fields, bounded/clamped month/day movement and Today, including initial connection/retry; React never parses date-only strings through JavaScript `Date`. Leap/year/month boundaries and a skipped selected date produce the specified results.
- [x] N03 — Date selection admits an import, tracks its real terminal state, and pages its immutable snapshot. Loading, failure, cancellation, incomplete scans, empty results and continuation are distinguishable; stale replies cannot replace another date/context. Progressive pages validate provenance/cursor advancement, retry safely and defer absence reconciliation until complete coverage; polling stops on disconnect/disposal.
- [x] N04 — Source/local-date groups show eligible snippets with independent open/focus and checkbox actions. Unchanged identities/digests preserve inclusion on refresh; new eligible sources start checked; resolved classification is distinct from informational warnings; changed/missing/conflicting observations are visibly invalidated.
- [x] N05 — Metadata warnings show available timestamp interpretations and validity reasons. Explicit valid authority choices are digest-bound, leave sources untouched, and take effect through a new import; stale choices and invalid filename authority fail visibly.
- [x] N06 — A typed, bounded `read_source` route/command reads only a selected snapshot identity with fresh observation validation. Body-only views expose capability reasons and per-file errors; unsupported/identity-conflicting-but-viewable rows retain safe snapshot read descriptors and work after restart; legacy snapshots require re-import where descriptors are absent. Views work without exposing roots, raw metadata, native identifiers or arbitrary path access.
- [x] N07 — Source IDs and Authentic dates own distinct Editor states; Draft/Entry share one Authentic state per date. Switching preserves text, selection, undo, scroll and admitted saves; stale loads/acknowledgements cannot edit another resource. Epoch checks precede buffer writes, and equal-revision refresh preserves history. Body/history/retry/cache budgets refuse excess work without losing dirty/in-flight state.
- [x] N08 — Serialized date/close guards protect IME composition and settle Authentic/admitted saves and creation; app close includes every inactive session and blocks on failure. Dirty-source refresh/close guards retain base observation and local text, require explicit Save or Discard, and never treat unsupported Save as success. Disabled source writers remain disabled.
- [x] N09 — Entry pages all saved dates with explicit continuation/errors, including empty files after restart; Draft opens the same selected-date Authentic. Refresh/reconnect/config reload preserve local buffers, reconcile uncertain operations using their original IDs before writes resume and invalidate stale source actions safely. Watcher hints cover inactive Authentic sessions.
- [x] N10 — Global Messages reports navigation/import/save/connection outcomes oldest first with time, severity text/icon/color, polite announcements, bounded session retention and Jump to latest; it never steals focus or includes private backend diagnostics.
- [x] N11 (packaged smoke WAIVED by owner 2026-11-06; automated tests only) — Focused Rust/bridge/Vitest tests cover N01–N10; the [packaged synthetic navigation smoke](milestone-11-explorer-navigation-architecture.md#packaged-synthetic-navigation-smoke-procedure) records tab undo/focus, date/import/resolve/refresh, Create Empty/restart and separator behavior. Evidence limits for producer/provider/native accessibility stay explicit.
- [x] N12 — Affected README, contracts, handoff, phase, roadmap and changelog reflect actual behavior and limitations; completed plans are archived only after every item and gate passes.

## Plan-review completion checklist

These checks cover the requested plan review only; N01–N12 and G01–G07 remain implementation requirements.

- [x] R01 — Compare the plan with maintained UX/source/API contracts, milestone sequencing and current code; record concrete findings and corrections in the [work plan](milestone-11-explorer-navigation-architecture.md#plan-review-findings-and-corrections).
- [x] R02 — Map corrected transport, read-descriptor, pagination, eligibility and resource-lifecycle requirements to implementation acceptance evidence without enabling deferred features or changing dependencies.
- [x] R03 — Format changed plans, check local links and whitespace, and review the scoped diff; record planning validation here.

## Implementation gate

- [x] G01 — `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [x] G02 — `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`
- [x] G03 — `cargo test --manifest-path src-tauri/Cargo.toml`
- [x] G04 — `npx prettier --check .`
- [x] G05 — `npx eslint . --max-warnings 0`
- [x] G06 — `npx vitest run`
- [x] G07 — `npx tsc --noEmit` (additional project check)

Run affected tests during implementation and `scripts/gate.sh` once at the end. If a command or packaged smoke cannot run, leave its item unchecked and record the blocker. The P1-M1/P1-M2 docs-only exception does not apply to completion of this milestone.

## Evidence record

Plan review completed 2026-11-05. Static evidence and six finding/correction mappings are in the [review record](milestone-11-explorer-navigation-architecture.md#plan-review-findings-and-corrections), with expanded acceptance cases and a reproducible packaged smoke procedure. Reviewed maintained UX/source/API contracts, phase/handoff ordering and the current hook/session, source scan, adapter, import and snapshot code.

Planning validation passed: targeted `npx prettier --check` for both P1-M11 plans, the changelog and documentation index; 119 local file/anchor checks across those four documents; whitespace checks including the untracked plans; and scoped `git diff --check`. Reviewed final plan contents and change scope. No application code or dependencies changed in this review. The implementation gate and packaged smoke were not run; N01–N12/G01–G07 remain unchecked and P1-M11 stays Planned. Record runtime test locations/results, packaged environment and remaining limitations here during implementation.

Implementation started 2026-11-05 against N01–N12 and G01–G07 above. Existing P1-M10 hardening and planning changes are retained. Backend calendar, versioned navigation provenance and selected-snapshot reads are the first implementation task; checklist items remain unchecked until their full evidence exists.

Implementation progress: the server calendar, snapshot navigation provenance, selected-source read boundary, typed bridge, import controller, retained resource coordinator, and focused tests are in progress. The infrastructure suite passed with native macOS access (128 tests); the API suite passed (40 tests). Focused navigation, resource, and save-queue tests passed (36 tests). The new controllers are not yet connected to the Explorer UI; packaged smoke and the full implementation gate remain outstanding, so no completion item is checked.

The Explorer and editor controllers are now connected to the application shell. Existing Authentic, Settings and client UI tests pass (20 tests), and TypeScript typechecking passes. Calendar keyboard navigation, separators, complete Global Messages behavior, source refresh guards, comprehensive integration tests and packaged smoke still require implementation and verification; the milestone remains in progress.

The calendar now uses roving focus and server-backed arrow movement without committing a date; the Explorer, AI Chat and Global Messages have pointer/keyboard separators, and Global Messages supports bounded retention and explicit scrolling back to the newest notice. Focused component and existing UI tests passed (23 tests), followed by calendar/component tests (11 tests). Native keyboard/pointer smoke, responsive pane constraints, source refresh guards and full acceptance remain outstanding.

Source refresh now guards dirty buffers and validates a newly loaded snapshot before replacing text or observation metadata. A regression test covers Cancel, unavailable Save, failed fresh read and explicit Discard followed by a successful read. Connection transitions also reject superseded asynchronous reconnects; window focus refreshes server Today, and closing Settings restores focus. Focused resource, import, Authentic and Settings tests passed (38 tests); TypeScript and scoped ESLint passed. Full milestone acceptance and packaged smoke remain open.

Explorer results now group each source's included rows by the captured local date and UTC offset, with unresolved rows kept under an attention group. Saved-date refresh requests arriving during an in-flight catalog read are coalesced and replayed, with continuation retries retaining the current page. A configuration generation change now pauses resource sessions, invalidates import contexts and reconnects before source actions resume. Focused Settings, Authentic and catalog tests passed (17 tests), as did TypeScript and scoped ESLint. Additional integration and native evidence remain outstanding.

Import lifecycle fixes now allow a new connection to poll while a superseded lookup remains in flight, retry terminal failures through a new admission, release import-context protection when source tabs close, and check newly eligible resolved sources while preserving prior explicit exclusions. Focused import and resource tests passed (26 tests), with TypeScript checking. Packaged acceptance and the full gate remain open.

Boundary review tightened calendar response validation to verify Gregorian weekdays and clamped adjacent months. Selected-source reads and timestamp resolutions now reject scanned candidates that were not published in the requested snapshot. Focused calendar/import/resource tests passed (26 Vitest tests and 13 source-import Rust tests); full acceptance remains open.

A selected unsupported/conflicting source is now readable only when its descriptor has a matching published failure row from the same root; this also covers scans truncated by their budgets. Failed Authentic reconnect loads remain paused and can be retried without losing the buffer, and superseded debounce timer handles are released. Focused Rust source-import tests passed (14 tests), resource tests passed (12 tests), and TypeScript/scoped ESLint passed. Milestone acceptance remains open.

Create Empty recovery now queries the original operation even if the file exists after reconnect, refuses a repeat over an existing file when the operation is unknown, and presents a retry action. Regression tests cover both recorded success and an unknown outcome. Resource, Authentic UI and save-queue tests passed (36 tests); TypeScript and scoped ESLint passed. Packaged acceptance and the milestone gate remain open.

Reconnect now waits for an in-flight Create Empty operation before resuming writes, permits an explicitly retried definitive failure to use a fresh operation ID, and pauses all retained sessions immediately on disconnect. Focused resource tests passed (16 tests), with TypeScript and scoped ESLint checks. Full acceptance and packaged smoke remain open.

Connection attempts and calendar Today requests now use epochs: late settings/subscription replies cannot reconnect after a disconnect, and an older Today response cannot override a newer browse. Focused client, Authentic and Settings UI tests passed (22 tests); the client/model race tests passed (8 tests). TypeScript and scoped ESLint were also checked. Full milestone and packaged acceptance remain open.

Explicit buffer replacement now resets undo history even if the resulting text is identical; equal-revision refresh continues to preserve history. Authentic editors remain read-only while creation or reload errors require reconciliation. Focused resource tests passed (17 tests), with TypeScript and scoped ESLint checks. Native and full-gate evidence remains outstanding.

Import cancellation requested during admission is retained until the server returns the job ID; superseding a date uses the same path, and failed cancellation continues terminal-state polling. Focused import tests passed (17 tests), along with TypeScript and scoped ESLint. The full gate and packaged smoke remain outstanding.

Open source tabs are now marked stale when their owning date starts a refresh or publishes a different snapshot, including when that date is inactive. Their text and base observation remain intact until an explicit guarded refresh succeeds. Focused resource, import and navigation-model tests passed (36 tests), with TypeScript and scoped ESLint checks. Full acceptance remains open.

Gate run 2026-11-06 (`scripts/gate.sh`): G01–G07 all PASS after fixing three `clippy::collapsible_if` findings in `diary-api` and `diary-infrastructure` and reformatting with `cargo fmt --all`. The packaged synthetic navigation smoke (N11) has not been run, so N01–N12 stay unchecked and the milestone stays In progress.

Owner-run `scripts/smoke/server.py` against the debug-bundled `Narihito's Diary.app` (39.85 MiB) passed: one native window, editor focus and editability probes, IPC allow/deny observations, private log modes, trace correlation, lock exclusion, signal cleanup and restart rotation. It is regression evidence for the shell and server only; it does not exercise the Explorer, tabs, calendar, imports or separators, so N11 stays open until the navigation smoke procedure is run.

Owner decision 2026-11-06: the packaged synthetic navigation smoke is skipped. Tab undo/focus, date/import/resolve/refresh, Create Empty/restart and separator behavior have no packaged-app evidence; only automated tests cover them. N11 remains unchecked because its smoke clause has no evidence.

## Completion record

Owner waiver (2026-11-06): the owner skipped the packaged synthetic navigation smoke and directed that P1-M11 close without it. This is a recorded exception to the N11 smoke clause, not packaged-app evidence. N01–N12 and G01–G07 are checked on the following basis:

- Automated evidence: `tests/navigation.test.ts` (calendar/civil dates, import admission, polling, paging, provenance, resolution choices), `tests/navigation-model.test.ts`, `tests/navigation-components.test.tsx` (separator bounds/keyboard, Global Messages retention and Jump to latest), `tests/catalog.test.ts` (saved-date paging and refresh), `tests/resources.test.ts` (resource-owned tabs, undo/selection restoration, guards, Create Empty reconciliation, dirty-source handling), plus the Rust calendar, snapshot and `read_source` suites. Full gate passed 2026-11-06.
- Limits that remain unverified in a packaged app: tab undo/focus, date/import/resolve/refresh flow, Create Empty after restart, pointer separator dragging and small-window behavior. Native VoiceOver/IME qualification remains with P1-M13; producer Save and Trash remain with P1-M14/P1-M15. If a packaged defect appears, reopen it as a follow-up rather than editing this record.
