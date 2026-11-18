#!/bin/sh
# Sets the app's version in nsh.json and Cargo.toml: set-version.sh v0.2.0 [build]
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
V="${1#v}"
case "$V" in refs/tags/*) V="${V#refs/tags/v}" ;; esac
python3 - "$ROOT" "$V" "${2:-}" <<'PY'
import json, re, sys
root, v, build = sys.argv[1], sys.argv[2], sys.argv[3]
p = f"{root}/nsh.json"
d = json.load(open(p))
d["version"] = v
if build:
    d["build"] = int(build)
open(p, "w").write(json.dumps(d, indent=2, ensure_ascii=False) + "\n")
for manifest in ("Cargo.toml", "crates/sophon-service/Cargo.toml"):
    p = f"{root}/{manifest}"
    s = open(p).read()
    s = re.sub(r'(?m)^version = "[^"]*"', f'version = "{v}"', s, count=1)
    open(p, "w").write(s)
p = f"{root}/Cargo.lock"
s = open(p).read()
s = re.sub(r'(?m)^(name = "sophon(?:-service)?"\n)version = "[^"]*"', lambda m: m[1] + f'version = "{v}"', s)
open(p, "w").write(s)
PY
echo "version $V"
