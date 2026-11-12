#!/bin/sh
# Writes into nsh.json the Info.plist values that say which Xcode and SDK
# built the iOS app (DTPlatformName, DTSDKName, DTXcode, …). Xcode writes
# them itself; `nsh package` does not, and App Store Connect refuses an
# upload without them. They belong to the machine that builds, so the
# Release workflow runs this before packaging and the result is not
# committed: ios-build-info.sh [path/to/nsh.json]
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FILE="${1:-$ROOT/nsh.json}"
SDK_VERSION="$(xcrun --sdk iphoneos --show-sdk-version)"
SDK_BUILD="$(xcrun --sdk iphoneos --show-sdk-build-version)"
XCODE_VERSION="$(xcodebuild -version | sed -n 's/^Xcode //p')"
XCODE_BUILD="$(xcodebuild -version | sed -n 's/^Build version //p')"
OS_BUILD="$(sw_vers -buildVersion)"
python3 - "$FILE" "$SDK_VERSION" "$SDK_BUILD" "$XCODE_VERSION" "$XCODE_BUILD" "$OS_BUILD" <<'PY'
import json, sys
path, sdk, sdk_build, xcode, xcode_build, os_build = sys.argv[1:]
# Xcode 26.1.2 is "2612", 27.0 is "2700".
parts = (xcode.split(".") + ["0", "0"])[:3]
dt_xcode = f"{int(parts[0]):02d}{int(parts[1])}{int(parts[2])}"
d = json.load(open(path))
d.setdefault("ios", {}).setdefault("infoPlist", {}).update({
    "BuildMachineOSBuild": os_build,
    "DTCompiler": "com.apple.compilers.llvm.clang.1_0",
    "DTPlatformBuild": sdk_build,
    "DTPlatformName": "iphoneos",
    "DTPlatformVersion": sdk,
    "DTSDKBuild": sdk_build,
    "DTSDKName": f"iphoneos{sdk}",
    "DTXcode": dt_xcode,
    "DTXcodeBuild": xcode_build,
})
open(path, "w").write(json.dumps(d, indent=2, ensure_ascii=False) + "\n")
print(f"iOS build info: Xcode {xcode} ({xcode_build}), SDK iphoneos{sdk} ({sdk_build})")
PY
