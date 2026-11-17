#!/bin/sh
# Writes service-patches/paseo-<version>.patch: Sophon's changes to the agent
# service, as compiled, for scripts/bundle-service.sh to apply over the package
# from npm.
#
#   scripts/make-service-patch.sh path/to/paseo [version]
#
# path/to/paseo is a checkout of github.com/caoxing9/paseo, on a branch made
# from the release tag of `version` (0.16.1 by default) and built
# (`npm ci && npm run build:server`). The patch is the difference between its
# compiled protocol, client and server and those of @getpaseo/cli@<version>.
#
# Left out: source maps, type declarations, build info, the web UI, and the
# generated outbound validators (protocol/dist/generated/validation), which a
# build renumbers throughout; the service does not use them, and the clients
# in it that do accept a message with fields they do not know.
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FORK="$(cd "${1:?usage: $0 path/to/paseo [version]}" && pwd)"
VERSION="${2:-0.16.1}"
OUT="$ROOT/service-patches/paseo-$VERSION.patch"

WORK="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$WORK"' EXIT
npm install -g --no-audit --no-fund --loglevel error --prefix "$WORK/npm" "@getpaseo/cli@$VERSION" >/dev/null
FROM="$WORK/npm/lib/node_modules/@getpaseo/cli/node_modules/@getpaseo"

mkdir -p "$WORK/a" "$WORK/b"
for package in protocol client server; do
  [ -d "$FORK/packages/$package/dist" ] || { echo "$FORK/packages/$package is not built" >&2; exit 1; }
  for side in a b; do
    if [ "$side" = a ]; then src="$FROM/$package/dist"; else src="$FORK/packages/$package/dist"; fi
    mkdir -p "$WORK/$side/$package/dist"
    rsync -a --copy-links --prune-empty-dirs \
      --exclude='/generated/validation/' --exclude='/server/web-ui/' \
      --include='*/' --include='*.js' --exclude='*' \
      "$src/" "$WORK/$side/$package/dist/"
  done
done

COMMIT="$(git -C "$FORK" rev-parse --short HEAD)"
BRANCH="$(git -C "$FORK" rev-parse --abbrev-ref HEAD)"
{
  echo "Sophon's changes to the agent service (Paseo $VERSION), as compiled: the"
  echo "difference between @getpaseo/cli@$VERSION from npm and a build of"
  echo "github.com/caoxing9/paseo at $COMMIT (branch $BRANCH)."
  echo "scripts/bundle-service.sh applies it in node_modules/@getpaseo."
  echo
  (cd "$WORK" && diff -ruN a b || true) | sed -E 's/^(\+\+\+|---) ([^	]+)	.*/\1 \2/'
} > "$OUT"
echo "wrote $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
