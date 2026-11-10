# Smoke tests and tools

Status: Active

Built-binary smoke scripts and the redacted log trace command. Run from the repository root; live scripts spend account usage and are env-gated.

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
