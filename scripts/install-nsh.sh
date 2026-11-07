#!/bin/sh
# Builds the `nsh` CLI at the NSH revision Cargo.toml pins, so the CLI and the
# crates the app links always match (the released nsh can lag behind the pin,
# e.g. v0.4.0 rejects nsh.json "schemes").
#
#   scripts/install-nsh.sh            # builds into ~/.cache/sophon-nsh/<rev>
#   NSH_SRC=/path scripts/install-nsh.sh
#
# Prints the directory holding the `nsh` binary; add it to PATH.
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REV="$(sed -n 's/^nsh-core = .*rev = "\([0-9a-f]*\)".*/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
[ -n "$REV" ] || { echo "no nsh-core rev in Cargo.toml" >&2; exit 1; }
SRC="${NSH_SRC:-$HOME/.cache/sophon-nsh/$REV}"

if [ ! -d "$SRC/.git" ]; then
  mkdir -p "$SRC"
  git -C "$SRC" init -q
  git -C "$SRC" remote add origin https://github.com/longbridge/nsh.git
fi
if [ "$(git -C "$SRC" rev-parse HEAD 2>/dev/null || true)" != "$REV" ]; then
  git -C "$SRC" fetch -q --depth 1 origin "$REV"
  git -C "$SRC" checkout -q --detach FETCH_HEAD
fi
(cd "$SRC" && cargo build --release --locked -p nsh-cli >&2)
echo "$SRC/target/release"
