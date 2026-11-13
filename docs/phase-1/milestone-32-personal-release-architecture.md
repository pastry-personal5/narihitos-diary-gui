# P1-M32: Personal release qualification work plan

Status: Active

Execute the [release checklist](milestone-32-personal-release-overview.md) against the final ordinary packaged app and standalone server. Reuse existing verification tools and application boundaries; fix demonstrated defects with code, regression coverage and docs together. Planning does not certify a capability or authorize new dependencies.

## Acceptance boundary

Apply the [owner's updated acceptance policy](milestone-32-personal-release-overview.md#acceptance-policy-owner-update-2026-11-09):
by-human tests are documented but exempt from M32/Phase 1 exit gates. Sections
3–5 describe scenario coverage; mandatory automated portions map to R/A items,
while direct human/native/live observations map to the [H01–H12 guide](milestone-32-by-human-tests.md).
Do not reintroduce a human gate through a prerequisite, linked feature checklist,
measurement requirement or final audit. Observed defects still require triage.

## 1. Freeze scope and evidence inventory

Create a scenario ledger keyed by R01–R15 and the [handoff evidence register](implementation-handoff.md#capability-evidence-register). Review E1–E13 and the three linked M32 feature plans; link earlier passed evidence rather than copying claims. Classify each remaining case by execution method and separately mark it required automated/artifact coverage or exempt by-human observation. Link every manual case to H01–H12 and the owner policy; a native automated check can remain required when feasible. Audit unresolved review findings against current code and tests before deciding whether a fix is needed.

Record the current commit and any uncommitted changes, macOS/hardware, toolchain, supported CLI versions and configured capabilities without exposing paths, credentials or diary text. The architecture's macOS 26.7.1 target is not proof of the actual machine's OS or support for other versions. No new implementation starts until the applicable checklist exists.

### Execution order and evidence rules

| Stage               | Work and prerequisite                                                                 | Exit evidence                                                                                           |
| ------------------- | ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| 1. Inventory        | R01; inspect existing tests and live/GUI prerequisites before changing code           | Scenario ledger, exact gaps and required fixture inputs; existing waivers identified                    |
| 2. Automate         | A01–A05/A07; A06 feasibility; implement missing coverage and fix demonstrated defects | Focused checks pass; each automation task has test/coverage/feasibility evidence; no new stack required |
| 3. Freeze artifacts | R02, after relevant fixes and offline checks                                          | Ordinary release build and copied deliverable manifest; no fixture feature or checkout dependency       |
| 4. Qualify          | A02/A06 final runs and R03–R12 against the frozen artifacts                           | Required automated process/native results and dispositions for exempt human/live observations           |
| 5. Close            | R13–R15 after required automated/artifact cases pass or have an explicit owner waiver | Final gate, evidence audit, user docs, linked checklists and phase status agree                         |

Automation harness work can proceed before a release build; its final artifact
run depends on stage 3. Missing GUI permissions, phone access or a disposable
live destination leave dependent H observations unverified and exempt; continue
required independent work. Record these prerequisites early; do not defer their discovery until the
final gate. The owner exemption covers human execution; unavailable required non-GUI automation remains blocked. A06 permits an evidenced feasibility disposition.

Each scenario ledger row must name its R/A/E IDs, observable outcome, exact test
or procedure, execution layer, required inputs, result and evidence reference.
Use `planned`, `passed`, `failed`, `blocked` or `exempt`; a prerequisite skip is
`blocked` for required release coverage. Exempt needs a specific owner decision.
Track `gate: required|exempt-by-human` separately from observed result, so a manual case can remain `planned` or `blocked` without blocking release. A completed owner-reported case must identify that evidence source. An automation feasibility disposition is separate from an observed behavior result.

| Evidence obligation               | Release checklist coverage                                                      |
| --------------------------------- | ------------------------------------------------------------------------------- |
| E1 bridge/discovery/native access | R02/R03/R09                                                                     |
| E2 durable persistence/recovery   | R04/R06/R14                                                                     |
| E3 source reading/date coverage   | R05/R09                                                                         |
| E4 producer writes/Trash          | R05/R06; preserve M15 automated-only acceptance                                 |
| E5 process/real adapters          | R03/R07/R10; owner-reported Qwen manual completion in H05/H06                   |
| E6 candidate paragraph acceptance | R07/R14; separate from caret AI suggestions in R08                              |
| E7 iCloud                         | R11 automated coverage; H10 live File Provider observations exempt              |
| E8 Notion                         | R12; prior live evidence needs a change-impact assessment                       |
| E9 native UX/symbols              | R08/R09/R10; VoiceOver exemption preserved                                      |
| E10 dependency/toolchain/support  | R02/R13/R14                                                                     |
| E11 Apple/prompts                 | R02/R07/R08; helper fallback and actual live integration have separate outcomes |
| E12 logs/settings/shortcuts       | R03/R09/R13                                                                     |
| E13 background AI                 | R07/R08/R10                                                                     |

Previous evidence may be reused with an explicit explanation of why changes
since its revision do not invalidate the observed contract. Runtime, asset or
packaging changes require affected required checks and artifact qualification to rerun;
record the impact on prior manual evidence without demanding another human run.
Documentation-only finalization needs documentation checks, not another native
workflow run. Hash the final copied artifacts and preserve the tested runtime
revision plus any final documentation-only revision; do not claim one hash covers
a later rebuilt binary. A helper or provider's unavailable-state test passes that
negative case only; it cannot establish successful integration. Missing human-only integration proof is an explicit non-gating limitation.

## 2. Build and identify the actual deliverable

### Automated test matrix

Implement A01–A07 during M32 using the current Vitest, Rust, Node and Python
tooling. Inspect existing assertions before adding cases; a scenario already
covered needs a final-revision rerun and evidence link, not another equivalent
test. Keep milestone IDs in docs only. Test behavior and observable artifacts,
not source spelling or snapshots that merely mirror the implementation.

| Task                         | Placement and approach                                                                                                                                                                                                                                                                           | Observable assertions and limits                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A01: release artifacts       | Extend `scripts/check-fixture-exclusion.mjs` to accept explicit artifact paths while preserving its documented debug use; add release resource/metadata checks with existing Node/Python tools. Extend `tests/version-policy.test.js` / `tests/notices.test.js` only for uncovered policy cases. | Scan the supplied server, standalone client and bundled client, verify matching versions/arm64 identity and resources/notices by their actual owner; distinguish server helper presence from the approved unavailable-helper outcome without treating fallback as successful Apple integration. Deliberately contaminated/missing artifacts and version drift must fail. Record hashes of inspected files. Marker absence alone does not prove production endpoint exclusion: exercise forbidden routes/commands in the process/bridge tests too. |
| A02: process lifecycle       | Refactor reusable launch/config/readiness/cleanup helpers in `scripts/smoke/` so an ordinary-artifact mode takes explicit paths and never rebuilds or silently substitutes debug binaries. Add Python harness tests in `scripts/tests/` for its failure/cleanup behavior.                        | Launch from outside the checkout with spaces in paths, authenticate only through that server's discovery, reject stale/second-client attempts, restart and close within deadlines. Kill only owned processes. Verify independent server/sentinel survival; failed startup and timeouts must leave no owned descendants. Actual GUI close needs a GUI session and is a separate lane.                                                                                                                                                              |
| A03: durable workflows       | Reuse `authentic.py`, `imports.py`, `producer.py --schema2`, `trash.py --enabled-server`, `review.py`, `export.py` and Rust fake-destination/HTTP tests. Add only ledger-identified missing recovery sequences.                                                                                  | Assert saved bytes/revisions and producer metadata/audio after reopen; lost replies and original-ID retries do not repeat mutation; stale tokens and changed destinations refuse unsafe writes; partial/uncertain states reconcile without guessing. Fault-only scenarios stay separate from ordinary release smoke evidence. No live iCloud/phone/Notion behavior is inferred from fakes.                                                                                                                                                        |
| A04: cross-feature UI        | Extend existing `candidate-table`, `generation-navigation`, `paragraph-review-*`, `review-editor`, `profiles-ui`, `settings-ui` and `client` suites where combined transitions lack coverage.                                                                                                    | Closed tabs stay closed during background updates; composition-blocked actions have no side effects; stale suggestions cannot apply; one accepted editor transaction is undoable and enters autosave; user chat/profile ownership survives paragraph cancellation; visibility settings persist without hiding operational failures. Use application/editor ports and real CodeMirror tests as appropriate. jsdom cannot prove pixel layout or native IME.                                                                                         |
| A05: load and reconciliation | Extend `activity-preview`, `activity-runtime`, `activity-wake`, `activity-store`, save-queue and server/bridge tests with controlled clocks/adapters and bounded event streams.                                                                                                                  | Slow/no-output/flood runs leave save and cancel paths available; enforce ten visible publications/second, no offscreen text notifications and payload bounds; duplicate/gapped events, frontend reload, server replacement and lease expiry preserve correct result ownership and zero implicit inference; baseline history stays silent while a new completion after temporary disconnect announces once. Assert counts/state and acknowledged saved content; do not make host-speed assertions from fake-clock tests.                           |
| A06: packaged GUI            | Use available macOS accessibility/screenshot automation against the ordinary `.app` and isolated workspace; reuse the existing GUI smoke's permission/session conventions. Avoid adding a browser/automation dependency without approval.                                                        | Automate native click/key/focus/resize and screenshot capture where the runner can observe them, including minimum window/narrow Explorer, dialogs and suggestion actions. Check geometry/focus/state rather than brittle image hashes. Synthetic DOM composition events do not satisfy Korean IME qualification; actual sleep, provider states and Retina appearance belong in exempt H observations when not controllable.                                                                                                                      |
| A07: reproducible lanes      | Document exact invocations and prerequisites alongside the existing gate/smokes in the contribution guide. Add harness-level tests for missing paths, failed readiness, timeouts and cleanup as new helpers are introduced.                                                                      | Offline tests stay in the existing gate where supported. Artifact/process and GUI smokes run as explicit additional commands, each recording artifact/build mode. Missing release artifacts fail a requested artifact run; unavailable GUI/live prerequisites report unverified with their gate classification, never passed; A06 feasibility can close without a human run. No default test consumes account usage or writes to a service.                                                                                                       |

Run offline coverage first, then ordinary-artifact/process checks, then available
packaged GUI automation. Document remaining direct native/live observations
in the exempt manual guide. A tool or permission gap is an explicit evidence gap, not a reason
to weaken an assertion or install a new testing stack silently. Automating a GUI
case can satisfy that case when it observes the required native behavior; manual
repetition is optional, including when an observation remains unverified.

Python harness regressions are not currently included in `scripts/gate.sh`.
Run them explicitly with the existing interpreter:

```sh
python3 -m unittest discover -s scripts/tests -p 'test_*.py'
```

Keep that result alongside the gate result. Document the implemented artifact
and GUI command interfaces when A01/A02/A06 exist; do not invent executable
commands in the plan or count an unimplemented harness as automated coverage.

### Build procedure

Use the existing commands/toolchain, with no dependency changes:

```sh
npm run check:versions
npm run build
cargo build --release --manifest-path src-tauri/Cargo.toml -p diary-server
npm run tauri build -- --bundles app
```

Verify the resulting release paths from build output; do not substitute `make bundle`, which builds a debug app. Respect any configured Cargo target directory. Copy the server, `.app` and standalone-server notices into a fresh installation directory outside the checkout. Record hashes for the server, standalone/bundled client and bundle resources, and confirm native arm64 output and compatible app/server metadata.

`make release` wraps the standalone server and packaged release app builds;
`make release-server` and `make package` run each part separately. Tauri's
configured before-build command runs the frontend version/type/build checks.

The actual asset owners are important: infrastructure `build.rs` embeds the
Swift helper in the **server**, which extracts and verifies it in its private
runtime directory. Generation/chat prompts are server-embedded; paragraph-review
prompts are imported by the client adapter and compiled into frontend assets.
There need not be loose prompt or helper files under `.app/Contents/Resources`.
Check expected resources by their real packaging route and verify request
payloads through an external fake CLI, not only phrase matching in a binary.
Record the extracted helper's digest when present and probe availability through
the copied server. An empty embedded helper may correctly produce
`helper_missing`, but helper fallback does not establish live Apple success. Link prior live
evidence or record that limitation; further by-human runs are exempt.

Build without `scaffold-smoke` or `fault-injection`. Run
`scripts/check-fixture-exclusion.mjs` with explicit `--server`, `--client`,
`--app` and `--notices` paths for release verification, as documented in the
[contribution guide](../contribution-guide.md#release-automation-lanes).
Without artifact options it scans debug paths; `--target-dir` or
`CARGO_TARGET_DIR` selects their output root. A debug scan is not release proof.
Keep debug/fault-injection artifacts separate from the deliverable and rebuild
ordinary artifacts after any shared-output fixture build.

## 3. Isolated workflow and recovery qualification

Run with a new private workspace/runtime/config and disposable producer fixtures. Invoke the absolute standalone server path with `--config`, then the absolute `.app/Contents/MacOS/diary-client` with `--connect` to the validated discovery path, from a directory outside the checkout. Do not use Vite, `run-gui.sh` or a development-origin override to establish release launch. Document shell quoting for the app name and paths containing spaces. Verify client shutdown/lease cleanup independently of the still-running server.

| Area                    | Scenario and observable result                                                                                                                                                                            | Implementation owners / reusable evidence                                                                                                   |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Connection and platform | Correct discovery succeeds; wrong/stale discovery, auth/origin/Host and second-client attempts fail safely; folder access uses the actual server/client identities                                        | Server admission/discovery, Tauri permissions; `scripts/smoke/server.py` and `isolation.py` provide synthetic/debug evidence                |
| Authentic               | Empty create persists; shared Draft/Entry edits survive acknowledged saves/restart; IME delays unsafe flushes; external/failed/uncertain writes preserve buffers and expose recovery                      | Application editor/resource/source ports, CodeMirror, persistence adapters; `authentic.py`                                                  |
| Sources                 | Captured-zone D-1/D/D+1 import, conflicts, re-import and dirty-source guards; schema 1/2 producer reopen with metadata/audio intact                                                                       | Source/import policy and producer adapters; `imports.py`, `producer.py`, `producer.py --schema2`                                            |
| Trash                   | Preview exact ownership, confirm once, surface partial/uncertain outcomes and retry only remaining members; unsupported ownership stays disabled                                                          | Source/Trash application owners and native adapter; `trash.py --enabled-server`                                                             |
| AI and review           | Chat/generation use captured context and correct independent profiles; candidate table, transient results and durable candidate accepts obey guards; caret suggestion is undoable and preserves user chat | Generation/chat workflows, activity store, paragraph controller/AI/editor ports; `agent.py`, `review.py`, existing env-gated live harnesses |
| Exports                 | Flush/capture exact saved revision, preserve destination guards and one-use confirmations, show truthful cancel/uncertain/restart outcomes                                                                | Export application service and iCloud/Notion adapters; `export.py`, fake Notion tests and prior live proof                                  |

These script names are reuse points, not a claim they all exercise release artifacts. The implementation owner inspects and adapts their arguments/build behavior; the delegated runner executes checks and reports which binaries each actually runs. `server.py` currently assumes debug paths/default ports; several other smokes build their own binaries. The ordinary-artifact mode must require explicit paths, use isolated ports and readiness tied to the launched PID/instance, refuse hidden rebuilds, and never borrow another server's discovery. Reuse the ordinary development-close smoke (`gui-close.py`) for its launch-owned process contract; also test ordinary packaged close separately. Fault injection stays in disposable synthetic workspaces. Never simulate failure by damaging private diary data.

Include startup on empty and previously initialized synthetic workspaces,
server/workspace lock exclusion, compatible restart and safe rejection of newer
storage/API versions. Exercise migration/recovery in disposable copied fixtures
with the existing supported backup/exclusive-operation policy; do not invent a
user-data migration or downgrade path. Denied filesystem permissions are not
TCC proof: label POSIX denial and real privacy/provider denial separately.

## 4. Native presentation and background AI measurements

A05 deterministic assertions and feasible A06 automation remain required. Direct
human measurements and presentation observations below are optional H07–H09;
missing measurements do not block acceptance.

For feasible A06 automation or an optional H07/H08 UI pass, use the ordinary packaged WKWebView. Cover minimum supported window dimensions, a narrow Explorer, resized AI Chat, candidate Info/Open controls, closed-tab retention, paragraph suggestion placement, Accept/Reject/undo, modal focus, keyboard shortcuts/palette, Korean composition, Status Line ready/blocked transitions and saved settings. Verify actual Retina bitmap/SF Symbol rendering and text fallback. Compare current behavior with the maintained UX contract, including its newer toolbar/dialog placement.

Drive ordinary-release AI scenarios through an explicitly configured external
synthetic CLI, following `scripts/smoke/agent.py` and the existing
`diary-infrastructure` `fake_cli` example. The production profile's existing
`executable` override supplies the test process; the server and `.app` retain
their production admission, version, transport and output-validation guards.
Build the fake executable separately, never ship it, and use an isolated profile
home without real credentials or endpoints. Extend the fake's protocol-valid
slow/stream/partial/cancel cases as needed. Malformed stdout flooding proves
rejection/draining, not maximum admitted valid streaming; measure both separately.
This gives packaged responsiveness evidence without provider usage or a
production fake-admission endpoint. Fault-only cases still use separately
identified fixture builds. Any necessary measurement code follows existing
application/adapter boundaries and uses content-free counters.

Measure the budgets in [the AI experience contract](../architecture/ai-job-experience.md#accessibility-and-evidence): input-to-feedback target 100 ms, preview publication at most ten times/second per visible run, elapsed display at most once/second, no offscreen text renders, existing 64 MiB session payload budget and reserved save/control capacity. Record input sizes, stream rate, run duration, sample count, timing distribution/maxima, autosave acknowledgements, cancel request/ack times and cleanup, render counts and memory before/during/after repeated runs. Distinguish measured process RSS from the logical payload budget. Report missed targets as findings; do not invent a new RSS or cancel-latency threshold or silently relax the existing budgets.

Exercise disconnect/reconnect and replay gaps, same-session frontend reload, server restart and sleep/activation both with a surviving lease and after expiry. Verify origin-date routing, silent historical result restoration, no caret/focus/tab theft, truthful Interrupted/results unavailable states and zero implicit extra inference. Include reduced-motion settings in feasible automation or H08; VoiceOver remains exempt. Synthetic throughput measurements and real-profile integration each need their own evidence label.

Record measurement parameters before each run and repeat under the same
parameters after a fix. Include one visible stream and the maximum admitted
payload/rate, recording limits actually reached rather than assuming an
unbounded flood is representative. Input-to-feedback ends at the visible local
acknowledgement, separately from server admission and provider output. Measure
cancel request, server acknowledgement, terminal state and descendant cleanup
separately. Report sample count, median/tail/max where meaningful, save latency,
render/payload counts and idle memory after repeated cycles. The 100 ms value is
the existing target, not a new universal per-sample guarantee; other missing
numeric limits need an explicit engineering finding rather than an invented pass
threshold. When measurements are performed, a repeatable target miss is a finding requiring
triage or an explicit acceptance decision. Unmeasured H09 observations are exempt,
not passed; deterministic A05 assertions remain required.

## 5. Live provider boundaries

Use only synthetic text and disposable destinations. For an optional live observation needing an unavailable destination/profile, record the missing input and exempt disposition; continue required synthetic work. Obtain any necessary destination/account authorization at execution time, after preparing the concrete fixture/procedure. This planning request supplies no new service-write or inference authorization; retain authorizations already recorded in the decision register.

- Source saves: establish producer/phone reopen for recording and manual snippets, including live iCloud synchronization, guarded external changes and unchanged recording/audio metadata. Do not turn this into a phone Trash gate.
- iCloud export: record File Provider/TCC availability under actual packaged identities, exact file read-back, external-edit confirmation, cancellation/uncertainty/restart and absence of leftover staging files. Observe eviction/download-needed/denial only on disposable data where possible. Record cases that cannot be induced as unverified; local coordination success proves no remote-upload completion.
- Notion: compare the final adapter contract with prior live round-trip evidence and run the current fake-service recovery/limit matrix. Reuse `notion-live.py` only when necessary within its existing throwaway-root authorization; never clear/archive pages or widen the destination. Do not induce destructive remote failures to mimic local fault injection.
- Real profiles: reuse existing approved synthetic chat/generation/cancel paths for Codex, Claude and Apple as applicable. Check packaged prompt/helper integration and a small paragraph-review example with supported/uncertain context and creation-time clues. Record quality observations without treating a few examples as a general model evaluation. The owner reports Qwen Generate Draft, Review Paragraphs and AI Chat completed by human; see the manual guide. No additional manual live run is required.

Unavailable human/live observations remain unverified and exempt under the owner update. They do not block checklist completion or Done; required automated coverage and observed defect dispositions still apply. Preserve service authorization boundaries even for optional tests.

## 6. Final validation and documentation

Delegate command execution and first-pass failure analysis to `test_runner`, specifying changed files and required evidence and letting it discover documented commands. Run affected checks while fixing findings, then the complete seven-lane `scripts/gate.sh`, version/build checks and affected smokes on the final revision. Wait for the runner before judging completion. A later relevant change invalidates affected results; a green gate never establishes unobserved packaged/live behavior; exempt human evidence is not required for closure.

Update the user guide and README with tested release artifact layout, notices for standalone server copies, absolute-path independent launch, private configuration/discovery, save/close recovery, optional-agent limitations and export semantics. Audit existing text against the independent Paragraph Review profile and current Settings. Preserve recorded platform/SDK licensing caveats; no public-distribution or broader support claim follows from personal qualification.

For every result record: checklist/scenario ID and required/exempt gate classification; date; revision and artifact hash/build mode; environment; sanitized command/procedure; expected and observed outcome; metrics or screenshot/log reference; limitations; pass/fail/blocked/exempt with decision link. Keep raw credentials, private paths, content and logs outside the repo. Use synthetic screenshots and redacted diagnostic IDs; copy only reviewable evidence into the docs.

Finish with an A01–A07/R01–R15/E1–E13 coverage audit and Phase 1 exit-criteria review. Update linked M32 feature checklists with actual automated/native results and explicit exemptions for unrun human observations, then close/archive milestone plans and update roadmap, phase, handoff and index links together. This plan leaves implementation and release acceptance pending.

## Automatic image engine readiness follow-up

The Illustrated controller owns an epoch-bound, coalesced automatic check pass
on connection and Settings > AI Agents entry. Discover first, then await each
configured engine probe sequentially because the server registry serializes
engine access. Reuse the asynchronous bridge and server `spawn_blocking` check
route; no renderer or UI worker is needed. Check replies update cached readiness
through a fresh discovery read. Publish app-authored warnings for discovery,
probe and prerequisite failures, deduplicate unchanged problems until recovery,
and discard obsolete connection/disposal replies. Keep explicit Check as retry.

## Image engine server shutdown follow-up

The image registry owns a permanent shutdown flag shared with configured command
engines at startup and reload. ServerState shutdown sets it without taking the
registry's operation mutex, allowing an active probe/render to observe shutdown.
The command adapter passes it to the existing supervisor and rejects new work
once stopped. After either a termination signal or transport completion/error,
server cleanup stops agents and API work and waits on a blocking worker for the
image registry operation mutex, then awaits Illustrated worker acknowledgement
before dropping discovery and the workspace lock. This also releases already
completed photo staging held by a cancelled multi-photo job. The existing supervisor owns TERM/grace/KILL/reap and disposable render staging.
SIGKILL cannot run shutdown handlers; it is outside graceful termination.

## Krea factory image size follow-up

Set the repository Krea command wrapper's explicit width and height to 624 and
832, respectively. Keep generation settings and upscaling policy unchanged.
Verify the emitted CLI dimensions with the existing synthetic wrapper smoke;
no live GPU render is needed for this default argument change.
