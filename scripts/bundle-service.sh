#!/bin/sh
# Puts the agent service inside a packaged Sophon.app, so a Mac with only
# Sophon runs its agents: Sophon is the window, the agents run in a service
# (Paseo's daemon, `@getpaseo/cli`, Apache-2.0), which the app starts from
# `Sophon.app/Contents/Resources/service` (`store::service_cli`).
#
#   scripts/bundle-service.sh path/to/Sophon.app
#
# What goes in, under `service/node`:
#   bin/node    a Node runtime of its own (nodejs.org, checked by its SHA-256)
#   bin/paseo   the service's command line (npm, into the same prefix)
# and what comes out again, over half of the 730 MB, which the service never
# runs: Node's headers and npm, the copy of Claude Code the agent SDK
# carries (the service runs the user's own), the speech library (Sophon
# uses the system's), builds for other systems, and source maps, type
# declarations and documents.
#
# Built once per pair of versions into `.nsh/service-<node>-<service>` and
# copied from there. Signing is the caller's: every Mach-O file in it has
# to be signed before the app is (`scripts/sign-service.sh`).
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="${1:?usage: $0 path/to/Sophon.app}"
[ -d "$APP/Contents" ] || { echo "$APP is not an app bundle" >&2; exit 1; }

# The Node runtime, and the service's version: the one Sophon is built against.
NODE=v22.23.5
SERVICE=0.11.4
case "$(uname -m)" in arm64) ARCH=arm64 ;; *) ARCH=x64 ;; esac

BUILT="$ROOT/.nsh/service-$NODE-$SERVICE-$ARCH"
if [ ! -x "$BUILT/node/bin/paseo" ]; then
  WORK="$BUILT.making"
  rm -rf "$WORK" "$BUILT"
  mkdir -p "$WORK/node"
  FILE="node-$NODE-darwin-$ARCH.tar.gz"
  BASE="https://nodejs.org/dist/$NODE"
  curl -fsSL --retry 3 -o "$WORK/$FILE" "$BASE/$FILE"
  WANT="$(curl -fsSL --retry 3 "$BASE/SHASUMS256.txt" | awk -v f="$FILE" '$2 == f { print $1 }')"
  GOT="$(shasum -a 256 "$WORK/$FILE" | awk '{ print $1 }')"
  [ -n "$WANT" ] && [ "$WANT" = "$GOT" ] || { echo "Node's download does not match its checksum" >&2; exit 1; }
  tar -xzf "$WORK/$FILE" -C "$WORK/node" --strip-components 1
  rm "$WORK/$FILE"
  # npm is a script run by the Node beside it: that folder first on the PATH.
  (cd "$WORK/node" && PATH="$WORK/node/bin:/usr/bin:/bin:/usr/sbin:/sbin" bin/npm install -g \
    --no-audit --no-fund --loglevel error --prefix "$WORK/node" "@getpaseo/cli@$SERVICE")
  [ -e "$WORK/node/bin/paseo" ] || { echo "the service's package installed no command line" >&2; exit 1; }

  # What the service never runs.
  N="$WORK/node"
  rm -rf "$N/include" "$N/share" "$N/lib/node_modules/npm" "$N/lib/node_modules/corepack" \
    "$N/bin/npm" "$N/bin/npx" "$N/bin/corepack" "$N/CHANGELOG.md" "$N/README.md"
  M="$N/lib/node_modules/@getpaseo/cli/node_modules"
  rm -rf "$M"/@anthropic-ai/claude-agent-sdk-* "$M"/sherpa-onnx-*
  # node-pty's builds for other systems and processors.
  for built in "$M"/node-pty/prebuilds/*; do
    [ "$(basename "$built")" = "darwin-$ARCH" ] || rm -rf "$built"
  done
  # What only people read or compilers check: source maps, type
  # declarations, documents. Nothing loads them when the service runs,
  # but for the service's own packages: it builds its built-in plugins
  # from TypeScript as it starts, and those look their types up.
  find "$M" -type f \( -name "*.map" -o -name "*.d.ts" -o -name "*.d.mts" -o -name "*.d.cts" -o -name "*.md" -o -name "*.markdown" \) \
    -not -path "$M/@getpaseo/*" -delete
  mv "$WORK" "$BUILT"
fi

DEST="$APP/Contents/Resources/service"
rm -rf "$DEST"
mkdir -p "$DEST"
# Links kept as links (bin/paseo is one, into lib).
cp -R "$BUILT/node" "$DEST/node"
echo "service $SERVICE on Node $NODE: $(du -sh "$DEST" | cut -f1) in $DEST"
