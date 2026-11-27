#!/bin/sh
# Builds the docs site in site/: the gallery's web build in site/demo/ and
# the docs page (tools/preview.py) that runs it live. Needs the NSH CLI 0.7
# and an LLVM with wasm32 support on PATH for C dependencies
# (macOS: `brew install llvm`, then PATH="$(brew --prefix llvm)/bin:$PATH").
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
rm -rf "$ROOT/site"
mkdir -p "$ROOT/site"
(cd "$ROOT/examples/gallery" && nsh package --target web -o "$ROOT/site/.web")
mv "$ROOT/site/.web/"* "$ROOT/site/demo"
rmdir "$ROOT/site/.web"
python3 "$ROOT/tools/preview.py"
touch "$ROOT/site/.nojekyll"
echo "site: $ROOT/site"
