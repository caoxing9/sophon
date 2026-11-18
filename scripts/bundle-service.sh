#!/bin/sh
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="${1:?usage: $0 path/to/Sophon.app}"
[ -d "$APP/Contents" ] || { echo "$APP is not an app bundle" >&2; exit 1; }
DEST="$APP/Contents/Resources/service"
BUILD="$ROOT/target/service-bundle"
cargo build --locked --release --manifest-path "$ROOT/Cargo.toml" --target-dir "$BUILD" -p sophon-service
mkdir -p "$DEST"
install -m 755 "$BUILD/release/sophon-service" "$DEST/sophon-service"
rm -rf "$DEST/node"
