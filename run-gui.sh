#!/usr/bin/env bash
# Start the GUI against an already running server. Optional arg: discovery.json path.
set -euo pipefail

export RUST_BACKTRACE=1

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
CONFIG=config/global_config.yaml

if [[ $# -gt 0 ]]; then
  DISCOVERY=$1
else
  # The server writes discovery.json under paths.runtime_root from the config.
  RUNTIME_ROOT=$(sed -n 's/^[[:space:]]*runtime_root:[[:space:]]*"\{0,1\}\([^"#]*[^"# ]\)"\{0,1\}.*/\1/p' "${CONFIG}" | head -n 1)
  [[ -n "${RUNTIME_ROOT}" ]] || { printf 'No paths.runtime_root in %s\n' "${CONFIG}" >&2; exit 1; }
  # Newest first: a stale file from an earlier workspace must not win.
  DISCOVERY=$(find "${RUNTIME_ROOT}" -type f -name discovery.json -exec ls -t {} + 2>/dev/null | head -n 1 || true)
fi

if [[ -z "${DISCOVERY}" || ! -f "${DISCOVERY}" ]]; then
  printf 'No discovery file found (looked under %s)\n' "${RUNTIME_ROOT:-the given path}" >&2
  printf 'Start the server first (./run-server.sh), then run this script again.\n' >&2
  exit 1
fi

# The client rejects symlinked path components, so pass the canonical path.
DISCOVERY=$(realpath "${DISCOVERY}")
export DIARY_LOG=debug # connection-stage records; read with: diary-server trace --select debug:50

# Job control gives this launch its own process group, including npm, Tauri,
# Cargo and the frontend server. Never match process names or kill the server.
set -m
GUI_PID=
cleanup() {
  local status=$?
  trap - EXIT INT TERM HUP
  if [[ -n "${GUI_PID}" ]]; then
    kill -TERM -- "-${GUI_PID}" 2>/dev/null || true
    # Give normal handlers time to drain; escalate only within our own group.
    for ((attempt = 0; attempt < 40; attempt++)); do
      kill -0 -- "-${GUI_PID}" 2>/dev/null || break
      sleep 0.05
    done
    kill -KILL -- "-${GUI_PID}" 2>/dev/null || true
    wait "${GUI_PID}" 2>/dev/null || true
  fi
  exit "${status}"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
npm run tauri -- dev --exit-on-panic -- -- --connect "${DISCOVERY}" &
GUI_PID=$!
wait "${GUI_PID}"
