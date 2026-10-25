#!/bin/bash
# Use an existing stable local identity; never create certificates here.
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
exec bash "$SCRIPT_DIR/sign-macos.sh" "${1:-$SCRIPT_DIR/../src-tauri/target/debug/bundle/macos/taucode.app}"
