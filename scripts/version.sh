#!/bin/sh
# Prints the app's version (nsh.json).
sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$(dirname "$0")/../nsh.json" | head -n 1
