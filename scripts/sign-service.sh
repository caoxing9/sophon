#!/bin/sh
# Signs the service inside a packaged Sophon.app (scripts/bundle-service.sh)
# with a Developer ID, before the app itself is signed: Apple notarizes an
# app only when every Mach-O file in it is signed, with the hardened
# runtime and a timestamp. `codesign --deep` on the app does not reach
# them: they are in Resources.
#
#   scripts/sign-service.sh path/to/Sophon.app "Developer ID Application: …"
set -eu
APP="${1:?usage: $0 path/to/Sophon.app identity}"
IDENTITY="${2:?usage: $0 path/to/Sophon.app identity}"
SERVICE="$APP/Contents/Resources/service"
[ -d "$SERVICE" ] || { echo "no service in $APP" >&2; exit 1; }
codesign --force --options runtime --timestamp --sign "$IDENTITY" "$SERVICE/sophon-service"
