# P1-M3: Scaffolding work plan

Status: Planned

Assemble the [scaffolding checklist](milestone-03-scaffolding-overview.md#completion-checklist) from the completed P1-M2 design. This work plan defines modules, the implemented interface subset, sequencing and acceptance evidence; the maintained architecture remains authoritative for product behavior.

## Required inputs

| Contract | Use in this milestone |
|---|---|
| [Dependency rule and component allocation](../architecture.md#component-allocation) | Crate/client boundaries and Cargo workspace coverage |
| [API and processes](../architecture/api-and-processes.md) | Two commands, discovery/auth, leases, explicit Tauri permissions, bounded HTTP/SSE and errors |
| [Configuration](../architecture/configuration-and-exports.md#configuration-schema-and-validation), [versions/platform](../architecture/configuration-and-exports.md#version-and-platform-strategy) | Strict startup schema, private paths, optional profiles, version compatibility and native acceptance target |
| [Workspace schema](../architecture/domain-and-persistence.md#workspace-and-file-schema) | Lock location and initialization/recovery boundary; no partial durable storage implementation |
| [Containment](../ux-information-architecture.md), [terms](../ux-terms.md), [GUI](../ux-gui.md) | Minimal Standard Layout and accurate capability/connection labels |
| [Handoff E1/E10/E11](implementation-handoff.md#capability-evidence-register) | Evidence ownership; all other feature/runtime gates remain deferred |

Inspect the [prompt inventory](../../assets/prompts/README.md) before any later agent behavior work. This milestone validates profile configuration only; it neither edits prompt behavior nor loads/invokes an agent.

## Module allocation

Create the locations already specified by the architecture; add only types/ports required by the skeleton. Empty future features do not need placeholder implementations that could be mistaken for supported operations.

| Location | Scaffold responsibility |
|---|---|
| `src-tauri/Cargo.toml` | Workspace plus Tauri client package; explicit default members include the client and all five crates below |
| `src-tauri/crates/diary-domain` | Pure values/invariants needed by the scaffold; no framework, serialization or I/O types |
| `src-tauri/crates/diary-application` | Required application ports and transient job/session coordination; depends inward on domain |
| `src-tauri/crates/diary-infrastructure` | Config, lock/discovery and clock/entropy adapters; test fakes implement application ports |
| `src-tauri/crates/diary-api` | Typed HTTP/SSE DTO conversion, auth/admission and redacted errors; depends on application |
| `src-tauri/crates/diary-server` | Independent `diary-server` executable and server composition root |
| `src-tauri/src` | `diary-client` composition, discovery/session HTTP bridge and Tauri channels; no server use cases |
| `src/application`, `src/adapters` | Client state/bridge interface; Tauri invocation/channel adapters and DTO validation |
| `src/editor`, `src/components` | CodeMirror lifecycle module and text-labeled React regions |

Architecture checks must inspect dependency edges and workspace package selection, including build/dev dependencies where applicable. Do not import reference submodules, share HTTP DTOs as domain types or introduce an outward crate dependency to satisfy a test. Keep reference submodules and generated/build output outside formatter/linter traversal without excluding maintained application code, tests or docs.

## Startup and fixture boundaries

Implement real config validation, OS lock exclusion and discovery/session behavior against isolated synthetic roots. Workspace initialization and operation recovery classification belong to P1-M5; Authentic publication/recovery belongs to P1-M6. Until their applicable evidence passes, do not declare storage mutation-ready, manufacture a partially initialized workspace, repair private files or open an existing database as if its state were validated. Lock tests use a fixture-prepared workspace tree following the maintained schema; fixture creation is test setup, not the product initializer. Status exposes storage/features as unavailable, independently of a successful connection.

Validate the whole startup schema, including disabled export/profile sections, with invented credentials only. Missing source roots/profiles remain unavailable according to the contract; malformed config, unsafe permissions and root overlaps reject startup. CLI executable selection may resolve launch `PATH`/override paths without executing them. Do not run `--version`, login, inference or a native model availability probe as part of scaffold startup; availability checks requiring adapters remain deferred. Diagnostics identify fields/error codes without printing rejected values, raw YAML, token bytes or private profile paths.

Fake jobs are deterministic in-memory application-port implementations. Test-only composition seeds jobs through those ports after session admission, then drives queued/running/terminal snapshots, cancellation, floods and replay gaps through the real status/cancel/SSE transport. Test the shared transient admission/deduplication logic directly through application ports; no import/generation endpoint is needed to admit a fake. The harness has no file mutation, child process, model, SQLite or export adapter. No fake-job, URL, path or shell command is added to the API or Tauri inventory. The ordinary build excludes fixture composition; tests must inspect that exclusion. Fixture and ordinary builds use the same command permissions, including denial for an unprivileged test window.

Keep fixtures in temporary test-owned directories. Never read `config/global_config.yaml`, producer/private samples or actual CLI profile homes to populate tests. Native folder access tests read invented sentinel files from the server process; the WebView receives a bounded outcome, not a general path-access command. Cleanup may remove only fixture-owned resources and discovery records belonging to that server instance.

## Implemented interface subset

Paths below are relative to `/api/v1` and reuse the [maintained command inventory](../architecture/api-and-processes.md#operations-and-tauri-commands). Omit future commands from the invoke handler, app manifest and capability grants together; disabled UI alone does not restrict access.

| Explicit Tauri commands | HTTP/local behavior in P1-M3 |
|---|---|
| `connect_server`, `get_status` | Discovery validation and authenticated `GET /status`; distinguish connected, unavailable capabilities and incompatible protocol |
| `open_session`, `heartbeat_session`, `close_session` | `POST /sessions`, `POST /sessions/{id}/heartbeat`, `DELETE /sessions/{id}`; one in-memory client lease |
| `get_settings` | `GET /settings`; redacted startup capability/config summary only, with no credentials or private paths |
| `get_job`, `list_jobs`, `cancel_job`, `subscribe_events` | Session-owned status/list/cancel and SSE/channel plumbing; synthetic jobs exist only under test composition |
| `get_app_metadata` | Local typed app version DTO |

Defer config reload, source/Authentic operations, durable `get_operation`, review/chat/generation/results, profile checks, exports, native symbols, settings persistence and client log commands to the handoff milestones. Return no fabricated successful feature result. DTO/error unions describe only implemented behavior; retain the contract's additive-field handling and reject malformed/oversized or unknown required variants.

Register every included command in `AppManifest::commands`, its invoke handler and individual main-window permission. Explicitly select capability identifiers; reject overlapping grants and secondary-window access. `--connect` is the explicit discovery-path exception at the Rust process boundary, never an arbitrary path accepted from React. Keep the API token/session in Rust bridge memory, disable proxies/redirects, use local-content CSP and cancel subscriptions on disconnect/unmount.

## Approach

1. **Prepare S01 approval.** Inventory exact direct versions, transitive license/notice review, Rust/Node/npm/Tauri/Xcode/macOS/CPU set, enabled native features, test/fixture features, package scripts and bundling/build integration. Include the frontend dev/build/type-check runner, formatter, ESLint and Vitest setup. Distinguish the research machine's observed Node version from the required current-LTS choice. Ask for the concrete package/tool changes only after this inventory is reviewable; do not install from candidate recommendations.
2. **Assemble S02/S08 after approval.** Create the workspace, two binaries and frontend modules/configuration. Verify Cargo metadata/default-member coverage, dependency direction and both executable outputs. Establish version consistency and actual frontend type/build checks early so later work uses the same gate. Keep the two development launch commands independent; a client script never supervises the server.
3. **Implement S03–S05.** Add strict startup validation, private lock/discovery lifecycle, loopback binding, token validation, status, lease and redacted settings. Test failure paths before wiring UI success. No unimplemented durable recovery may become an implicit prerequisite or claimed startup success.
4. **Implement S06/S07.** Wire the typed bridge/channels and fixture composition, then reducers/DTO checks and minimal CodeMirror/React layout. Label the scratch editor synthetic and unsaved; do not display Authentic Saved state. Show the Main Toolbar Strip, Icon Bar, Explorer's Date/Snippet/Draft/Entry tabs, Tabbed Editor and AI Chat with accessible text. Settings/features may be disabled placeholders with reasons; full resizing, calendar/import, persistence, shortcuts and AI activity interactions remain deferred. Test editor/subscription cleanup and transport recovery with injected clocks/state rather than slow sleeps.
5. **Collect S09 evidence.** Build the standalone server and Tauri app with approved commands, including frontend type checking and the packaged fixture variant. Run the native procedure below and verify the ordinary build excludes fixtures. Fix observed scaffold failures; leave later feature/native UX gates in place.
6. **Close S10/G01–G06 only with evidence.** Run affected checks while iterating and the full six-command gate once after final changes. Record exact environment/results, update run/contribution docs and repair links. Keep P1-M3 incomplete if approval, any gate command or native evidence is unavailable; archive plans/update milestone order only after actual completion.

## Acceptance matrix

These are planned tests, not observed results. Store outcomes under the overview's [evidence record](milestone-03-scaffolding-overview.md#evidence-record); all fixtures are synthetic.

| Item | Minimum verifiable evidence |
|---|---|
| S01 — E10 setup | Owner-approved inventory and actual tool versions; compatible direct/transitive licenses/notices, enabled features and platform/build integration recorded. No unapproved optional adapter/tool is pulled into scaffold scope. |
| S02 — Assembly | Cargo metadata and architectural checks cover every specified member/default member and inward edges; root fmt/clippy/test selection covers both binaries/all crates. Approved frontend production build/type check fails on type errors and succeeds for the scaffold; a Tauri bundle and standalone server are produced. |
| S03 — Config/E11 schema | Valid minimal config with empty lists; reject wrong/unknown/duplicate keys, YAML aliases/tags/coercion, unsupported version, size/depth/list limits, unsafe owner/mode/symlink and root equality/nesting/physical aliases. Test absolute required roots versus config-relative other paths; duplicate aliases, missing sources/profiles, all CLI home/default/override rules, rejected legacy `home`, and Apple alias/type-only acceptance with forbidden fields rejected. No executable/model runs. |
| S04 — Launch/E1 | Separate config/connect commands; second server cannot acquire the workspace OS lock or modify discovery. Default first bind, occupied-prefix fallback through 8789, count 0 and 8, invalid counts/port overflow and complete range exhaustion. Discovery is published only after scaffold readiness, has private modes and fresh instance/token, and is removed only by its owning instance. SIGINT/SIGTERM release resources; crash leaves a stale record that cannot establish liveness; restart requires a fresh handshake. Client close leaves server running. |
| S05 — Access/E1 | Reject wrong/missing token (including status/SSE), Host/Origin/peer, null/wildcard Origin, query credentials, malformed/oversized/symlinked or unsafe discovery and discovery path swaps. Validate exact instance/workspace/protocol; disable proxy/redirect behavior even under hostile environment/redirect fixtures. Fake-clock tests cover 5-second heartbeat/30-second lease, same-session reconnect, expiry cleanup and `client_busy` without takeover. Command/permission/capability inventories agree and stay the same in both builds; main-window allowed, secondary-window/unregistered/generic invocation denied. Seeded secrets never reach WebView DTOs, logs or evidence. |
| S06 — Transport | Application-port tests: same request ID/input returns one admitted fake job even after busy state changes; changed input fails; retry after a lost admission reply returns that job. HTTP/SSE/channel tests: ordered session/job sequences, duplicate/stale-instance/session event suppression, state-before-notification and idempotent cancellation. Exercise encoded HTTP/SSE size limits, bounded replay ring/transient memory/job admission, control access during saturation, slow consumers, reconnect within ring, gap/restart resync even when the notification was lost, and full current job-state retrieval. Reserve future save capacity without implementing a save. Ordinary builds exclude fixture composition; neither build exposes a fake-admission endpoint/command. |
| S07 — Client | Vitest checks typed DTO validation, disconnected/reconnected reducers, stale/duplicate events, channel disposal and CodeMirror create/destroy cleanup on remount. Render canonical named regions and disabled capability explanations; sample editor changes stay in memory and are never reported saved. UI state/console excludes token/config/private paths. Minimal keyboard entry/focus is observed natively in S09; complete E9/E12/E13 acceptance remains later. |
| S08 — Versions | `package.json.version`, Cargo workspace/package inheritance and Tauri bundle literals agree; deliberate drift fails the check. UI metadata gives `Version v1.0.0`; status reports actual implemented protocol/config capabilities without inventing storage support. Compatible handshake succeeds; incompatible API major/minor capability fails before session/feature use. |
| S09 — Native/E1/E10 | Packaged procedure below passes on the recorded Apple Silicon/macOS/toolchain set. Rust/Vitest mocks alone cannot satisfy this row. |
| S10 — Docs | README/contribution instructions show actual independent dev/packaged commands, explicit config/discovery paths, synthetic setup, build/type/gate commands, fixture exclusions and implemented limitations. Evidence is indexed and all later capability gates remain assigned. Status/archive/link updates reflect actual completion. |

The [API bounds](../architecture/api-and-processes.md#errors-and-bounds) and [event contract](../architecture/api-and-processes.md#jobs-events-and-recovery) supply production limits; use configurable small test limits where useful, with assertions for the real defaults. Test failures and observed outcomes, not just constant declarations. Durable operation IDs/receipts, provider results and saved revisions are not simulated into capability claims.

## Packaged synthetic smoke procedure

1. Record revision, toolchain, OS/CPU, server executable path/identity, Tauri bundle ID and ordinary/fixture build mode. Create a private fixture config and separate workspace/runtime/source folders with invented sentinel content; keep token/discovery contents out of screenshots/logs.
2. Launch `diary-server --config PATH` in one shell, then the packaged `diary-client --connect PATH` in another. Record the bound loopback address and discovery path without the credential. Verify actual packaged Origin/CSP and an allowed typed main-window call; the frontend cannot contact HTTP/files/shell directly. Use the test window to verify a command denial without giving it extra capabilities.
3. Through fixture composition, run/cancel a fake job and observe an ordered status/event round trip. Edit the synthetic CodeMirror buffer, check basic keyboard/focus and close/reopen cleanup. Keep all content/save/agent/export controls unavailable; this is not the later AI responsiveness or IME qualification.
4. Have the packaged server adapter read a synthetic allowed sentinel and attempt a genuinely denied folder. Record the observed OS error/access context and confirm unavailable is not reported as empty success. Inspect server/client identities separately; a client-side successful read does not prove server access, and one folder does not prove iCloud/TCC support.
5. While the first client is connected, verify a competing client is rejected. Close the admitted client, verify the server remains available and admit a fresh session after release/expiry. Interrupt/restart the server, verify private discovery/lock cleanup or stale handling and rotated credentials, then reconnect without scanning ports. Check the ordinary build excludes fixture composition and both builds expose only the listed commands/permissions. Remove only test-owned files/processes and record each result or blocker.

Manual packaged checks with the approved Rust/Vitest tooling suffice for this milestone; any new native automation plugin/build integration needs its own concrete approval. Mocks, development-only origin tests and a generic filesystem unit test do not replace this procedure.

## Plan review record

2026-11-05: Expanded the earlier sequencing-only plan into S01–S10/G01–G06 acceptance, module and command inventories, fixture/build isolation, startup-versus-storage boundaries, negative/reconnect cases and a reproducible native procedure. Added actual frontend build/type checks alongside the unchanged six-command gate. These are planning corrections within approved architecture, not new product decisions, dependency approval or executed runtime evidence.

Documentation validation: 38 maintained/root/prompt-index Markdown files, 344 local links and 107 anchors passed, excluding fenced examples from link checks. Title/status, final newline/trailing whitespace, P1-M3 index coverage, checklist/matrix IDs and Planned/unchecked state passed; `git diff --check` passed. Removed the dangling index link to a nonexistent security-review document; no security-review completion is inferred.

Gate attempts remain blocked: G01–G03 cannot run because `src-tauri/Cargo.toml` does not exist. G04–G06 were attempted with `npx --offline --no-install` to prevent unapproved installation; Prettier, ESLint and Vitest are not installed/cached (`ENOTCACHED`). No formatter, lint, test, bundle or native result is claimed. All S/G completion items remain unchecked, and no dependency was installed.

## Handoff

E1/E10 and E11's profile-schema preparation in the [evidence register](implementation-handoff.md#capability-evidence-register) determine scaffold acceptance. [P1-M2](../archive/milestones/p1-m2/milestone-02-architecture-overview.md#architecture-clarifications-and-review) owns configuration/prompt clarifications, architecture review, interviews and background-AI design. This plan review clarifies assembly/evidence and does not reopen those decisions.

The [phase plan](phase-1.md#milestones) defines P1-M4–P1-M28 themes. Logging belongs to P1-M4; workspace/operations to P1-M5; durable Authentic and its editor/Create Empty workflow to P1-M6/P1-M7; settings to P1-M8; source/navigation/input features to P1-M9–P1-M15; agents/AI/review to P1-M16–P1-M24; exports to P1-M25–P1-M27; personal-release qualification to P1-M28. Create each feature milestone's overview/checklist and work plan before implementation; the [handoff](implementation-handoff.md#delivery-order-and-scope) maps contracts and evidence to these themes.
