#!/bin/sh
# Fixed command-engine entry point; the prompt is never evaluated by this shell.
set -eu
umask 077
SCRIPT_DIR=$(CDPATH= cd -- "$(/usr/bin/dirname -- "$0")" && pwd)
exec /usr/bin/python3 "$SCRIPT_DIR/krea-image-engine.py" "$@"
