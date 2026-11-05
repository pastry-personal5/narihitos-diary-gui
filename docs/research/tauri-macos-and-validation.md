# Tauri macOS and validation findings

Status: Active

Research date: 2026-11-05. Covers R4.1–R4.6 and R14.1–R14.3, with platform checks explicitly separated from documentation findings. This is a feasibility and validation handoff; no app scaffold, signing setup, or new testing dependency is installed.

## Environment and prerequisites (R4.4)

Read-only local commands observed macOS `26.7.1` build `25G241`, Xcode selected at `/Applications/Xcode.app/Contents/Developer`, Rust `1.98.0`, Node `26.8.1`, and Git `2.54.0`. These are the current research machine's tools, not approved minimum versions or proof of Node LTS compatibility. `xcodebuild -version` additionally confirmed Xcode `27.2` build `27B52`. A CPU/memory query was denied by the sandbox, so hardware capacity was not independently established. No app build was run.

[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) allow Xcode Command Line Tools for desktop-only development. The [macOS bundle flow](https://v2.tauri.app/distribute/macos-application-bundle/) produces a macOS app bundle; a generated iOS Xcode project is not a prerequisite for this desktop app. P1-M3 should verify the actual selected toolchain with a minimal desktop build/run before claiming support.

## Filesystem permissions and trust (R4.1)

Three independent mechanisms need evaluation: Tauri IPC capabilities control frontend access to commands/plugins; macOS filesystem/TCC permissions affect process access; App Sandbox entitlements constrain a sandboxed distribution. A wide Tauri filesystem grant does not solve OS access, and Rust's direct filesystem calls do not inherit a plugin's path checks automatically.

[Tauri capabilities](https://v2.tauri.app/security/capabilities/) combine permissions for matching windows/WebViews; multiple matching capabilities expand the effective grants. Recommend an explicit local main-window capability set, no remote WebView privileges, and backend-owned path validation. Avoid generic frontend filesystem/shell/SQL commands when task-specific commands suffice. Keep source, workspace, export, profile, and executable roots distinct. Validate canonical containment and freshness at use time, including symlinks and overlapping roots.

The same reference explicitly says app commands registered with `invoke_handler` are available to all app windows/WebViews by default. Restricting them needs an application command manifest/permissions, not merely omitting filesystem-plugin permissions. P1-M2 must specify that command allowlist and argument validation; P1-M3 must verify a denied command/window case.

Compare a user-selected-folder flow with configured paths. Persistent access in a sandboxed build may require bookmarks/entitlements; an unsandboxed Developer ID app has a different access model. TCC prompt behavior for the actual iCloud paths and packaged identity remains unverified. Apple documentation pages fetched during research returned JavaScript stubs and Markdown-fetch errors, so they are not treated as evidence for exact entitlements or prompt guarantees. R14 must validate the chosen distribution/access model before P1-M2 locks it in.

## Commands, streaming, and processes (R4.2–R4.3/R4.6)

[Tauri commands](https://v2.tauri.app/develop/calling-rust/) support typed request/response boundaries. Recommend opaque asset/profile/job IDs and typed errors instead of frontend-supplied arbitrary paths or command lines. Long work returns/uses a cancellable job handle; Rust owns durable status. Reconnecting a view must be able to query status rather than depend on receiving every transient event.

[Tauri frontend communication](https://v2.tauri.app/develop/calling-frontend/) distinguishes lightweight events from ordered channels intended for streaming. Evaluate channels for agent deltas/progress; reserve events for coarse notifications. Include run IDs and sequence numbers; bound queues/message sizes, batch UI updates, and discard stale events from cancelled/superseded runs. Channels are not a substitute for application authorization or durable job state.

Compare Rust process supervision with a frontend shell plugin. Prefer Rust ownership to keep executable/profile selection, stdin prompts, environment filtering, and output parsing in one trusted boundary. `kill` of a direct child is not proof that descendants exit: process-group management, escalation/reaping, app-close cleanup, and hung pipes need a synthetic child/grandchild fixture. Drain stdout and stderr concurrently with bounded buffers; a process can block if one pipe fills. Treat stream text as untrusted and redact diagnostics.

Finder launch cannot be assumed to have a terminal's initialized PATH. The prior art already supports an explicit Codex executable path; generalize the option for all three agents rather than invoking a login shell or shell command string. Validate configured binaries and args; stream prompts on stdin. A minimal environment still needs provider/auth/runtime prerequisites; stripping everything or passing the parent's full environment are both poor untested defaults. No agent binary bundling is required by the owner's installed-profile workflow.

## WKWebView and editor risks (R4.5)

The deployed app uses the system WebView, so Node/Vitest success does not establish browser feature or IME correctness. Test the actual supported macOS versions for CodeMirror composition, clipboard, selection, keyboard shortcuts, focus restoration after tab switches, long-document scrolling, and pane resize. Feature-detect Temporal before use; canonical timezone logic can stay in Rust. Set a local-content CSP and avoid raw imported HTML injection; Markdown source editing needs no preview renderer or remote assets.

## Validation mapping (R14.1–R14.2)

| Risk                             | Appropriate evidence                                                                                                                            |
| -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Parser/identity/time selection   | Synthetic Rust unit fixtures, including malformed inputs and full date inclusion sets                                                           |
| Autosave/recovery/external edits | Temporary-workspace integration tests, controlled interruption at each durable boundary, stale-revision races                                   |
| Export retries and ordering      | Fake HTTP service for pagination, limits, lost replies, remote drift; optional synthetic-account smoke test later                               |
| Agent adapters                   | Fake executables emitting split/malformed/oversized JSON, stderr floods, early exit, timeout, and descendants; separately verify real CLI flags |
| UI behavior                      | Vitest tests for transition effects, keyboard/selection independence, stale job events, save acknowledgements, and React editor cleanup         |
| Native permissions/runtime       | Packaged Finder launch, real WKWebView, iCloud availability/permission denial, missing CLI, IME and VoiceOver checks                            |

The current [Tauri WebDriver docs](https://v2.tauri.app/develop/tests/webdriver/) describe an embedded WebdriverIO service route supporting macOS, while direct `tauri-driver` still lacks a native macOS WKWebView driver. Do not repeat the outdated blanket assertion that all Tauri WebDriver automation is unavailable on macOS. Embedded automation adds development plugins and must be evaluated/approved separately, excluded from production builds. For initial P1-M3 evidence, manual packaged smoke checks plus existing Rust/Vitest tests avoid deciding a new dependency now.

P1-M3's scaffold gate remains all six repository commands; the P1-M1/P1-M2 docs exemption does not extend to it. Check runtime behavior separately: green mocked tests do not establish filesystem permissions, streaming confinement, signing, or native focus/IME support.

## Delivery alternatives (R14.3)

Compare personal local installation with Developer ID distribution and a sandboxed Mac App Store release. [Tauri signing guidance](https://v2.tauri.app/distribute/sign/macos/) provides the signing route; distribution criteria, certificates, entitlements, notarization, quarantine/Gatekeeper handling, minimum macOS, and CPU architectures must be selected and validated as a release milestone. Existing Apple Silicon hardware does not imply an Intel support promise. Running arbitrary installed agent CLIs is a material compatibility question for a sandboxed distribution.

Bounded feasibility check proposed before the dependent architecture decision: one minimal packaged desktop app reads an owner-selected synthetic folder and starts a synthetic CLI from Finder, with cancel/exit cleanup and denied-path cases. Use temporary data only; record build identity, OS, entitlements, commands, and expected/observed results. Destination is an explicitly planned feasibility task before P1-M2 finalizes distribution/process access, or a declared blocker on those decisions. This research has not run that check.

P1-M2 implications: choose the distribution/access model, backend-owned command surface, process supervision, and ordered streaming contract. R14's release requirements and packaged iCloud/CLI feasibility remain open evidence; neither a successful source read nor documentation establishes them.
