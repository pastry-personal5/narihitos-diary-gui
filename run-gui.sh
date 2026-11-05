#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
RUNTIME_ROOT="${SCRIPT_DIR}/runtime_root"
DISCOVERY=$(find "${RUNTIME_ROOT}" -type f -name discovery.json -print -quit 2>/dev/null || true)

if [[ -z "${DISCOVERY}" ]]; then
  printf 'No discovery file found under %s\n' "${RUNTIME_ROOT}" >&2
  printf 'Start the local diary server first, then run this script again.\n' >&2
  exit 1
fi

npm run tauri dev -- -- -- --connect "${DISCOVERY}"
