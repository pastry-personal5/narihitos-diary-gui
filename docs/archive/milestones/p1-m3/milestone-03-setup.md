# P1-M3 setup inventory

Status: Done (archived)

The owner approved additional dependencies and build tools on 2026-11-05 during implementation. This record tracks selected setup and remaining evidence; it does not claim milestone completion.

## Environment and selection

Observed: macOS 26.7.1 (25G241), arm64; Rust/Cargo 1.98.0; Apple clang 21.0.0 through `/Applications/Xcode.app/Contents/Developer`. Installed Node 26.8.1/npm 11.19.0 precede the selected current-LTS Node 26.13.0/npm 11.20.0 baseline. The latter is verified in the [official Node release index](https://nodejs.org/dist/index.json); installation remains pending.

Initial tooling pins: Prettier 3.9.10 (MIT), ESLint 10.14.0 (MIT), @eslint/js 10.0.1 (MIT), Vitest 5.1.0 (MIT). Versions/licenses were read from the public npm registry before manifest creation. Lockfile resolution and transitive license/notice review remain pending. No optional adapter is selected in this initial assembly.

The Cargo workspace currently uses standard-library code only, includes all five planned crates and the client package as default members, and builds both binary entry points. Entry points explicitly fail because process/GUI composition is incomplete. The domain protocol compatibility predicate rejects major, minor and missing-capability mismatches; it is not yet wired to a handshake.

## Build integration and evidence

`npm run check:versions` currently compares package and Cargo workspace versions. Tauri version checking must be added when its config exists. No Tauri build or frontend production build is claimed. `npm test` will run Vitest after installation and test implementation. Formatter/linter traversal excludes reference submodules and generated output.

2026-11-05 working-tree evidence: `cargo fmt --manifest-path src-tauri/Cargo.toml` and `cargo test --manifest-path src-tauri/Cargo.toml` passed across six workspace packages, including one protocol mismatch test. This is initial assembly evidence only; all completion checkboxes remain open until final full-gate and native evidence exist.

## Coordinator increment

2026-11-05 working tree: application-owned monotonic lease and transient job coordinator added, without framework or serialization dependencies. Seven Rust tests pass for exclusion/expiry, lost-reply admission retry and mismatch, idempotent cancellation, count/result-metadata bounds, save-slot reservation, encoded replay size limits and reconnect gap/restart detection. Workspace Clippy passes. This proves application behavior only; adapters must supply cryptographic IDs/digests, monotonic clock, actual encoded envelope sizes and cancellation of running work. HTTP/SSE, permission inventories and native evidence remain pending, so S05/S06 remain unchecked.

Node 26.13.0 was downloaded into `/private/tmp/diary-p1-m3-tools`, checked against the official SHA-256 release manifest and extracted there without changing the system installation. Use its `bin` directory first on PATH for subsequent tooling. Initial npm installation uses `--ignore-scripts`; native build scripts will need explicit review where required. Architecture tests inspect Cargo metadata, including build/dev dependency edges, six-package default selection, both executable targets and inherited versions.

## Configuration increment

2026-11-05 working-tree S03 evidence: `cargo test --manifest-path src-tauri/Cargo.toml -p diary-infrastructure` passes seven synthetic tests. Implemented strict schema/tagged profiles, empty optional lists, port/alias/list/size/depth bounds, YAML duplicate/alias/anchor/tag rejection and scalar type checks. Root resolution detects equality/nesting and symlink physical aliases; profile/config paths cannot lie inside data roots. Config loading walks ancestors with descriptor-relative `O_NOFOLLOW`, validates opened current-user 0700 parent/0600 regular file, reads bounded bytes through that descriptor and checks existing workspace/runtime permissions. Tests cover unsafe modes, symlinked ancestors/files, replacement after opening, missing optional paths and no workspace creation. No real config/profile home or agent was read/executed.

Selected Rust pins: serde 1.0.229 (`derive`, MIT OR Apache-2.0), serde_yaml_ng 0.10.0 (MIT), libc 0.2.191 (MIT OR Apache-2.0), unsafe-libyaml 0.2.11 (MIT), dev-only tempfile 3.27.0 (MIT OR Apache-2.0). Cargo resolved their transitive versions in its lockfile. `unsafe-libyaml` provides an event preflight before recursive decoding; no native YAML library/build tool is added. Full transitive license/notice audit remains pending.

S03 remains incomplete: executable PATH resolution/availability summaries, broader adversarial schema/path coverage and server composition are still required. S04–S09 transport/native work and final full gate remain pending. npm install is still live under execution session 90433 and stalled resolving the Vite packument at the last observation; do not start a competing install without checking that handle.

## Lifecycle increment

2026-11-05 working-tree S04 evidence: infrastructure tests now pass 12 tests, including five lifecycle cases. `cargo test --manifest-path src-tauri/Cargo.toml -p diary-infrastructure` needed loopback access outside the sandbox for the real contiguous-port test; the sandbox attempt failed to acquire any test sockets, while the loopback-enabled rerun passed. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` passed afterward.

Implemented an OS advisory lock on an existing fixture-prepared `.diary/workspace.lock` (no initializer), SHA-256 workspace key, IPv4-only first-available bounded binding, fresh native OS entropy for UUID/token, strict bounded private discovery reads, staged publication, and removal guarded by instance identity. Tests cover lock exclusion/release, untouched uninitialized workspace, private record round trip, restart credential rotation, old-instance cleanup retaining a newer record, hostile origin/schema/oversize rejection, occupied-prefix fallback through nine candidate ports and complete exhaustion. Added exact pins serde_json 1.0.154 and sha2 0.11.1 (MIT OR Apache-2.0); transitive license/notice audit remains pending.

These are adapter tests, not server/native completion: independent executable composition, readiness ordering, signal/crash integration, authenticated liveness, descriptor-relative publication/cleanup hardening and packaged procedure remain pending. S04/S05/S09 remain unchecked. The pending npm process was confirmed live (PID 2579, execution session 90433), still fetching the Vite registry metadata; no duplicate installation was launched.

## Standalone server transport increment

2026-11-05 working tree: `diary-server --config PATH` now composes strict config, existing-workspace lock, first-available IPv4 listener, authenticated router and private discovery. `--development-origin http://localhost:1420` is the only optional development origin; ordinary packaged policy targets `tauri://localhost`. Status/settings explicitly report storage/source/agent/export unavailable. Registered HTTP subset covers status, session lease, settings, job list/read/cancel and SSE. No admission endpoint or source/agent adapter exists.

`cargo test --manifest-path src-tauri/Cargo.toml -p diary-api` passed three route tests covering auth/Host/Origin/proxy/query/peer denial, redacted status/input errors, client exclusion/reconnect, session-required jobs and absent fake-admission route. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` passed before the final signal-registration change; final gate remains pending.

`cargo build --manifest-path src-tauri/Cargo.toml -p diary-server && python3 scripts/smoke/server.py` passed with loopback permission after fixing the observed startup signal race by registering SIGINT/SIGTERM handlers before discovery publication. The smoke creates/removes only temporary invented config/workspace/runtime state and verifies real independent process launch, unauthenticated denial, instance/status match, second-client rejection, OS-lock exclusion, client lease release leaving the process live, signal cleanup and restart token/instance rotation. No credentials are printed. This is a debug-server smoke, not packaged Tauri/macOS evidence.

Added pins: axum 0.8.9 (default JSON/HTTP features), tokio 1.54.1 (API time/sync; server macros/multi-thread/net/signal/time), serde/serde_json as above, UUID 1.29.0 (`v4`), subtle 2.6.1 (constant-time token comparison), futures-util 0.3.34 (SSE stream); test-only tower resolves to 0.5.3 (`util`). These use MIT and/or Apache-2.0 license declarations; complete resolved notice audit remains pending.

Remaining transport work includes heartbeat/expiry sweep with a suspend-aware clock, shutdown deadline/stream cancellation, bounded cursor pagination, complete fake-job HTTP/SSE replay/resync tests and typed bridge integration. Current SSE route is initial plumbing only; S04–S06/S09 stay unchecked. npm session 90433 was re-polled and remains live; no competing install has been started.

## Transport recovery increment

2026-11-05 working tree: seven API tests pass (`cargo test --manifest-path src-tauri/Cargo.toml -p diary-api`). Added test-only coordinator composition through application admission, exercising a synthetic cancellation via real HTTP/SSE handlers, authoritative state before notification, bounded reconnect resync after replay loss, injected-clock expiry, shutdown admission denial and bounded job cursor query. No production fake-admission route was added. Complete slow-consumer/flood, exact SSE-byte, stream-disposal and fixture exclusion evidence remains pending.

Server now sweeps expired leases every second using an injected clock boundary (production macOS raw monotonic clock), stops admission/erases transient session state on shutdown and caps graceful network drain at ten seconds. Jobs use validated `?cursor=` pagination with 200-row pages; other query strings remain forbidden, including credentials. Workspace-wide Clippy passed, and `cargo build --manifest-path src-tauri/Cargo.toml -p diary-server && python3 scripts/smoke/server.py` passed again after these changes with loopback permission.

Formatting now runs `cargo fmt --manifest-path src-tauri/Cargo.toml --all` while iterating. The unchanged root fmt gate alone needs a coverage audit because Cargo formatting package selection differs from default-member test selection; S02 remains open until this is resolved/documented without weakening the required gate.

The original npm install was explicitly terminated after process/log inspection confirmed it remained stalled at Vite metadata while a separate public registry request succeeded; execution session 90433 returned terminal exit 143. A new bounded install (15-second fetch timeout, one retry, exact Vite 8.3.6 pin) is running as session 79884. Inspect that live handle before starting another install. Tool installation, lockfile and frontend gate evidence remain pending.

## Frontend assembly increment

2026-11-05 working tree: initial npm tooling installation completed (session 79884, exit 0, 114 packages). Added text-labeled Standard Layout with the canonical toolbar/Icon Bar/Explorer/Tabbed Editor/AI Chat regions, disabled capability explanations and a dedicated CodeMirror create/destroy module. The scratch buffer is explicitly synthetic, never saved and has no persistence calls. Added a typed bridge port, fixed-name Tauri invoke adapter, allowlist DTO reconstruction and tests for incompatible/malformed DTOs, redaction, disconnected/reconnected UI, canonical regions and CodeMirror cleanup/remount. These tests/build are pending execution, and Rust Tauri composition/channel plumbing is still absent; S07/S08 remain incomplete.

Selected exact frontend versions are in `package.json`: React/React DOM 19.3.1, Tauri API 2.12.4, CodeMirror state/view/commands/Markdown 6.7.6/6.43.15/6.11.1/6.5.2, plugin-react 6.2.1, React typings 19.3.0, TypeScript ESLint 8.72.1, hooks/refresh ESLint plugins 7.1.1/0.5.7, jsdom 30.1.3 and Testing Library React/DOM 16.3.3/10.4.2. React/editor/test packages declare MIT; TypeScript declares Apache-2.0; Tauri API declares Apache-2.0 OR MIT. Resolved transitive audit remains open.

npm rejected TypeScript 7.0.2 against TypeScript ESLint's declared `<6.1.0` range. The public npm registry confirms stable TypeScript 6.0.3; that compatible exact version replaces the initial candidate. The retry install is live in session 24220; inspect it before another install. Source/config/test files exist but no passing frontend build or UI test is claimed yet. UI guidance used the local ui-ux-pro-max React effect-cleanup/accessibility search, retaining the repository's existing containment contract.

Frontend verification update: session 24220 completed successfully (132 additional packages). `npm run build` passed after adding Vite CSS ambient types: app/Cargo version check, strict TypeScript type check and Vite production output. Vite reports a 719.70 kB minified shared editor/app chunk (242.39 kB gzip); this is an observed bundling warning, not runtime performance evidence. `npx vitest run` passed 3 files/10 tests, including four client/editor tests, three DTO tests and three architecture/version tests. `npx eslint . --max-warnings 0` passed after importing Node's URL explicitly in the version checker. Maintained frontend/config/test files were formatted with Prettier; the final repository-wide formatting gate remains pending. No Tauri app was built/launched and no native visual/accessibility evidence is claimed.

## Tauri assembly increment

2026-11-05 working tree: approved pins Tauri 2.12.5/tauri-build 2.7.4 and npm Tauri CLI 2.12.4 installed. Reqwest 0.13.5 disables default features and enables only JSON/stream (loopback HTTP needs no TLS). Rust bridge holds discovery/token/session, reads discovery through descriptor-relative no-follow opens with private metadata checks and a 16 KiB limit, disables proxies/redirects, caps HTTP responses at 10 MiB and checks handshake instance/workspace/API/capabilities. Fixed commands cover connect/status, sessions, settings and job queries/cancel. `subscribe_events` currently returns an explicit unavailable error; forwarding/cleanup is pending. Window checks additionally reject non-main callers.

Tauri config explicitly selects only the local main-window capability and its individual generated command permissions; build manifest and invoke handler list the eleven scaffold commands. Version checking now includes the Tauri literal. Local-content CSP restricts WebView transport to IPC. Packaged origin/CSP and secondary-window denial are still unverified. Shell launch is `diary-client --connect PATH`; no server supervision was added.

`cargo check --manifest-path src-tauri/Cargo.toml -p diary-client` passed after adding an original project-owned SVG app icon (`assets/icons/app.svg`) and generating only retained macOS PNG/ICNS assets through the approved Tauri CLI. This uses no SF Symbols assets. Tauri minimum OS configuration follows the selected 26.7.1 acceptance baseline, not a claim of broader compatibility. `npx vitest run` again passed all 10 tests. Workspace Clippy was started as session 82677; inspect its result before claiming success. S02/S05/S07/S08/S09 remain incomplete pending full typed transport/channel tests and packaged acceptance.

## Event-channel/session increment

2026-11-05 working tree: `subscribe_events` now opens authenticated bounded SSE and forwards allowlisted typed Tauri channel messages. Rust validates event kind/state/identity/id/sequence consistency, suppresses stale-session/instance and duplicate event/job sequences, and caps partial frames and replay-filter metadata. Comments/CRLF/split frames are handled. Stream HTTP disables proxies/redirects with 5-second connect and 15-second read-idle deadlines independent of the ordinary request deadline. Reconnect/session close aborts the stream task; channel-send failure ends it. Unknown required events report disconnected; replay gaps report resync. No raw SSE/error/token contents cross IPC.

`cargo test --manifest-path src-tauri/Cargo.toml -p diary-client` passed three decoder/filter tests. `npm run build`, `npx vitest run` (10 tests) and `npx eslint . --max-warnings 0` passed after adding the TypeScript event validator and watch lifecycle. React opens a lease/subscription after handshake, heartbeats every five seconds without overlapping calls, shows interrupted transport, and disposes heartbeat/channel/session on reconnect/unmount. Rust chooses and retains its proposed session UUID before admission for lost-reply retry. Workspace Clippy was launched as session 96450; inspect outcome before claiming it passed.

Still required: dedicated adapter timer/channel cleanup and hostile stream/response tests, complete paginated snapshot refresh after resync, bounded channel backpressure evidence, reconnect/close race review and packaged IPC/CSP/denial/folder smoke. These code paths remain scaffold-only and do not establish durable storage, AI or complete native acceptance. S05–S09 remain unchecked.

## Snapshot recovery and adapter evidence

2026-11-05 working tree: job-list pages now include an additive server event watermark captured under the coordinator lock. Before subscribing, the Rust bridge reads bounded pages and sends typed `job_snapshot` pages, then resumes SSE from the first page's watermark so racing later changes replay. Invalid/nonprogressing cursors and excessive page counts reject recovery. The decoder additionally bounds total decoded frame bytes per received chunk. The TypeScript adapter validates snapshot pages, retries gap recovery at most three consecutive times, prevents overlapping heartbeats and disposes callbacks/timers/session once. Unmount during handshake skips session creation; failed subscription setup disables its callback before release.

`npx vitest run` passes 4 files/15 tests, including five new adapter tests for timer overlap, idempotent release, late callback suppression, failed-subscription cleanup, resync resubscription, heartbeat error redaction and bounded snapshot validation. `npm run build` passes. `cargo test --manifest-path src-tauri/Cargo.toml -p diary-client -p diary-api` passes 3 client and 7 API tests; workspace Clippy passes. ESLint initially caught an unused test callback argument; the corrected rerun succeeded before packaging started.

Added the standard Tauri `custom-protocol` feature forwarding for bundled local content. `npm run tauri build -- --debug --bundles app` is running as session 11806; inspect that exact handle before another package build. No packaged runtime/native check is yet claimed. Remaining acceptance includes hostile bridge/discovery/HTTP tests, channel backpressure and lifecycle race coverage, config availability/executable selection, cross-process crash/stale discovery tests, resolved dependency notices, final full gate/docs and packaged smoke. All S completion items remain open.

## Packaged IPC increment

The ordinary debug `.app` bundle completed, followed by a `scaffold-smoke` fixture bundle with the same command/capability inventory. Actual packaged main-window metadata/handshake/session release and secondary-window denial passed in the isolated standalone process smoke. Exact command, environment, binary hashes, observations and limitations are in the [native evidence record](milestone-03-native-evidence.md). No S09 completion is claimed. Main-window close handling now waits for bridge lease release before destruction, and secondary-window events cannot trigger session cleanup; its native close test is still pending.

`npx vitest run` passed 5 files/19 tests after adding exact command/manifest/permission inventory checks, local capability/CSP checks, deliberate version-drift rejection and workspace-wide formatting verification within the full gate. This supplements the unchanged root fmt command with coverage of every Rust member. Fixture-feature Clippy is running as session 79975; inspect its outcome before claiming a pass.

## Configuration availability increment

2026-11-05 working tree: startup now resolves CLI default executables against the server launch PATH/cwd and honors explicit absolute overrides without falling back from a missing override. Selection checks canonical regular executable files with `access(X_OK)`; it never runs them. Source/profile directory observations report typed missing/denied/unverified reasons without reading profile contents. Apple profiles report native-adapter-unverified and use no CLI fields. The application-owned summary has aliases/reasons only; server Settings and typed bridge DTOs expose no paths/credentials and keep availability false until the later adapter milestones.

Added explicit null/empty/path-length/NUL checks and component-aware canonical resolution, including missing-parent `..` normalization. Optional missing/denied roots can remain configured without blocking schema validation; overlap checks still reject private profile/config paths inside data roots. The initial denied-root fixture accidentally reused a source root as a profile home and correctly failed overlap validation; separate synthetic denied directories now pass and report their distinct unavailable reasons.

`cargo test --manifest-path src-tauri/Cargo.toml -p diary-infrastructure --lib config::tests` passes 12 tests; `cargo test --manifest-path src-tauri/Cargo.toml -p diary-api` passes 8 including exact redacted startup Settings summaries. Workspace Clippy rerun is live in session 2679; inspect before claiming success. The fixture native bundle evidence from the previous increment remains limited to its recorded binary hashes; these new sources have not been repackaged. Final configuration/gate/native audits are still pending, so S03 remains open.

## Descriptor-relative publication and crash evidence

2026-11-05 working tree: private config/discovery filesystem access shares a narrow internal directory-descriptor adapter. Runtime/key directories and staged files are opened without following symlinks and validated on their opened metadata. Publication writes/syncs/stages/renames relative to those descriptors; cleanup retains the descriptor and atomically moves the public record into an exclusive private cleanup slot before validating ownership. A newer public record cannot be unlinked through a stale pathname check. Foreign/malformed/symlinked records are restored without overwriting a later replacement; if restoration loses a race, the moved record stays preserved privately.

`cargo test --manifest-path src-tauri/Cargo.toml -p diary-infrastructure` passed all 20 tests with loopback access. New tests relocate the runtime path after publication, reject runtime/key symlinks without touching their targets, and preserve malformed/symlinked replacement records. Workspace Clippy passed. The rebuilt independent server passed `python3 scripts/smoke/server.py`, now including SIGKILL, stale-record liveness failure and fresh-instance/token rotation after crash. The smoke readiness check matches the launched PID so it cannot mistake a stale record for newly published readiness. This is synthetic debug-server evidence; recorded packaged-client hashes remain from the earlier observation and are not updated by these source changes.

S04/S05 remain open for the complete acceptance audit, including the client's adversarial discovery/HTTP matrix and packaged/native scenarios. No durable initializer/recovery or source/agent operation was added.

## Adversarial client discovery/HTTP evidence

2026-11-05 working tree: client test setup now uses synthetic private discovery files and a local raw HTTP fixture server. `cargo test --manifest-path src-tauri/Cargo.toml -p diary-client` passed 12 tests with loopback access, including a child-process handshake under hostile HTTP_PROXY/ALL_PROXY and empty NO_PROXY settings. The child runs the actual bridge handshake test and succeeds without contacting the configured proxy. Request assertions check the fixed status path, exact loopback Host and bearer header, with no query credential.

Tests reject malformed/oversized/duplicate-key discovery, invalid instance/token/workspace/protocol and noncanonical/credential-bearing origins; unsafe file/parent modes, leaf/ancestor symlinks and a pathname replacement after opening are exercised. HTTP tests cover incompatible instance/workspace/API/capabilities before session use, redirects, wrong response media type, declared and streamed-over-limit bodies (including chunked transfer without Content-Length). Replacing discovery with invalid bytes now clears a previous Rust connection/session instead of leaving its credentials live. Response content type and status capability count/string bounds are enforced before forwarding.

Workspace Clippy passed after the initial nine-client-test increment; three added tests afterward passed with the complete 12-test suite. Only existing approved tempfile/Tokio versions were added as client test dependencies. No real discovery/config/profile or remote service was used. Final all-target checks and packaged evidence for these newer sources remain pending; S05 is not yet marked complete.

## Replay ordering and slow-consumer bounds

2026-11-05 working tree: publication now deduplicates unchanged per-job sequences, so repeated cancellation returns the same notification/event ID instead of fabricating additional progress. Per-job notification bookkeeping is included in a larger conservative metadata reservation. API SSE polling retrieves one event rather than cloning the complete replay ring on every pull. Four owned stream slots bound simultaneous live SSE consumers, with the limit exposed in status; dropping a stream releases its slot. Status, heartbeat and cancellation remain outside that bound.

`cargo test --manifest-path src-tauri/Cargo.toml -p diary-application -p diary-api` passed 8 application and 10 API tests. Added idempotent notification/watermark and slow-consumer slot/control-access tests. The earlier replay-gap fixture repeatedly published one unchanged state; after publication deduplication it no longer overflowed replay and its unbounded body read waited. That known test process was explicitly terminated (session 89766, SIGTERM), then the fixture was corrected to queued/running/cancelled transitions and its read was bounded by a timeout. The corrected gap test passes. No failed or hanging run is counted as acceptance evidence.

Channel acknowledgement/backpressure, complete flood/native fake-job composition and remaining packaged acceptance are still pending; these server bounds alone do not establish bounded Tauri/WebView delivery. S06 stays unchecked. Workspace Clippy caught a collapsible conditional in publication; it was corrected before the final rerun.

## Bounded Tauri delivery increment

2026-11-05 working tree: event and snapshot channel packets carry a subscription UUID and delivery counter. A window of eight unacknowledged packets bounds queued data, with a one-second wait and one reserved disconnect packet on a stalled stream. Encoded packets are limited to 64 KiB before channel send. Cumulative acknowledgements reject future counters, ignore stale subscription IDs and cannot change domain/job/SSE cursors. Start and acknowledgement are a tagged payload of the existing `subscribe_events` command; the command/permission inventory is unchanged. Start carries Tauri's `JavaScriptChannelId`, resolved once on the calling WebView; acknowledgements do not reconstruct/drop the streaming channel.

The bridge serializes connect/subscribe/close lifecycle changes and aborts/stops delivery on disposal. The adapter validates each packet/event, acknowledges processed messages, rejects delivery gaps and ignores callbacks from replaced channel generations. Tests use paused Tokio time to exercise a full delivery window without sleeps. `cargo test --manifest-path src-tauri/Cargo.toml -p diary-client delivery::tests` passed two flow tests; additional encoded-size/reserved-terminal channel tests are running as session 28182. Workspace Clippy passed after the flow integration. `npx vitest run` passes 5 files/22 tests and ESLint passes. The frontend build passed before the final channel-ID payload correction; final build/native verification remains pending.

Initial compilation correctly rejected nested `Channel<T>` deserialization. The final implementation uses the Tauri-supported `JavaScriptChannelId` within the tagged start payload; this avoids accidentally unregistering the streaming callback when an acknowledgement completes. Existing native fixture evidence remains tied to its earlier binary; no updated packaged-channel runtime result is claimed. S06/S07 remain open for the full native/flood/lifecycle acceptance audit.

## Packaged fake-job increment

A fresh fixture app/server passed the actual native snapshot/acknowledged-channel/cancelled-event/readback/release sequence with secondary-window denial and lifecycle/crash smoke. Exact commands, hashes and evidence limits are recorded in [native evidence](milestone-03-native-evidence.md#packaged-fake-job-channel-observation--2026-11-05). Fixture-feature workspace Clippy passed. Ordinary debug bundling is now running as session 43188; inspect its result before binary marker checks or another bundle build. The new exclusion checker requires already-built ordinary binaries and bundle; its source presence alone is not exclusion evidence.

Ordinary bundling session 43188 finished successfully. `node scripts/check-fixture-exclusion.mjs` passed against both standalone binaries and the ordinary packaged client; `npx vitest run` passed 5 files/23 tests including opt-in fixture composition checks. Remaining native folder/editor/CSP/close evidence and final acceptance/gate/docs work stay open.

## Native folder/CSP increment

The packaged fixture now asserts actual origin and a `connect-src` CSP violation before performing its native channel checks. A fixture-only standalone server module reads a fixed invented sentinel from two configured test roots, reports verified readable bytes or actual denied error as bounded Settings outcomes, and never exposes paths/content through IPC. Native smoke passed twice; the final run recorded server EACCES (errno 13) and all prior IPC/channel/lifecycle markers. Exact hashes, procedure and limits are in [native evidence](milestone-03-native-evidence.md#packaged-csp-and-synthetic-folder-observation--2026-11-05). The script restores its denied fixture directory permissions during cleanup. Python bytecode output is now ignored.

Current app/server artifacts are fixture builds. Ordinary rebuild/exclusion, editor/native-close scenarios, final dependency notices/run docs and the full six-command gate remain pending. No S09 completion is claimed. The normal API schema remains unchanged; only the fixture Settings response carries folder outcomes and both server/client compile those fields out in ordinary builds.

## Dependency inventory and handoff documentation increment

Collected 429 Cargo plus 282 npm resolved package records and 375 distinct upstream notice files with hashes/provenance paths using `scripts/collect-notices.py` and existing tooling. The collector reports published packages lacking separate installed notice files; root [third-party notices](../../../../third-party-notices.md) explicitly leaves review open rather than treating SPDX metadata as clearance. Raw legal text is preserved without formatting. README/contribution instructions now describe the actual disposable scaffold, separate server/client commands, current toolchain, fixture boundaries, ordinary exclusion and notice regeneration. The indexed [acceptance audit](milestone-03-acceptance-audit.md) retains all original S/G requirements and remaining evidence. S01/S10 and the overall milestone remain incomplete.

## Upstream notice supplementation increment

Fetched repository-level license assets only from the exact Git SHA recorded in published crate metadata, preserving source URLs and content hashes in `supplemental-notices.json`. The collector now merges those supplements and preserves 378 distinct text assets across 711 package records. The remaining no-notice list is reduced to six Cargo and seven installed npm records. Some collected repository notices (notably objc2) link to standard license terms and document SDK-derived-source caveats; collecting a notice is not a blanket license-clearance claim or a change to the approved personal Tauri scope. Full text/provenance review remains open.

The fetcher executes no upstream code and installs no tool. Integrity tests verify collected legal bytes/hashes and exact npm lockfile-version coverage; they intentionally do not claim that the remaining license review has passed. S01 remains unchecked. Existing native/compiler evidence is unchanged by this legal-asset collection.

## Notice coverage closure increment

Collected original AUTHORS/README/source-header notices for packages that omit LICENSE files, shared same-commit crate notices and same-version official native-binary wrapper notices, exact-release saxes upstream text, and declared standard license texts. The inventory now contains 711 records/389 distinct notice assets; no resolved Cargo or installed npm package remains without collected notices. Selected alternatives prefer MIT then Apache-2.0 and preserve every AND component; no LGPL alternative is selected. Raw upstream SDK caveats are retained without claiming independent clearance. The root notice document records the policy, provenance and unmodified MPL source links.

The Tauri resource map now includes the inventory/legal assets and root notices. Resource inclusion still requires a fresh bundle/inspection; the source configuration alone is not packaged evidence. Notice integrity/version tests pass, now also checking nonempty required notice coverage and explicit alternatives. Native editor/normal-close/reconnect checks, final audits and the full gate remain pending; S01 remains open until current package resources and exact tool integration evidence are verified.

## Native editor/close harness increment

The new fixture sequence focuses the actual CodeMirror content DOM, waits for fixed synthetic text entered through macOS System Events, then completes fake cancellation/channel/readback, explicit release/reconnect and an active-lease normal window close. The fixture's scheduled native `Window::close()` uses the ordinary CloseRequested handler; successful close release and main destruction have distinct markers. A competing packaged process is checked for the allowlisted `client_busy` error. Production bridge errors now preserve that bounded code, and the UI explains that another client must close before reconnecting.

The first combined run failed the text assertion (no cancelled-job/readback marker) while normal window close/release markers were observed. It is not passing keyboard evidence. Focus timing was corrected to settle after native activation and wait for observed sentinel text before launching the competitor. The corrected fixture bundle built successfully and its smoke is live in session 61322; poll it before starting another runtime run.

A direct resource inspection of the new bundle passed: root third-party notices and inventory matched source bytes, and all 389 packaged license text hashes matched the inventory. This establishes actual resource inclusion for the fixture bundle, while final ordinary packaging/exclusion remains pending.

## Native keyboard diagnosis increment

The corrected-focus smoke from session 61322 still failed. Added bounded fixture probes reporting only native/document focus, text length, trusted key counts and sentinel-match booleans. The diagnostic run observed `focused=false`, active CodeMirror DOM, length zero and zero keys/trusted keys. A follow-up explicitly focusing both window and WebView also failed: System Events reported the target non-frontmost with zero accessible windows. These failures are not keyboard/input acceptance and do not indicate data persistence.

The latest fixture adds explicit regular activation policy, app/window show and WebView focus to test shell-launched native visibility. It rebuilt successfully; runtime smoke is live as session 2954 and must be polled before another run. Production command grants are unchanged. Added an adversarial bridge fixture test for the allowlisted client_busy code without private error details; it has not yet been run. The existing 24 JavaScript tests and ESLint passed during diagnosis.

No native failure has been substituted with synthetic DOM insertion or a mock keyboard result. S07/S09 remain open, and independent final gate/config/security/documentation audits can still progress if GUI input requires further work.
