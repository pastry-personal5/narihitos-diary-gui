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
exec npm run tauri dev -- -- -- --connect "${DISCOVERY}"
