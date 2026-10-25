#!/bin/bash
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
cd "$SCRIPT_DIR/.."
export CARGO_NET_OFFLINE=true
IDENTITY=${TAUCODE_SIGNING_IDENTITY:-taucode Dev}
DEVELOPER_ID=false
if [[ "$IDENTITY" == "Developer ID Application:"* ]]; then DEVELOPER_ID=true; fi
if [[ "$IDENTITY" != 'taucode Dev' && "$DEVELOPER_ID" == false ]]; then
  echo 'TAUCODE_SIGNING_IDENTITY must be a Developer ID Application identity or taucode Dev' >&2
  exit 1
fi
if [[ -n "${TAUCODE_NOTARY_PROFILE:-}" && "$DEVELOPER_ID" == false ]]; then
  echo 'Notarization requires a Developer ID Application identity' >&2
  exit 1
fi
TARGET=${CARGO_TARGET_DIR:-target}
if [[ "$TARGET" != /* ]]; then TARGET="$PWD/src-tauri/$TARGET"; fi
mkdir -p "$TARGET/release"
MARKER=$(mktemp "$TARGET/release/.release-start.XXXXXX")
STAGING=""
trap 'rm -f "$MARKER"; if [[ -n "$STAGING" ]]; then rm -rf "$STAGING"; fi' EXIT
# Keep signing/notarization in this script; Tauri must not import certificates or
# consume password/API-key credentials inherited from the caller's environment.
env -u APPLE_ID -u APPLE_PASSWORD -u APPLE_API_KEY -u APPLE_API_KEY_PATH \
  -u APPLE_API_ISSUER -u APPLE_CERTIFICATE -u APPLE_CERTIFICATE_PASSWORD \
  -u APPLE_TEAM_ID APPLE_SIGNING_IDENTITY=- \
  ./node_modules/.bin/tauri build --bundles app,dmg
APP="$TARGET/release/bundle/macos/taucode.app"
bash "$SCRIPT_DIR/sign-macos.sh" "$APP"
node "$SCRIPT_DIR/check-bundle.mjs" "$APP"
GATEKEEPER_OK=true
if ! spctl -a -vv "$APP"; then
  GATEKEEPER_OK=false
  if [[ "$DEVELOPER_ID" == false ]]; then
    echo 'Local identity: Gatekeeper rejection is expected.'
  else
    echo 'Gatekeeper has not accepted this release; checking again after notarization.'
  fi
fi
# Recreate the DMG from the signed app, not Tauri's pre-signing snapshot.
DMG=""
while IFS= read -r -d '' candidate; do
  if [[ -n "$DMG" ]]; then echo 'Expected one newly built DMG' >&2; exit 1; fi
  DMG=$candidate
done < <(find "$TARGET/release/bundle/dmg" -name '*.dmg' -type f -newer "$MARKER" -print0)
if [[ -z "$DMG" ]]; then echo 'Tauri did not produce a DMG' >&2; exit 1; fi
STAGING=$(mktemp -d "$TARGET/release/dmg-stage.XXXXXX")
ditto "$APP" "$STAGING/taucode.app"
ln -s /Applications "$STAGING/Applications"
hdiutil create -ov -format UDZO -volname taucode -srcfolder "$STAGING" "$DMG"
if [[ "$DEVELOPER_ID" == true ]]; then
  codesign --force --timestamp --sign "$IDENTITY" "$DMG"
else
  codesign --force --timestamp=none --sign "$IDENTITY" "$DMG"
fi
codesign --verify --strict "$DMG"
if [[ -n "${TAUCODE_NOTARY_PROFILE:-}" ]]; then
  xcrun notarytool submit "$DMG" --keychain-profile "$TAUCODE_NOTARY_PROFILE" --wait
  xcrun stapler staple "$DMG"
  xcrun stapler staple "$APP"
  xcrun stapler validate "$DMG"
  spctl -a -vv "$APP"
elif [[ "$DEVELOPER_ID" == true ]]; then
  echo 'Signed but not notarized: set TAUCODE_NOTARY_PROFILE for distribution.'
fi
node "$SCRIPT_DIR/release-manifest.mjs" "$APP" "$DMG"
if [[ "$DEVELOPER_ID" == true && "$GATEKEEPER_OK" == false && -z "${TAUCODE_NOTARY_PROFILE:-}" ]]; then
  echo 'Gatekeeper rejected the Developer ID build. Configure notarization before distributing it.' >&2
  exit 1
fi
echo "Release: $DMG"
