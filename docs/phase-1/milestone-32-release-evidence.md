# P1-M32 release execution evidence

Status: Active

Execution ledger for the [release checklist](milestone-32-personal-release-overview.md).
Required artifact results remain separate from automated application coverage and
exempt human observations. Dated execution records below distinguish passing coverage from open release
scenarios; the existence of a test is not a passing result.

## Coverage inventory

All rows below are required. The linked H01–H12 observations are separately exempt
by the owner's 2026-11-09 policy, and remain unverified except the owner-reported
Qwen run. Earlier live-provider results retain their original versions and limits.

| Release / automation / evidence | Observable contract and executable coverage                                                                                                                                                                                                                                                    | Layer / inputs                                      | Execution result                                                                   |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- | ---------------------------------------------------------------------------------- |
| R01 / A03–A05 / E1–E13          | This ledger maps every release item; outstanding artifact and GUI gaps remain explicit below                                                                                                                                                                                                   | Documentation audit                                 | Inventory implemented                                                              |
| R02 / A01 / E10–E11             | `release-artifacts.test.js`: missing files, contamination, versions, architecture, mismatched clients, notices and prompts; `check-fixture-exclusion.mjs` explicit paths checks actual artifacts                                                                                               | Node / ordinary server, client, app, notices        | Passed (dated results below)                                                       |
| R03 / A02 / E1,E5,E12           | `release.py`, `test_release.py`: copied absolute launch outside checkout, PID-bound readiness, bad/stale tokens, second lease, release, restart, sentinel survival, timeout/early-exit cleanup                                                                                                 | Python / ordinary server, disposable configuration  | Pending; native client shutdown is separate                                        |
| R04 / A03 / E2                  | `authentic.py`, Rust persistence/receipt tests, `saveQueue`, `authentic-ui`, `resources`, `navigation`: durable read-back, uncertainty, composition and transition guards                                                                                                                      | Rust, Vitest, synthetic server / disposable files   | Passed (dated results below)                                                       |
| R05 / A03 / E3–E4               | `imports.py`, `producer.py` and `producer.py --schema2`, `source-save`: source read-back, metadata/audio preservation and conflicts                                                                                                                                                            | Producer/server / disposable schema 1 and 2         | Passed (dated results below)                                                       |
| R06 / A03 / E2,E4               | `trash.py --enabled-server`, Rust receipts and `trash-coordinator`, `trash-ui`: restart/interruption and guarded recovery                                                                                                                                                                      | Native/server synthetic fixtures, Vitest            | Passed (dated results below)                                                       |
| R07 / A03,A05 / E5–E6,E11,E13   | `agent.py`, `review.py`, generation/chat/review controller and validation suites: eligible capture, cancellation, partial outcomes, stale baseline, serialized decisions, discard                                                                                                              | External fake CLI, server and application           | Passed (dated results below)                                                       |
| R08 / A04 / E6,E9,E11,E13       | `candidate-table`, `generation-navigation`, `paragraph-review-controller`, `paragraph-review-adapter`, `paragraph-review-context`, `review-editor`, `profiles-ui`: closed tabs, independent profiles, creation-time context, isolated chat, stale refusal; added real undo/autosave regression | Vitest/CodeMirror, Rust isolated conversation tests | Passed (dated results below)                                                       |
| R09 / A04 / E9,E12              | `shortcuts`, `command-palette`, `navigation-layout`, `accessibility`, `symbols`, `settings-ui`, `client`: composition, focus, bounds, persisted toggles and settings guards                                                                                                                    | Vitest plus native client tests                     | Passed automated coverage; jsdom is not native IME/layout proof                    |
| R10 / A05 / E1,E5,E13           | `activity-preview`, `activity-runtime`, `activity-store`, `activity-wake`, `saveQueue`, Rust supervisor/lease tests; `agent.py` explicit ordinary server: bounded preview/flood, offscreen silence, silent baseline, once-only new completion and no inference on reconnect                    | Controlled clocks, external fake CLI, server        | Pending; actual sleep/RSS/timing are H09                                           |
| R11 / A03 / E7                  | `export.py`, Rust iCloud/fault/reconciliation tests, `export`, `export-ui`: saved revision, destination guards, cancel, uncertain restart and staging                                                                                                                                          | Temp directory/fake destination                     | Passed automated coverage; no File Provider/upload claim                           |
| R12 / A03 / E8                  | Rust Notion fake-HTTP/restart tests and export UI; prior M27 live round trip retained                                                                                                                                                                                                          | Synthetic HTTP, no account usage                    | Passed automated coverage; runtime/provider contracts unchanged by harness changes |
| R13 / E10,E12                   | Notices/privacy tests, README/user guide independent Paragraph Review profile correction, contribution guide explicit lanes                                                                                                                                                                    | Documentation and offline policy checks             | Passed (dated results below)                                                       |
| R14 / A07 / E1–E13              | Final seven-lane gate, Python harness suite, production builds, explicit scans and affected smokes                                                                                                                                                                                             | Delegated runner                                    | Pending                                                                            |
| R15 / A06,A07 / E1–E13          | Final audit of required results and GUI feasibility; H01–H12 remain exempt rather than passed                                                                                                                                                                                                  | Evidence audit                                      | Pending                                                                            |

## Prior evidence impact

The changes include qualification tooling, regression tests and the approved
review correctness fixes. Provider arguments, prompts, dependencies and durable
schemas are unchanged. The earlier M17–M22 and M27 real-provider results still support their
recorded adapter contracts. They do not establish current packaged integration or
model quality. The Qwen human report is scoped in the manual guide. Local directory
and fake-HTTP tests establish no phone, File Provider, remote upload or TCC behavior.

The owner approved the five historical M24 fixes on 2026-11-10. Implementation
now guards EOF paragraph append separators (including accepting a later append
alone), reads disk outside the review lock, preserves typing during review
startup, rechecks old watcher reads and ends/reloads reviews after obsolete
lost-reply decisions. Rejection stays transient; no durable diary write is added.
The dated results below record the validated revision and subsequent review follow-up.

## Delegated results (2026-11-10)

These are the pre-hardening build results; the final results below supersede artifact identity after runtime fixes.

Runtime baseline: `7626a43a1d41f9b685adb2189df34f8f6ca7698c` plus the working-tree changes; qualification code
and documentation changes are uncommitted. macOS 26.7.1 (25G241), arm64,
Rust/Cargo 1.98.0, Node 26.8.1, npm 11.19.0. Node/npm are below the declared
setup baseline; successful checks on this host do not widen supported versions.

- All seven `scripts/gate.sh` lanes passed, as did `npm run build` and the seven Python harness tests (process permission required).
- Ordinary release server and Tauri `.app` builds passed. Explicit artifact scan passed: version 1.0.0, arm64, matching standalone/bundled client, notices/licenses and fixture-marker exclusion.
- Server SHA-256: `bdd3ac4db9e3ffd19538a64e13717f0aa1584687056b7b6b26a22e49f4d6d33a`.
- Bundled/standalone client SHA-256: `6fb53236202a1747b5678e767eb6e8c795fc7a1a93658bc0e82cee4f37998bc2`.
- Artifact paths: `src-tauri/target/release/diary-server`, `src-tauri/target/release/diary-client`, `src-tauri/target/release/bundle/macos/Narihito's Diary.app`. No fixture build is a deliverable.
- `release.py --server <ordinary-release-server>` passed copied-install launch, authentication, empty initial jobs, lease exclusion/release, orderly restart, crash/stale discovery and cleanup/sentinel survival. The HTTP lease client does not prove native client cleanup.
- `agent.py --server <ordinary-release-server> --fake-cli <external-test-cli>` passed. This is synthetic compatibility and embedded prompt/helper availability evidence, without provider usage. Apple helper unavailable: Swift/macOS 26 SDK unavailable to the build. This does not prove live Apple integration.
- Debug/fault synthetic `authentic.py`, `imports.py`, `review.py` and `export.py` passed. Producer schema 1 passed 12 fixtures/3 accepted mutants; schema 2 passed 15 fixtures/3 accepted mutants.
- Trash initially failed because the harness omitted existing producer source files from its Swift compilation. The source list is corrected and the subsequent rerun passed (see below). No producer source was modified.
- CUA inventory confirms a native macOS GUI surface is available. Packaged GUI actions remain pending; tooling cannot be described as unavailable based on this inventory.

These pre-hardening results do not close R15: runtime fixes require fresh
regressions and rebuilt artifact qualification. The helper negative
outcome is an explicit integration limitation, not a successful Apple run.

Focused artifact/editor regressions: 11 passed, including real undo and bundled
client contamination/missing-license negatives. Changed supported files pass
Prettier; `git diff --check` is clean. Python is validated by its interpreter,
not Prettier.

## Review-hardening and GUI follow-up

Trash qualification now passes after adding the pinned producer's performance,
read preparation, resource-presence and purge dependencies to the harness source
list. Native interruption/recovery, ordinary HTTP preview/confirm, snapshots,
restart, duplicates and producer catalog/reopen passed; the generated evidence is
`src-tauri/target/trash-harness-evidence.json`.

A06 feasibility: an isolated release server (PID 61411) and bundled client
(PID 61412) launched for native inspection. CUA `getApp` against the release bundle
failed with computer-use service error `-10005: timeoutReached` after 989 seconds.
No native actions/screenshots/focus claims follow. The launcher has a 300-second
client deadline and finally cleanup; the delegated runner confirmed both fixture processes were cleaned up.
Further GUI observations remain unverified and link to exempt H07–H09.

## Final post-hardening results (2026-11-10)

The delegated runner previously reported 9 domain review tests and 9 application review tests;
the latter count is corrected below (the application review module has 7 tests). It also reported
18 focused controller/editor/lifetime tests. All seven final gate lanes pass.
The ordinary release server/client and app were rebuilt; the explicit artifact
scan, copied-install lifecycle smoke, synthetic review smoke, ordinary-release
external-fake-CLI AI smoke and corrected enabled-server Trash smoke all pass.
Earlier unrelated durable/source/export results remain applicable: the runtime
fixes touch review boundaries, not their persistence or adapter implementations.

Final artifact identities (supersede the pre-hardening hashes above):

- Server: `85f532bdb908977871110e95195392c79e96df50725a39733c29d121b7aa21e8`.
- Standalone/bundled client: `b07652371d9fdf432829f6a4c1a46ba407f1c96d871dd1616ddb6500b6e30fc3`.

A03/A04 coverage audit is satisfied by the listed passing regressions and smokes;
no duplicated integration harness was added. A06 feasibility is complete with the
observed CUA service failure; native GUI behavior remains unverified, rather than
passed. H01–H12 retain their non-gating policy. The Apple helper remains unavailable.

Remaining release audit scope: A02/R03's complete copied-app/client lifecycle
proof is not established by the HTTP lease client or timed-out CUA observation.
A05/R10's ordinary-release maximum valid streaming plus concurrent acknowledged
save/control scenario is not established by separate deterministic unit suites
and the ordinary AI compatibility smoke. Those required artifact scenarios remain
open; this execution does not mark M32 or Phase 1 Done. No human test is being
required to substitute for those automated scenarios.

## Implementation review follow-up

Review found and corrected these defects:

- Process cleanup skipped an owned process group after its leader had exited, leaving descendants alive. Cleanup now signals the owned group and bounds escalation even for an exited leader.
- Readiness assumed discovery JSON was an object containing `pid`; malformed records could raise instead of waiting for the owned server. Records now require the launched PID, loopback origin and nonempty credentials/identity.
- Artifact checks only required a nonempty license directory. Every notice reference in both packaged inventories now requires the exact SHA-256 content, with missing/corrupted-license negatives.
- Candidate review ignored a refused editor update after a confirmed server decision. It now ends the obsolete view and reloads saved state without repeating that mutation.

Added Unicode/CRLF selective-append coverage alongside the existing separator
regressions. Follow-up validation passed 10 artifact tests, 9 Python harness tests,
10 domain review tests, 7 application review tests and 19 focused frontend
review/editor/lifetime tests. The latter includes refused presentation after an
acknowledged decision. No paragraph-boundary defect was found in the added
Unicode/CRLF subset cases.

`npm run build`, the ordinary Tauri app rebuild, the explicit artifact scan and
copied-install smoke all pass. The scan verifies both notice inventories and each
referenced license digest. The rebuilt server is unchanged:
`85f532bdb908977871110e95195392c79e96df50725a39733c29d121b7aa21e8`.
The standalone/bundled client now hashes to
`767f0f132a8d4648e23e063039c2e34b6681ae422c07706c981bced506916fdb`,
superseding the prior client identity for this frontend fix.

G01/G02/G04–G07 pass. G03 initially failed in the synthetic Apple test
`apple_generation_output_is_validated` with `Unavailable`; the test uses a shell
stand-in and this failure is not evidence of a missing native SDK. A direct
`agent_apple` rerun passed 19/19. The complete
`cargo test --manifest-path src-tauri/Cargo.toml --workspace` rerun then passed
with loopback permissions, including all 90 API, 94 client, 37 domain, 326
infrastructure and 56 logging tests plus other integration/doc suites. Thus all
seven gate lanes now pass for this implementation; the initial transient failure
is retained here rather than attributed to the SDK. No test was newly skipped or
timeout relaxed; existing live-provider tests remain intentionally ignored. Native GUI and maximum-streaming release
scenarios remain open as documented above.

## Packaged native GUI run (2026-11-10)

CUA access by bundle identifier succeeded on the ordinary release app, superseding
the earlier timeout for the observations below. The delegated runner launched
only a private temporary workspace/configuration with no sources or AI profiles.
Client PID 99858 and independent server PID 99849 identify this run; no private
configuration or diary data was used. Native UI actions used CUA accessibility
controls and keyboard input, with an app screenshot for visual inspection.

Passed observed GUI checks:

- Create Empty changed to an explained disabled state after creation.
- Entered synthetic two-paragraph text; the shared Entry view retained it and Global Messages reported Saved. Unicode Korean paste appeared intact in the editor and screenshot.
- Settings placed Server first and Status Line immediately before Keyboard Shortcuts. All three Status Line toggles remained off after closing/reopening Settings in the same app session; this did not verify persistence across an app restart.
- Trash Recovery showed its empty/recovery guidance, focused Close on open and returned focus to its toolbar button after Escape.
- AI Activities showed the empty state and closed on Escape.
- Command palette filtered `Settings` to Open Settings; Enter executed it and focused the Settings Go back button.
- Explorer splitter accepted keyboard Right, increasing its accessible size from 220 to 230. The 1200×800 app screenshot (2400×1600 capture) showed the three-pane layout, toolbar and synthetic text intact.
- Native Command+W closed the app; the subsequent CUA app inventory no longer listed Diary as running. Process exit, durable content and preference verification follow below.

Evidence limits and findings:

- Direct CUA `typeText` did not inject the Korean characters. Clipboard paste preserved them. This is an automation input limitation/observation, not proof of a native Korean IME defect or a passed composition test.
- AI Activities focus return is inconclusive: the first Escape snapshot reported focus on the HTML document rather than the opener; a repeated attempt did not report a focused element. Keep this as a focus finding for further investigation, not a passed focus-return check.
- No source or AI profile was configured, so candidate Info/Open, live generation/chat, paragraph suggestion Accept/Reject and provider exports were not exercised.
- Minimum-window geometry, narrow candidate tables, actual IME composition, native undo, live File Provider behavior and AI load remain unverified. This run used the release app in its build output, not a complete copied installation; A02/R03's copied-app qualification remains open.

Delegated verification: client exited `0` and the independent server was still
alive at client shutdown. After the launcher completed, a process listing found
no matching temporary server/client PIDs; the independent server was therefore
reaped by the fixture cleanup. The on-disk Authentic body matched exactly
`GUI test: 오늘은 차를 마셨다.\n\nSecond paragraph for save verification.`
Fixture cleanup removed discovery. The client hash is
`767f0f132a8d4648e23e063039c2e34b6681ae422c07706c981bced506916fdb`.
