#!/bin/sh
# Builds Sophon for the iPhone plugged into this Mac, signs it with your Apple ID
# (a free Personal Team is enough) and installs it.
#
# Free signing lasts 7 days: run this again when Sophon stops opening.
# Needs: Xcode signed in to your Apple ID (Settings → Accounts), the iPhone
# connected and unlocked, Developer Mode on (Settings → Privacy & Security).
#
#   scripts/ios-device.sh             # the first connected iPhone, your Personal Team
#   SOPHON_TEAM=XXXXXXXXXX scripts/ios-device.sh
#   SOPHON_DEVICE=<udid> scripts/ios-device.sh
set -eu
cd "$(dirname "$0")/.."

BUNDLE_ID=$(sed -n 's/.*"id": *"\([^"]*\)".*/\1/p' nsh.json | head -1)
# The nsh CLI of the commit Cargo.toml pins (a released one can be older).
if [ -z "${NSH:-}" ]; then
  if [ -x ref/nsh/target/release/nsh ]; then NSH=ref/nsh/target/release/nsh; else NSH="$(scripts/install-nsh.sh)/nsh"; fi
fi

# The device.
DEVICE=${SOPHON_DEVICE:-$(xcrun devicectl list devices 2>/dev/null |
  awk '/physical/ && /iPhone|iPad/ && /connected|available/ { for (i = 1; i <= NF; i++) if ($i ~ /^[0-9A-F]{8}-[0-9A-F]{16}$/) { print $i; exit } }')}
[ -n "$DEVICE" ] || { echo "No iPhone connected: plug it in and unlock it." >&2; exit 1; }

# The team: the free Personal Team of the Apple ID signed in to Xcode.
TEAM=${SOPHON_TEAM:-$(defaults read com.apple.dt.Xcode IDEProvisioningTeamByIdentifier 2>/dev/null |
  awk '/isFreeProvisioningTeam = 1/ { free = 1 } free && /teamID/ { gsub(/[ ;]/, ""); sub(/teamID=/, ""); print; exit }')}
[ -n "$TEAM" ] || { echo "No team: sign in to Xcode → Settings → Accounts, or set SOPHON_TEAM." >&2; exit 1; }
echo "Device $DEVICE, team $TEAM, $BUNDLE_ID"

# A development certificate and a profile for this app and device, which
# Xcode makes (and renews) by building a placeholder app with the same id.
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
xcodebuild -quiet -project scripts/ios-signer/Signer.xcodeproj -scheme Signer \
  -destination "id=$DEVICE" -derivedDataPath "$WORK" \
  -allowProvisioningUpdates -allowProvisioningDeviceRegistration \
  DEVELOPMENT_TEAM="$TEAM" PRODUCT_BUNDLE_IDENTIFIER="$BUNDLE_ID" build
APP="$WORK/Build/Products/Debug-iphoneos/Signer.app"
PROFILE="$WORK/embedded.mobileprovision"
cp "$APP/embedded.mobileprovision" "$PROFILE"
IDENTITY=$(codesign -dvv "$APP" 2>&1 | sed -n 's/^Authority=\(Apple Development: .*\)/\1/p' | head -1)
echo "Signing as $IDENTITY"

"$NSH" package --target ios --icon assets/app-icon/icon-ios-1024.png --sign "$IDENTITY" --profile "$PROFILE"
xcrun devicectl device install app --device "$DEVICE" .nsh/out/Sophon.app
echo "Installed. First time: on the iPhone, Settings → General → VPN & Device Management → trust your Apple ID."
xcrun devicectl device process launch --device "$DEVICE" "$BUNDLE_ID" >/dev/null 2>&1 && echo "Launched." || true
