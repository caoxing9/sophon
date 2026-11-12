#!/bin/sh
# Signs the service inside a packaged Sophon.app (scripts/bundle-service.sh)
# with a Developer ID, before the app itself is signed: Apple notarizes an
# app only when every Mach-O file in it is signed, with the hardened
# runtime and a timestamp. `codesign --deep` on the app does not reach
# them: they are in Resources.
#
#   scripts/sign-service.sh path/to/Sophon.app "Developer ID Application: …"
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
APP="${1:?usage: $0 path/to/Sophon.app identity}"
IDENTITY="${2:?usage: $0 path/to/Sophon.app identity}"
SERVICE="$APP/Contents/Resources/service"
[ -d "$SERVICE" ] || { echo "no service in $APP" >&2; exit 1; }
COUNT=0
# Node last: it is what the others are loaded by.
find "$SERVICE" -type f ! -path "$SERVICE/node/bin/node" | while IFS= read -r file; do
  if file -b "$file" | grep -q "Mach-O"; then
    codesign --force --options runtime --timestamp --sign "$IDENTITY" "$file"
  fi
done
codesign --force --options runtime --timestamp --entitlements "$HERE/service.entitlements" --sign "$IDENTITY" "$SERVICE/node/bin/node"
COUNT=$(find "$SERVICE" -type f -exec sh -c 'file -b "$1" | grep -q "Mach-O" && echo x' _ {} \; | wc -l | tr -d ' ')
echo "signed $COUNT files of the service"
