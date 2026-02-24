#!/usr/bin/env bash
set -e

APK_DIR="src-tauri/gen/android/app/build/outputs/apk/universal/release"
UNSIGNED="$APK_DIR/app-universal-release-unsigned.apk"
SIGNED="$APK_DIR/app-universal-release-debug-signed.apk"
ADB="$HOME/Android/Sdk/platform-tools/adb"
BUILD_TOOLS="$HOME/Android/Sdk/build-tools/$(ls "$HOME/Android/Sdk/build-tools" | sort -V | tail -1)"

echo "==> Signing APK with debug keystore..."
"$BUILD_TOOLS/apksigner" sign \
  --ks ~/.android/debug.keystore \
  --ks-pass pass:android \
  --key-pass pass:android \
  --ks-key-alias androiddebugkey \
  --out "$SIGNED" \
  "$UNSIGNED"

echo "==> Installing on device..."
"$ADB" install -r "$SIGNED"

echo "==> Done."
