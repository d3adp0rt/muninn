#!/usr/bin/env bash
# Build a debug APK for arm64-v8a.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/27.2.12479018}"
export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/build-tools/36.0.0:$PATH"

cd "$ROOT"

cargo ndk -t arm64-v8a -o android/app/src/main/jniLibs build

cd android
./gradlew assembleDebug
