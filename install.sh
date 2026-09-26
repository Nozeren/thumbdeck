#!/usr/bin/env bash
# Build thumbdeck and install it for the current user (no sudo).
#   Linux: ~/.local/bin/thumbdeck + a launcher entry and icon (shows up in wofi, rofi, ...)
#   macOS: ~/Applications/thumbdeck.app
# Run it again after pulling changes to update.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

info() { printf '\033[1;32m==>\033[0m %s\n' "$*"; }

[ -d node_modules ] || { info "Installing JavaScript dependencies"; npm install --no-audit --no-fund; }

case "$(uname -s)" in
    Linux)
        info "Building (release)"
        npm run tauri build -- --no-bundle
        info "Installing to ~/.local"
        install -Dm755 src-tauri/target/release/thumbdeck "$HOME/.local/bin/thumbdeck"
        install -Dm644 assets/icon.svg "$HOME/.local/share/icons/hicolor/scalable/apps/thumbdeck.svg"
        install -Dm644 src-tauri/icons/128x128.png "$HOME/.local/share/icons/hicolor/128x128/apps/thumbdeck.png"
        install -Dm644 /dev/stdin "$HOME/.local/share/applications/thumbdeck.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=thumbdeck
Comment=Your projects and their commands, one click away
Exec=$HOME/.local/bin/thumbdeck
Icon=thumbdeck
Terminal=false
Categories=Development;
StartupWMClass=thumbdeck
DESKTOP
        command -v update-desktop-database >/dev/null && update-desktop-database -q "$HOME/.local/share/applications" || true
        info "Done: open thumbdeck from your app launcher, or run 'thumbdeck'"
        ;;
    Darwin)
        info "Building (release)"
        npm run tauri build -- --bundles app
        info "Installing to ~/Applications"
        mkdir -p "$HOME/Applications"
        rm -rf "$HOME/Applications/thumbdeck.app"
        cp -R src-tauri/target/release/bundle/macos/thumbdeck.app "$HOME/Applications/"
        info "Done: open thumbdeck from Spotlight or ~/Applications"
        ;;
    *)
        echo "Unsupported system: $(uname -s)" >&2
        exit 1
        ;;
esac
