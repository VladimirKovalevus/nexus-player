#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
APPIMAGE_SOURCE="$SCRIPT_DIR/../target/release/bundle/appimage/nexus-player-frontend_0.1.0_amd64.AppImage"
ICON_SOURCE="$SCRIPT_DIR/src-tauri/icons/128x128.png"

APP_ID="nexus-player"
APPIMAGE_TARGET="$HOME/.local/bin/$APP_ID"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
DESKTOP_TARGET="$DATA_HOME/applications/$APP_ID.desktop"
ICON_TARGET="$DATA_HOME/icons/hicolor/128x128/apps/$APP_ID.png"

# WebKitGTK env flags (see README "Linux: WebKitGTK environment flags"), picked by the
# symptoms each flag targets. Override with: WEBKIT_FLAGS="VAR=1 VAR2=1" ./install.sh
# (WEBKIT_FLAGS="" disables them).
detect_webkit_flags() {
  local flags=()
  local wayland=0 nvidia=0
  [[ "${XDG_SESSION_TYPE:-}" == wayland || -n "${WAYLAND_DISPLAY:-}" ]] && wayland=1
  [[ -e /proc/driver/nvidia/version ]] && nvidia=1

  if command -v systemd-detect-virt >/dev/null && systemd-detect-virt -q --vm; then
    flags+=(LIBGL_ALWAYS_SOFTWARE=1)
  fi
  if (( nvidia || wayland )); then
    flags+=(WEBKIT_DISABLE_DMABUF_RENDERER=1)
  fi
  if (( nvidia && wayland )); then
    flags+=(__NV_DISABLE_EXPLICIT_SYNC=1)
  fi
  echo "${flags[*]}"
}

# Launcher kind:
#   desktop - reads .desktop entries (rofi drun, wofi, fuzzel, DE menus)
#   path    - runs binaries from $PATH (wmenu-run, dmenu_run, bemenu-run)
# Override with: LAUNCHER=desktop|path ./install.sh
detect_launcher() {
  local configs=(
    "$HOME/.config/i3/config"
    "$HOME/.config/sway/config"
    "$HOME/.config/hypr/hyprland.conf"
    "$HOME/.config/niri/config.kdl"
  )
  local cfg
  for cfg in "${configs[@]}"; do
    [[ -f "$cfg" ]] || continue
    if grep -qE '^[^#]*(rofi[^#]*drun|wofi|fuzzel|tofi-drun|anyrun|walker)' "$cfg"; then
      echo desktop
      return
    fi
    if grep -qE '^[^#]*(wmenu-run|dmenu_run|bemenu-run|tofi-run)' "$cfg"; then
      echo path
      return
    fi
  done
  # Full desktop environments (GNOME, KDE, XFCE...) always use .desktop entries
  echo desktop
}

if [[ ! -f "$APPIMAGE_SOURCE" ]]; then
  echo "AppImage not found: $APPIMAGE_SOURCE" >&2
  echo "Build it first with: npm run bld" >&2
  exit 1
fi

WEBKIT_FLAGS="${WEBKIT_FLAGS-$(detect_webkit_flags)}"
EXEC_PREFIX=""
if [[ -n "$WEBKIT_FLAGS" ]]; then
  EXEC_PREFIX="env $WEBKIT_FLAGS "
fi

install -Dm755 "$APPIMAGE_SOURCE" "$APPIMAGE_TARGET"
echo "Nexus Player installed to $APPIMAGE_TARGET."
echo "WebKitGTK flags: ${WEBKIT_FLAGS:-none}"

LAUNCHER="${LAUNCHER:-$(detect_launcher)}"

case "$LAUNCHER" in
  desktop)
    install -Dm644 "$ICON_SOURCE" "$ICON_TARGET"
    mkdir -p "$(dirname "$DESKTOP_TARGET")"
    cat > "$DESKTOP_TARGET" <<EOF
[Desktop Entry]
Type=Application
Name=Nexus Player
Comment=Music player
Exec=${EXEC_PREFIX}$APPIMAGE_TARGET %U
Icon=$APP_ID
Terminal=false
Categories=AudioVideo;Audio;Player;
StartupWMClass=nexus-player-frontend
EOF
    command -v update-desktop-database >/dev/null && update-desktop-database -q "$(dirname "$DESKTOP_TARGET")" || true
    command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -qtf "$DATA_HOME/icons/hicolor" || true
    echo "Desktop entry installed to $DESKTOP_TARGET."
    ;;
  path)
    echo "Make sure $HOME/.local/bin is included in PATH used by your launcher."
    if [[ -n "$WEBKIT_FLAGS" ]]; then
      echo "PATH launchers don't read .desktop files; export these in your shell profile: $WEBKIT_FLAGS"
    fi
    ;;
  *)
    echo "Unknown LAUNCHER: $LAUNCHER (expected: desktop|path)" >&2
    exit 1
    ;;
esac
