#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
APPIMAGE_SOURCE="$SCRIPT_DIR/../target/release/bundle/appimage/nexus-player-frontend_0.1.0_amd64.AppImage"
APPIMAGE_TARGET="$HOME/.local/bin/nexus-player"

if [[ ! -f "$APPIMAGE_SOURCE" ]]; then
  echo "AppImage not found: $APPIMAGE_SOURCE" >&2
  echo "Build it first with: npm run bld" >&2
  exit 1
fi

install -Dm755 "$APPIMAGE_SOURCE" "$APPIMAGE_TARGET"

echo "Nexus Player installed to $APPIMAGE_TARGET."
echo "Make sure $HOME/.local/bin is included in PATH used by wmenu-run."
