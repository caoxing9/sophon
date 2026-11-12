#!/bin/sh
# Checks that the app compiles for Android, without packaging it: what a
# pull request asks (a release packages). The C among the dependencies
# needs the NDK's compiler, and bindgen its sysroot (see
# `android-package.sh` for why the arguments are set under this name).
#
#   scripts/android-check.sh [cargo check options…]
set -eu
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
NDK="${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* 2>/dev/null | sort -V | tail -n 1)}"
[ -d "$NDK" ] || { echo "no Android NDK (ANDROID_NDK_HOME, or $SDK/ndk)" >&2; exit 1; }
TOOLS="$(ls -d "$NDK"/toolchains/llvm/prebuilt/* | head -n 1)"
# The API level NSH builds for (its MIN_SDK).
API=28
exec env \
  "CC_aarch64_linux_android=$TOOLS/bin/aarch64-linux-android$API-clang" \
  "CXX_aarch64_linux_android=$TOOLS/bin/aarch64-linux-android$API-clang++" \
  "AR_aarch64_linux_android=$TOOLS/bin/llvm-ar" \
  "BINDGEN_EXTRA_CLANG_ARGS_aarch64-linux-android=--target=aarch64-linux-android$API --sysroot=\"$TOOLS/sysroot\"" \
  cargo check --lib --locked --target aarch64-linux-android "$@"
