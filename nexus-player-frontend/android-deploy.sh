#!/usr/bin/env bash
set -e

APK_DIR="src-tauri/gen/android/app/build/outputs/apk/universal/release"
UNSIGNED="$APK_DIR/app-universal-release-unsigned.apk"
SIGNED="$APK_DIR/app-universal-release-debug-signed.apk"
SDK="${ANDROID_HOME:-$HOME/Android/Sdk}"
KEYSTORE="${ANDROID_KEYSTORE:-$HOME/.android/debug.keystore}"
ADB="$SDK/platform-tools/adb"

if [ ! -f "$UNSIGNED" ]; then
  echo "Unsigned APK not found: $UNSIGNED (run npm run android-build first)" >&2
  exit 1
fi
if [ ! -d "$SDK/build-tools" ] || [ ! -x "$ADB" ]; then
  echo "Android SDK tools not found in $SDK (set ANDROID_HOME)" >&2
  exit 1
fi
BUILD_TOOLS="$SDK/build-tools/$(ls "$SDK/build-tools" | sort -V | tail -1)"
if [ ! -f "$KEYSTORE" ]; then
  echo "Keystore not found: $KEYSTORE (set ANDROID_KEYSTORE)" >&2
  exit 1
fi

echo "==> Signing APK with debug keystore..."
"$BUILD_TOOLS/apksigner" sign \
  --ks "$KEYSTORE" \
  --ks-pass pass:android \
  --key-pass pass:android \
  --ks-key-alias androiddebugkey \
  --out "$SIGNED" \
  "$UNSIGNED"

echo "==> Installing on device..."
"$ADB" install -r "$SIGNED"

echo "==> Done."
