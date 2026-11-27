#!/bin/sh
# Pushes crates/nsh-shadcn's changes to its own repository
# (caoxing9/nsh-shadcn, main), whose history it is a subtree of. Run on
# Sophon's main after they are merged there.
set -eu
cd "$(dirname "$0")/.."
git subtree push --prefix=crates/nsh-shadcn https://github.com/caoxing9/nsh-shadcn main
