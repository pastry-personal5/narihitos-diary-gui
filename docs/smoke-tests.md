# Smoke tests and tools

Status: Active

Built-binary smoke scripts and the redacted log trace command. Run from the repository root; live scripts spend account usage and are env-gated.

The [Illustrated Notion qualification record](archive/milestones/p1-m36/milestone-36-illustrated-notion-upload-architecture.md#qualification-execution-2026-11-13)
documents the authorized test-root-only page-position and image-read probes.
`DIARY_NOTION_LIVE=1 python3 scripts/smoke/notion-live.py --qualify-pages --config config/global_config.yaml`
creates retained fixture pages using the privately configured token, without
starting the server or using production destination roots. `--prepare-images ID`
uploads synthetic JPEG fixtures to a direct child of the authorized root;
`--observe-images ID` only reads. Neither mode deletes or archives pages. Manual
Notion image edits are required between preparation and observation. Do not rerun
preparation after an uncertain write. Offline probe safety tests:
`python3 -m unittest discover -s scripts/tests -p 'test_notion_qualification.py'`.
`--probe-rapid-image-edit ID` creates a separate blue fixture and replaces it with
red pixels to test same-minute drift detection; exit 1 means the proposed metadata
evidence missed the replacement. `--qualify-image-bytes ID` only reads hosted
fixture bytes with bounded, credential-free requests and compares hashes with
synthetic JPEGs. Both modes retain all pages and leave the manually edited fixtures
intact. The [counterexample](archive/milestones/p1-m36/milestone-36-illustrated-notion-upload-architecture.md#image-drift-counterexample-and-proposed-resolution)
records the observed service limitation and pending design review.

Illustrated upload acceptance:

```sh
python3 scripts/smoke/illustrated-notion.py # isolated persistent fake Notion; no accounts
DIARY_NOTION_LIVE=1 python3 scripts/smoke/illustrated-notion.py --live # fixture writes only beneath the dedicated authorized test root
python3 scripts/smoke/illustrated-notion.py --gui-app "src-tauri/target/debug/bundle/macos/Narihito's Diary.app" # scaffold-smoke bundle; isolated fake destination
```

The upload harness starts an empty private workspace with no AI profile/engine and
seeds only its own published fixture entries. It verifies first/middle/last
backdated insertion at year/month/day levels, unchanged upload, stable day page
replacement, restart, original-file hashes and terminal JPEG-row cleanup. Live
mode reads only the configured token, verifies the fixed authorized test root and
creates a fresh retained child; it never substitutes a production root. Native mode
is synthetic only and verifies the accessible toolbar bitmap/action, pending and
missing-entry guards, no Authentic/viewer/AI prerequisite, and navigation preserved
through completion. The [M36 acceptance record](archive/milestones/p1-m36/milestone-36-illustrated-notion-upload-architecture.md#implementation-acceptance)
records retained live pages and ordinary artifact validation.

`make smoke` rebuilds the scaffold fixture server and packaged client, then runs
the native IPC/session and keyboard checks in an isolated workspace. It requires
a macOS GUI session and System Events accessibility permission. The fixture waits
for automatic connection, the enabled Create Empty action and an editable
Authentic buffer before focusing the editor. The harness sends native keys only
after readiness is observed; readiness failures include editor diagnostics.

```sh
python3 scripts/tests/test_run_gui.py # Synthetic launcher exit/failure/signal/TERM-resistant descendant regressions
python3 scripts/smoke/gui-close.py # Ordinary run-gui.sh native window-close proof; built diary-server, free port 1420, macOS GUI/System Events accessibility required
python3 scripts/smoke/producer.py # Schema 1 producer-codec save/reopen matrix using the reference submodule
python3 scripts/smoke/producer.py --schema2 # Captured mobile codec: schema 1 + text-only schema 2 import/save/producer-edit/read-back and conflict checks
python3 scripts/smoke/imports.py                                                                                   # Civil-date import/resolution/restart smoke on the real server binary
python3 scripts/smoke/trash.py --enabled-server                                                                  # Synthetic native Trash faults + pinned producer catalog + real-server integration
python3 scripts/smoke/agent.py                                                                                     # Built-binary prompt and Apple helper embedding + profile status and chat open/close refusals with fake CLIs
DIARY_GENERATION_LIVE=codex DIARY_GENERATION_LIVE_HOME=/absolute/profile python3 scripts/smoke/generation-live.py # real candidate + cancel; claude/apple also supported; Qwen exempt
python3 scripts/smoke/authentic.py                                                                                 # Authentic create/save/restart/kill-recovery smoke on the real server binary
python3 scripts/smoke/review.py                                                                                    # Paragraph review: start/reject/accept/invalidate/kill-recovery on a fault-injection server binary
python3 scripts/smoke/export.py # Export coordination: create/no-op/confirmation/duplicate/kill-recovery on the fault-injection server with the directory-backed fake destination
cargo run --manifest-path src-tauri/Cargo.toml -p diary-server -- trace --config PATH --select recent:20        # redacted debugging trace of the logs
```

GPU-free server image shutdown proof:

```sh
python3 -m unittest scripts/tests/test_image_shutdown.py
```

This builds the ordinary server and existing fake AI CLI, then uses isolated
workspaces and synthetic image programs to verify SIGTERM/SIGINT during rendering
and readiness probes, TERM handlers, bounded KILL escalation for stubborn
descendants, reload signal retention and completed-photo staging cleanup.
Assertions include server exit, child exit, discovery/staging removal, no partial
Illustrated publication and unrelated sentinel survival. It requires localhost
and process visibility; no real GPU, account or ordinary diary configuration is
used.

Real image-engine proof (requires the installed Krea 2 runtime, cached weights,
Metal GPU access and process visibility; no downloads):

```sh
python3 scripts/smoke/image-engines.py # GPU-free wrapper contract
DIARY_IMAGE_LIVE=1 python3 scripts/smoke/image-generation-live.py
```

The live harness runs outside the checkout working directory and retains private
`report.json`, `adapter.json`, `gpu-samples.json`, build/run logs and `image.png`
under a new `tmp/krea GPU evidence *` directory. It exercises the application
registry, production wrapper and PNG validator, followed by a distinct in-flight
cancel, process cleanup, staging disposal and unrelated process survival. Without
`DIARY_IMAGE_LIVE=1` it skips; failures exit nonzero and retain evidence. This is
standalone engine proof, with no diary generation or GUI coverage.

The GPU-free `scripts/tests/test_image_generation_live.py` regressions build the
Rust example with the existing Cargo toolchain. They require process visibility
but no GPU: an injected monitor failure must cancel the separately supervised
fake renderer, dispose staging and preserve an unrelated sentinel. The live
harness retains failure causes and requests cooperative cancellation on
interruption; its report also checks that provenance did not change mid-run.

`python3 scripts/smoke/illustrated.py` builds an isolated ordinary server plus synthetic Codex/image commands and verifies create/read/photo, request replay, failure/cancel preservation, silent replacement, version checks and confirmed deletion without touching ordinary data. `DIARY_ILLUSTRATED_LIVE=1 python3 scripts/smoke/illustrated.py --live` is the owner-authorized isolated xpp8 + Krea 2 proof: it reads the selected local config settings, uses the actual repository `.sh` wrapper in the generated test config and retains the workspace/evidence under `tmp/`. It never edits the private ordinary config. The live lane uses an existing account and local installed weights; it is not part of the default gate.

## Illustrated GUI

`python3 scripts/smoke/illustrated.py` runs the isolated ordinary-server/fake-agent/fake-engine API sequence, including agent/render cancellation, render failure, silent replacement, stale photo reads and confirmed deletion. It never reads private configuration unless explicitly invoked with the separately authorized live mode.

For packaged WKWebView evidence, first build the fixture app with `npm run tauri build -- --debug --bundles app --features scaffold-smoke`, then run:

```sh
python3 scripts/smoke/illustrated.py --gui-app "src-tauri/target/debug/bundle/macos/Narihito's Diary.app"
```

This adds native UI create/read/replace/close/reopen/delete, decoded blob PNGs/alt text, source-change Global Messages, failed-image Retry and remote-image CSP refusal. The app settings, workspace and server are isolated under the retained evidence directory; only owned processes are stopped. It requires a macOS GUI session and loopback permission. A timeout or failed assertion is nonzero. Fixture-only eval code adds no command or ordinary-app automation capability. Rebuild ordinary artifacts after the fixture run and verify fixture exclusion before release qualification. Route tests and jsdom alone do not establish WKWebView image rendering.
