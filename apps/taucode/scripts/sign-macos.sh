#!/bin/bash
set -euo pipefail
APP=${1:?Usage: sign-macos.sh app}
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
IDENTITY=${TAUCODE_SIGNING_IDENTITY:-taucode Dev}
STAMP=--timestamp=none
if [[ "$IDENTITY" == "Developer ID Application:"* ]]; then STAMP=--timestamp; fi
sign() {
  codesign --force --options runtime "$STAMP" --entitlements "$SCRIPT_DIR/../src-tauri/Entitlements.plist" --sign "$IDENTITY" "$1"
}
# Seal nested Mach-O files before their containing app; never sign with --deep.
while IFS= read -r -d '' binary; do
  if file -b "$binary" | grep -q 'Mach-O'; then sign "$binary"; fi
done < <(find "$APP/Contents/Resources" -type f -print0)
while IFS= read -r -d '' binary; do
  if file -b "$binary" | grep -q 'Mach-O'; then sign "$binary"; fi
done < <(find "$APP/Contents/MacOS" -type f -print0)
sign "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"
