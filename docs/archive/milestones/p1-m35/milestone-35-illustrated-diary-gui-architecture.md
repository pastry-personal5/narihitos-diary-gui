# P1-M35: Illustrated diary GUI work plan

Status: Done

Work plan for the [P1-M35 overview](milestone-35-illustrated-diary-gui-overview.md). Reuse existing resource, action and activity owners; qualify Atomic Editor for an immutable reading surface. Owner interview answers are authoritative in the overview. Completion evidence is linked there.

## Code review findings (2026-11-13)

- `src-tauri/src/bridge/illustrated.rs`, `src-tauri/src/main.rs` and `src-tauri/capabilities/main.json` already implement/register/permit list, read, create, delete and version-bound photo commands. T01 adds TypeScript contracts/adapters and missing coverage rather than duplicating those native commands. Inspect native response validation as part of that work.
- `src-tauri/crates/diary-api/src/images.rs` exposes cached list and explicit check routes, but the frontend bridge has no engine methods. Add minimal explicit native commands, validation and permissions for these routes. An engine list alone does not disclose the effective `illustrated.engine` selection; add a content-free readiness projection for that selection and relevant profile capability. The GUI must not infer availability from any unrelated ready engine or generic chat readiness.
- `src/application/ai-activities/progress.ts` accepts only Chat and Generation and requires fields absent from Illustrated progress. `src/application/events.ts`, the native progress bridge and activity adapters must accept the actual Illustrated projection through a discriminated variant. An invalid Illustrated record must not silently discard activity tracking for unrelated jobs. Inspect baseline/replay, status-line counts, details, cancellation and reconnect together.
- `src/application/navigation/resource.ts` supports only Authentic, Source and Candidate. `resources.ts` owns identity, load epochs, transition guards and the 32-tab limit; retain one resource map. Illustrated must not accidentally enter Authentic save, Candidate adoption or paragraph-review paths.
- `src/application/generation/controller.ts` already prepares saved Authentic state through `flushAuthentic`; reuse that policy boundary for Illustrated creation. Capture the originating date before awaiting, block submission on failed settling, and do not read a later selected date/profile after an asynchronous boundary.
- `src/components/explorer/Explorer.tsx` owns existing Explorer surfaces. Add the owner-selected dedicated Illustrated tab and its independent catalog; do not depend on Authentic catalog membership for access.
- `src/components/settings/SettingsView.tsx` uses `src/components/ai-agents/ProfileTable.tsx`; reuse accessible presentation/check timing patterns without importing its feature hooks as application policy. Reuse the Generation profile choice and expose capability for Illustrated assets, not merely Chat availability.
- `src/components/Bitmap.tsx` draws CSS masks; it is not a photo transport precedent. Existing CSP lacks `blob:`. Photo transport needs the approved narrow image CSP addition plus packaged WKWebView evidence.
- `src/application/navigation/budget.ts` accounts retained payload bytes, not decoded image memory or process RSS. Define separate bounded photo ownership/concurrency/decode limits before implementing loaders; no unbounded cache or implicit reliance on browser lazy loading.

## Design and ownership

The application controller (`src/application/illustrated/`) owns its catalog, admission state and reconciliation, consumes shared activity snapshots and exposes an immutable snapshot. ResourceCoordinator continues to own tab identities and transitions. Keep React, CodeMirror, Tauri, Blob and object-URL APIs outside `src/application`; inject adapters through composition. Do not mirror shared job maps, queues or reservations.

An Illustrated resource has stable identity by date and a replaceable version UUID. Bind each read/photo result to connection scope, resource instance, version and load epoch. Reject stale completions after close, reopen, replacement or reconnect. Keep the previous readable version until a new read succeeds; never mix one version's body with another version's photos. On reconnect or event resync, refresh catalog and revalidate open resources against the authoritative server. Offline retained text must be labelled stale; prohibit mutations and new photo requests until reconnected.

### Viewer qualification

Compose qualified Atomic Editor extensions inside `src/editor`, preserving app resource ownership rather than adopting the upstream React wrapper. Inspect the actual pinned package, direct CM/Lezer peers, deduplication, licenses/notices and bundle before changing dependencies. No build-tool replacement is authorized. If qualification fails, report findings and reopen the renderer decision.

Reject document-changing transactions, including checkbox/table widgets, paste, drop, undo and explicit dispatch. Only an owner-controlled version load can replace the body. Preserve selection, copy, keyboard navigation and reading position where practical; never expose save, review, candidate adoption or export for the Illustrated resource. Test existing Chat context fallback and other toolbar decisions explicitly rather than allowing a new resource kind to inherit an accidental default.

Render only the returned body, excluding durable private metadata and prompts. Resolve image references only when they match the returned date/photo manifest. Replace or disable every upstream image-loading path, including tables; forbid raw HTML and traversal. The owner permits filesystem or remote-image access if needed for transport, while limiting M35 content to generated diary photos. The selected bridge/object-URL design needs neither broad access grant nor additional image sources; validate every rendered image against the generated manifest. If another transport is needed, document its bounded roots/origins and minimum permissions. Route links through an explicit validated app policy; no upstream default `window.open` behavior. Use app CSS tokens and reduced-motion rules. Keep this scope read-only.

Photos use the existing version-bound PNG bridge and app-owned widgets. Add `blob:` only to CSP `img-src`. Defer bridge requests until viewport demand, bound concurrent loads, retained encoded bytes and decoded pixels, and evict/release on tab lifecycle changes. Record concrete bounds and reservation/refusal behavior before T08; accommodate the server's 32 MiB per-photo ceiling and up to 99 photos without fetching all eagerly. Failed loads show alt text, an accessible fallback and explicit Retry. Revoke URLs and release temporary bytes on errors and stale completion as well as normal disposal. Packaged smoke must prove local photos render and no remote image requests occur.

### Creation, activities and reconciliation

Use one shared action decision for toolbar and palette with a distinct generated toolbar PNG, tooltip and disabled reason. A shortcut-table entry does not imply assigning a new default chord. Availability includes connection, writable storage, selected date with saved Authentic, settled saves/review, usable Generation profile and Illustrated prompt assets, resolved writing locale, effective configured engine readiness and admission limits. Reflect unchecked, checking, unavailable and busy engines deliberately; Check only probes and never renders. Server admission remains authoritative.

Capture date/profile/locale and a request ID once; reuse the same ID for uncertain admission reconciliation. Settle pending saves before submitting, and do not freeze Authentic editing for the render duration. Submission acknowledgement is not publication. A later edit makes the captured source older; it must not alter the running job's inputs. Concurrent clicks and requests for a reserved date cannot create duplicate jobs.

Extend the shared activity discriminant, metadata validation, native projection, snapshot/replay, status-line presentation and details. Show text preparation, photo k of N, stopping and terminal outcomes without source text or prompts. Existing cancellation is a request; reconcile terminal server state because cancellation after the publication boundary can still yield success. Closing the viewer or activity dialog does not cancel creation. On completion, open the result only if the selected date still matches its origin; otherwise post a Global Message without moving focus. Refresh an existing viewer only after publication. Compare source identity/revision with authoritative saved Authentic state at load/reconciliation and after successful saves. A mismatch produces a quiet, deduplicated Global Message; unsaved keystrokes alone do not prove a saved revision change. Treat missing/unavailable source as a distinct state, not evidence of a changed revision. No automatic recreation.

On authoritative success, refresh catalog and any open viewer for that date; ignore stale terminal callbacks from old sessions/jobs. On failure or pre-publication cancellation, retain the previous version and post a Global Message. On reconnect, reconcile uncertain jobs and durable state before permitting retries. No parallel polling owner beside AIActivitiesRuntime.

Deletion is explicit, confirmed and bound to the displayed version UUID. Cancel does nothing. Send the confirmed version unchanged; on conflict, reload and require fresh confirmation rather than deleting a newer version. During an active same-date creation follow server busy rules. Successful deletion releases/updates the open resource and catalog without changing unrelated Authentic content. Define the focus fallback using existing tab-close behavior. Refresh reads must handle another client or external change without issuing deletes.

### Engine status

Settings shows content-free engine ID/label, effective selection, availability/reason and Check, using explicit native adapters. Never expose executable paths, triggers or prompt directives. Keep engine selection in server config; configuration/Illustrated changes require the existing restart policy. Recheck readiness after reconnect/restart; an unrelated engine becoming ready cannot enable the action. Reuse shared readiness state between Settings and the action, with one owner and invalidation policy.

## Stages and acceptance mapping

### Stage A - qualification, contracts, controller and Explorer (T01-T03)

- T01 - Qualify Atomic package/peer/license and strict read-only feasibility; add validated TypeScript Illustrated adapters, engine list/check native bridges and effective readiness projection. Define the Illustrated progress variant and extend native and frontend snapshot acceptance early. Covers C01, C16 and the prerequisites for C10/C17.
- T02 - Controller/catalog, connection epochs, stable resource identity and reconciliation; specify concrete photo budgets and load policy. Covers C02, C15 and C18 prerequisites.
- T03 - Dedicated Illustrated Explorer tab, independent catalog loading/error/empty/retry states and keyboard-open behavior. Entries remain accessible without an Authentic file. Covers C02 and C11.

### Stage B - create (T04-T06)

- T04 - Shared action, distinct toolbar PNG, palette and shortcut-table entry; Generation profile capability, effective engine and saved-source guards. Covers C03 and C15.
- T05 - Complete shared activity/progress/cancel/status-line integration, scoped replay/reconnect and uncertain-admission handling. Covers C04 and C16.
- T06 - Successful publication reconciliation, silent replacement, failure/cancel messages and owner-selected completion/focus behavior. Covers C05, C06, C18 and C20.

### Stage C - view and delete (T07-T09)

- T07 - Strictly immutable Atomic resource/viewer, app action/context guards and source-change Global Messages. Covers C07, C19 and C20.
- T08 - Manifest-only lazy photo widgets, narrow CSP change, stale-load cleanup, bounded allocations and failed-image Retry. Covers C08 and C17.
- T09 - Version-bound confirmed delete, busy/conflict handling and focus/catalog/resource cleanup. Covers C09 and C18.

### Stage D - settings, docs and proof (T10-T13)

- T10 - Engine status card and shared readiness/check behavior. Covers C10.
- T11 - Terms, dedicated Explorer tab, Settings placement, selected profile, rendered viewer, actions/context behavior and all affected docs in overview C12. Covers C11/C12.
- T12 - Isolated fake-agent/fake-engine server and packaged native proof: create/view/replace/delete, cancellation/failure preservation, background completion, reconnect, photo CSP/fallback and cleanup. Distinguish route, DOM and packaged evidence; synthetic HTTP smoke alone does not prove WKWebView rendering. Covers C04/C09/C13/C17/C18.
- T13 - Full `scripts/gate.sh`; record evidence and rerun affected M32 automated/artifact qualification, updating its release note. Covers C14; M35 does not close M32 automatically.

## Implemented qualification and bounds

Atomic Editor is pinned to `@atomic-editor/editor@0.6.2`. Required direct peers are explicitly pinned: autocomplete 6.20.4, language 6.13.1, search 6.7.2, Lezer common 1.5.5, highlight 1.2.5 and markdown 1.8.1, alongside existing CM state/view/commands/lang-markdown and React pins. The actual published MIT license is preserved in the third-party inventory. `npm ls` confirms one deduplicated CM state/view installation. The package's extensionless ESM requires a narrow Vitest inlining setting so tests use Vite resolution, as production does; no build tool or dependency version was replaced to match upstream tooling.

The viewer composes Atomic inline preview, reading mode and syntax styling with app widgets; upstream React ownership, image/table widgets and global CSS are omitted. Tables retain their Markdown representation rather than introducing a second image-loading or editing path. Reading links can be selected/copied; opening links is outside this viewer's actions. App transactions and direct view dispatches both refuse document changes, including `filter:false` dispatch. GFM checkbox widgets are disabled.

Photo transport is serialized globally. Each in-flight read reserves 32 MiB against the existing 64 MiB session payload budget, then retains its actual encoded byte size until eviction. Decode accounting is separate: at most 256 MiB across visible images, with 4 bytes/pixel for up to 8-bit PNGs and 8 bytes/pixel for 16-bit PNGs. Header bounds match the server (8192 per dimension, 32 million pixels); budget refusal gives a retryable fallback rather than evicting unrelated user text. These are payload/decode limits, not a process RSS guarantee. Widget IntersectionObserver demand uses a 200px margin; leaving that viewport, detach, disconnect, replacement and close release leases. Resource entry loads reserve 5 MiB temporarily; retained metadata and bodies remain under existing session/editor budgets. No unbounded photo cache exists.

Settings places Image engines in the existing AI Agents category. Source-age messages consume both local acknowledged-save revisions and authoritative bridge reads during load/resync, so they do not depend on a watcher already existing for a newly created Authentic tree. Admission retains its request identity across uncertain replies and handles lifecycle delivery arriving before its acknowledgement. The evidence record links tests and native/artifact results.

## Planning validation

R01 is supported by the owner answers recorded in the overview, decisions and changelog. R02 is supported by the inspected paths above. R03 is supported by the resolved Q1–Q6 and the staged acceptance mapping, including C20. Package qualification must record exact pins/licenses, and T02 must record concrete image budgets before the affected implementation. That paragraph records the plan-review stage. Subsequent implementation qualification and acceptance are in the [evidence record](milestone-35-illustrated-diary-gui-evidence.md).

Plan-review validation (2026-11-13): `scripts/gate.sh` G01–G07 PASS (`run.5exTWj`, loopback access enabled); final changed-document Prettier check, local Markdown link-target review and `git diff --check` PASS. The initial sandboxed gate (`run.3mfy6n`) failed G03 because bridge fixture listeners were denied loopback binding; that is not feature-failure evidence. These checks validate this documentation change, not M35 implementation completion.
