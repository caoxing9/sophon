#!/bin/sh
# Packages the Android app: `nsh package --target android` with what the
# build of NSH's Android runtime needs on a Mac.
#
# NSH hands bindgen (for the runtime's QuickJS) the NDK's own clang headers
# as well as the sysroot. bindgen runs on the machine's libclang (Xcode's,
# another version: the NDK ships none since r27), which adds its own, and
# the two `stdint.h` shut each other out: "unknown type name 'uint32_t'".
# bindgen reads `BINDGEN_EXTRA_CLANG_ARGS_<triple>` with the triple's
# hyphens before the name NSH sets (with underscores): set here, without
# the NDK's clang headers, it wins.
#
#   scripts/android-package.sh [nsh package options…]
set -eu
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
NDK="${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* 2>/dev/null | sort -V | tail -n 1)}"
[ -d "$NDK" ] || { echo "no Android NDK (ANDROID_NDK_HOME, or $SDK/ndk)" >&2; exit 1; }
SYSROOT="$(ls -d "$NDK"/toolchains/llvm/prebuilt/*/sysroot | head -n 1)"
# The API level NSH builds for (its MIN_SDK).
API=28
exec env \
  "BINDGEN_EXTRA_CLANG_ARGS_aarch64-linux-android=--target=aarch64-linux-android$API --sysroot=\"$SYSROOT\"" \
  "BINDGEN_EXTRA_CLANG_ARGS_armv7-linux-androideabi=--target=armv7a-linux-androideabi$API --sysroot=\"$SYSROOT\"" \
  "BINDGEN_EXTRA_CLANG_ARGS_x86_64-linux-android=--target=x86_64-linux-android$API --sysroot=\"$SYSROOT\"" \
  nsh package --target android --icon "$(dirname "$0")/../assets/app-icon/icon-ios-1024.png" "$@"
