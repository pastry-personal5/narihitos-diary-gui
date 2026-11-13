# P1-M35: Illustrated diary GUI evidence

Status: Archived

Evidence for the [overview](milestone-35-illustrated-diary-gui-overview.md) and [work plan](milestone-35-illustrated-diary-gui-architecture.md). Results below distinguish application/DOM tests, the packaged native fixture and ordinary release artifacts. No real AI account, user workspace or image engine was used.

## Environment and dependency qualification

Observed on 2026-11-13: Apple Silicon arm64, macOS 27.2.1 (26B131), Node 26.8.1 and npm 11.19.0. Builds succeeded on this environment; this is not evidence for other OS/toolchain versions. The working tree contains the M35 implementation and preceding owner-approved plan edits, without a new commit.

The actual published `@atomic-editor/editor@0.6.2` tarball was inspected before installation with scripts disabled. SHA-1: `ee7cb2e5db45de700189c4118e665a2165b7d27b`; registry integrity: `sha512-b4cZRduNOoZ5sJ7ywaANcnLLKP3EJYfMUm5BgYK3Wb+9jI+KUs04RZHzFMVcdedNjvfqBDbzW6KYAmq5bAayxw==`. Its MIT license, including Kenny Bergquist's 2026 copyright, is preserved byte-for-byte in the inventory and `assets/third-party/licenses/e1ce7da88958b6876939f6e6fd9c970b51c44ef3fcdb317ee9679eef78b6c58f.txt`. The [published manifest](https://registry.npmjs.org/@atomic-editor/editor/0.6.2) and [upstream exports](https://github.com/kenforthewin/atomic-editor/blob/b6ed65f01bde4510031bc7a495520bc1a7688c66/src/index.ts) identify the qualified extension API.

Required peers are exact direct dependencies; `npm ls` reports one deduplicated CM state 6.7.6/view 6.43.15 installation. No optional fenced-language peer was added. The notice collector added exactly two resolved npm records (Atomic Editor and CM search), preserving all existing records and notice bytes. Existing tooling was retained; a narrow Vitest dependency-inlining setting handles the package's bundler-style extensionless ESM. The production Vite build and actual-package DOM/native tests exercise that path.

Upstream reading mode intentionally permits checkbox dispatch, so the app disables checkbox controls and rejects document changes at both transaction and direct-view-dispatch boundaries. Upstream image/table widgets and React ownership are omitted; app widgets own authenticated photo transport and disposal. Tables retain Markdown representation. The work plan records exact memory limits and source/version scope ownership.

## Application and DOM acceptance

The final gate passed 500 frontend assertions across 68 files, including the keyboard-focus/find and late-cancellation regressions. The following suites and native observations support the completion checklist.

| Checklist | Evidence and observable assertion                                                                                                                                                                                                                                         |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C01       | `illustrated-validation`, `adapter`, native `bridge::illustrated`, `bridge::images` and native smoke: bounded/typed entry, ordered manifest, date, engine identity, binary and acknowledgement validation; malformed replies become `invalid_response`.                   |
| C02       | `illustrated-controller` and native Explorer leg: independent catalog, adjacent tab identity, orphan Illustrated access, create/replace refresh and disappearance after confirmed deletion.                                                                               |
| C03       | Controller/action guards plus shared registry tests: saved source, selected engine rather than another ready engine, Illustrated profile capability, locale, storage, admission and connection state. The native action starts disabled for an unsaved date.              |
| C04       | Real Authentic save-queue regression keeps buffers editable and acknowledges subsequent saves while an Illustrated photo job is running; activity UI renders text/photo/stopping progress and Cancel. Synthetic server leg cancels the fake agent and renderer.           |
| C05–C06   | Controller tests and synthetic/native creation legs: success replaces only after publication; failed/render-cancelled/agent-cancelled jobs retain the prior version and produce messages.                                                                                 |
| C07       | Actual Atomic DOM tests reject checkbox, paste, drop, undo, ordinary dispatch and `filter:false` dispatch; body stays unchanged. App/context tests disable save/export and use Authentic chat context.                                                                    |
| C08       | Observer-driven DOM tests and native decoded PNGs: ordered alt text, lazy bridge demand, URL release on eviction, replacement and close; stale completions allocate no URL.                                                                                               |
| C09       | Controller stale-confirmation tests and native Cancel/confirm sequence; server read returns 404 and both Markdown/photo directories are absent after deletion. Authentic bytes survive.                                                                                   |
| C10       | Native bridge/engine tests and `illustrated-ui`: effective selection, readiness/reason, Check and non-rendering probe behavior; native Settings checks the synthetic engine.                                                                                              |
| C11       | Viewer keyboard-focus/find test, existing accessibility/shortcuts/dialog suites, native Cancel initial focus, semantic buttons and inherited focus/reduced-motion CSS. Canonical terms updated. No new animation is introduced.                                           |
| C12       | README, user guide, product behavior, UX/IA/terms, API/image contracts, decisions, phase/changelog, roadmap, index and smoke instructions updated together.                                                                                                               |
| C13       | Packaged fixture and ordinary synthetic server run below, with two generated PNGs.                                                                                                                                                                                        |
| C14       | Final seven-lane gate below.                                                                                                                                                                                                                                              |
| C15       | Deferred real save acknowledgement precedes admission; save failure sends no create request; date/profile/locale snapshot is fixed; uncertain replies reuse the request ID; delivery-before-ack and disconnect races do not duplicate admission.                          |
| C16       | Native progress and frontend page/snapshot tests accept Illustrated alongside existing jobs, bind instance/session, reject invalid photo phases/counts/codes and retain content-free progress. Shared activity runtime owns reconciliation/polling.                       |
| C17       | Photo tests serialize transport, enforce pixel/retained-byte budgets, release on abort and ignore late arrivals; packaged CSP allows decoded blob photos and refuses remote image loads.                                                                                  |
| C18       | Confirmed delete is bound to the displayed UUID; stale deletion reloads without deleting a replacement. Obsolete reads after disconnect do not replace retained content. Server synthetic leg rejects old photo versions and preserves publication across failure/cancel. |
| C19       | `illustrated-ui`, action and existing resource/editor tests: Authentic context fallback, no Illustrated save/export/review/adoption, ordinary Generate/Chat guards and Illustrated activity/status presentation.                                                          |
| C20       | Controller tests and native Global Messages: once-per-version/source notices, completion on the originating date, background completion without navigation/focus theft, and acknowledged-save observation without relying on filesystem watcher startup.                  |

## Packaged native and synthetic server run

Command: `python3 scripts/smoke/illustrated.py --gui-app "src-tauri/target/debug/bundle/macos/Narihito's Diary.app"`, after a `scaffold-smoke` packaged build. Result: **PASS illustrated packaged GUI** and **PASS illustrated synthetic**. Retained evidence directory: `diary-illustrated-6bpehkfo` under the OS temporary directory.

The ordinary synthetic-server leg covers create, whole-job/agent/render cancellation, render failure preservation, successful replacement, stale version photo refusal and confirmed deletion. The native WKWebView leg covers Settings probe/readiness, create, real PNG decoding/alt text, source-change bridge resync and Global Message, replacement, background navigation/completion, same-lease reconnect, close/reopen, failed-image Retry, image CSP refusal and Cancel/confirmed delete. Disk assertions establish no Illustrated Markdown or PNGs remain. Native key input/IME and real GPU/model quality are not inferred from programmatic fixture actions.

Observed result: 2 photos/2 theme groups; 11 object URLs created and 11 revoked; packaged origin confirmed; remote image CSP refusal confirmed. Synthetic API sequence preserves its Authentic bytes exactly; the separate native source-change scenario deliberately updates only its isolated Authentic entry and verifies it survives deletion. Recorded sequence duration: 22.22 seconds (not a performance qualification).

| Artifact in that run            | SHA-256                                                            |
| ------------------------------- | ------------------------------------------------------------------ |
| Ordinary debug synthetic server | `14f38a53433c67cd9aebf6e1217e38b1da7da07d8b5d1bd766dcd4d07ac7105d` |
| Packaged fixture client         | `9539a1a71a43aacb58ccb61f112d8519bfa92503d322b238fa12b15d871a5ca1` |

Earlier native attempts were failed qualification runs, not passing evidence. They exposed a fixture-only optional Settings field mismatch and setup races (initial date selection, close acknowledgement and mounted-widget reads). A deprecated synthetic `execCommand` edit was not accepted as native keyboard evidence; the final source-change leg uses an authenticated save and app focus/resync. The final harness polls state, fails on its assertion record and cleans up only its own processes/settings/workspace. The ordinary release scanner now also rejects the new fixture environment/diagnostic markers.

## Ordinary release and M32 impact

`make release` passed, rebuilding the ordinary release server and `.app` after the fixture build. Explicit artifact verification passed: version 1.0.0, arm64, byte-identical standalone/bundled client, bundled notice/license inventory, prompt resources and absence of fixture markers. The full verification manifest is retained as `diary-illustrated-release-aDoYWH/artifacts.json` under the OS temporary directory. The final layout-only rebuild preserves the exact already-tested release-server bytes; the final standalone/bundled client hashes below match each other. The Apple helper is unavailable in this build (`helper_missing`); no live Apple integration is claimed.

| Ordinary release artifact             | SHA-256                                                            |
| ------------------------------------- | ------------------------------------------------------------------ |
| `diary-server`                        | `95783de7adb3fb4855c65f51eaebd02d05658c10fd4f25dd91dfd26e819487d6` |
| Standalone and bundled `diary-client` | `b0a362324860c5d116d70f155e13cf9a249867284f586cf22f7f3d908220b59d` |
| Bundled notice inventory              | `e140341aca2d39211d65e90cb8763fb3e6d5dbbecf8129dc0505ff923b053e4e` |

`release.py --server <absolute ordinary release server>` passed using copied artifacts in private paths with spaces: HTTP session/process/lease checks, independent server/sentinel survival and owned cleanup. It proves no native client action. The affected `agent.py --server <absolute ordinary release server> --fake-cli <absolute external fake CLI>` smoke also passed in explicit-artifact mode with no rebuild or real provider. M32 remains Active; this feature does not certify unrelated release observations or live integrations.

## Final validation

- Python harness suite: **15 tests PASS**, with process-inspection access. The first sandboxed attempt failed seven cleanup assertions because `ps` was denied; it is not counted as passing evidence.
- Frontend build/typecheck: PASS, with the existing bundle-size advisory recorded as a build advisory, not suppressed.
- Affected release-agent smoke: PASS (`agent smoke: ok`, explicit artifacts, no rebuild).
- Final `scripts/gate.sh`: **G01–G07 PASS**, `run.9Sko0I` (2026-11-13), with loopback access. G06 passed **500 tests across 68 files**. The prior implementation gate (`run.UuBoOo`) also passed before the final Explorer heading cleanup.
- Local Markdown links, final changed-document Prettier and `git diff --check`: PASS after archival/status updates.

Post-fixture cleanup: `make exclusion` PASS. The ordinary debug app/client and server were rebuilt after native qualification; the default debug scanner confirms fixture markers are absent. Ordinary release artifacts remain byte-identical to the recorded final release manifest.
