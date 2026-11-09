#!/bin/sh
# Installs Sophon on macOS from its GitHub Releases:
#
#   curl -fsSL https://raw.githubusercontent.com/caoxing9/sophon/main/install.sh | sh
#
# Options (environment variables):
#   SOPHON_VERSION=0.1.0   a given version instead of the latest
#   SOPHON_DIR=~/Applications   where the app goes (default /Applications,
#                               or ~/Applications when that is not writable)
#   SOPHON_NO_OPEN=1       do not launch it afterwards
set -eu

REPO="caoxing9/sophon"
APP="Sophon"

say() { printf '\033[1m%s\033[0m\n' "$*"; }
fail() { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || fail "Sophon runs on macOS."
case "$(uname -m)" in
  arm64) ARCH="aarch64" ;;
  x86_64) fail "Sophon runs on Apple silicon Macs only." ;;
  *) fail "unsupported CPU: $(uname -m)" ;;
esac

if [ -n "${SOPHON_VERSION:-}" ]; then
  VERSION="${SOPHON_VERSION#v}"
  BASE="https://github.com/$REPO/releases/download/v$VERSION"
else
  BASE="https://github.com/$REPO/releases/latest/download"
  VERSION="$(curl -fsSL "$BASE/latest.json" | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
  [ -n "$VERSION" ] || fail "cannot read the latest version from $BASE/latest.json"
fi

ARCHIVE="$APP-$VERSION-macos-$ARCH.tar.gz"
DIR="${SOPHON_DIR:-/Applications}"
if [ ! -w "$DIR" ]; then
  DIR="$HOME/Applications"
  mkdir -p "$DIR"
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

say "Downloading $APP $VERSION ($ARCH)…"
curl -fL --progress-bar "$BASE/$ARCHIVE" -o "$TMP/$ARCHIVE" || fail "no build of $APP $VERSION for $ARCH"
tar -xzf "$TMP/$ARCHIVE" -C "$TMP"
[ -d "$TMP/$APP.app" ] || fail "the archive holds no $APP.app"

if pgrep -xq "$APP"; then
  say "Quitting the running $APP…"
  osascript -e "quit app \"$APP\"" >/dev/null 2>&1 || true
  sleep 1
fi

rm -rf "$DIR/$APP.app"
mv "$TMP/$APP.app" "$DIR/$APP.app"
# Not notarized yet: drop the download's quarantine so Gatekeeper lets it open.
xattr -dr com.apple.quarantine "$DIR/$APP.app" 2>/dev/null || true

say "Installed $APP $VERSION in $DIR. It updates itself from now on."
[ -n "${SOPHON_NO_OPEN:-}" ] || open "$DIR/$APP.app"
