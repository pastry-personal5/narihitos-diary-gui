#!/usr/bin/env bash
# Start the local diary server with config/global_config.yaml (separate from the GUI by design).
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
export DIARY_LOG=debug
exec make server
