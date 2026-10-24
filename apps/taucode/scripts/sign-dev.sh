#!/bin/sh
# Sign a local taucode build with a stable self-signed identity so macOS keeps
# Screen Recording / Accessibility grants across rebuilds (ad-hoc signatures change
# every build, which makes macOS treat each build as a new app).
#
# One-time setup: Keychain Access → Certificate Assistant → Create a Certificate…,
# Name "taucode Dev", Identity Type "Self-Signed Root", Certificate Type "Code Signing".
set -eu
IDENTITY="${TAUCODE_SIGNING_IDENTITY:-taucode Dev}"
APP="${1:-$(dirname "$0")/../src-tauri/target/debug/bundle/macos/taucode.app}"
codesign --force --deep --timestamp=none --sign "$IDENTITY" --identifier dev.taucode.desktop "$APP"
codesign -d -r- "$APP" 2>&1 | grep designated
