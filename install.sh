#!/usr/bin/env bash
# Install thumbdeck for the current user (no sudo).
#   Linux: ~/.local/bin/thumbdeck + a launcher entry and icon (shows up in wofi, rofi, ...)
#   macOS: ~/Applications/thumbdeck.app
#
#   ./install.sh             build this checkout and install it (needs Node and Rust); run it
#                            again after pulling changes
#   ./install.sh --release   install the latest release instead: a download, no build tools
#   curl -fsSL https://github.com/Nozeren/thumbdeck-releases/releases/latest/download/install.sh | bash
#                            the same, without a checkout
#
# Either way, thumbdeck then updates itself from the releases.
set -euo pipefail

RELEASES=https://github.com/Nozeren/thumbdeck-releases/releases/latest/download
# The key releases are signed with (the one in src-tauri/src/updater.pub); a download is checked
# against it when minisign is installed
PUBLIC_KEY=RWQK5zHh1lb4wVvHmZrdZi4UE+6NGr0yALq9PjVvZ/2LvSuZJr7oM0A0

info() { printf '\033[1;32m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

# ------------------------------------------------------------ installing

# install_linux BINARY [SVG_ICON] [PNG_ICON]
install_linux() {
    info "Installing to ~/.local"
    install -Dm755 "$1" "$HOME/.local/bin/thumbdeck"
    [ -f "${2:-}" ] && install -Dm644 "$2" "$HOME/.local/share/icons/hicolor/scalable/apps/thumbdeck.svg"
    [ -f "${3:-}" ] && install -Dm644 "$3" "$HOME/.local/share/icons/hicolor/128x128/apps/thumbdeck.png"
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
}

# install_macos APP
install_macos() {
    info "Installing to ~/Applications"
    mkdir -p "$HOME/Applications"
    rm -rf "$HOME/Applications/thumbdeck.app"
    cp -R "$1" "$HOME/Applications/"
    # Not needed for a download made here, but a copy that came through a browser would be blocked
    xattr -dr com.apple.quarantine "$HOME/Applications/thumbdeck.app" 2>/dev/null || true
    info "Done: open thumbdeck from Spotlight or ~/Applications"
}

# ------------------------------------------------------------ from this checkout

from_source() {
    cd "$1"
    [ -d node_modules ] || { info "Installing JavaScript dependencies"; npm install --no-audit --no-fund; }
    info "Building (release)"
    case "$(uname -s)" in
        Linux)
            npm run tauri build -- --no-bundle
            install_linux src-tauri/target/release/thumbdeck assets/icon.svg src-tauri/icons/128x128.png
            ;;
        Darwin)
            npm run tauri build -- --bundles app
            install_macos src-tauri/target/release/bundle/macos/thumbdeck.app
            ;;
        *) die "unsupported system: $(uname -s)" ;;
    esac
}

# ------------------------------------------------------------ from the latest release

from_release() {
    local platform
    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64) platform=linux-x86_64 ;;
        Darwin-*) platform=darwin-universal ;;
        *) die "there's no release for $(uname -s) $(uname -m); build it from a checkout with ./install.sh" ;;
    esac
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    get() { curl -fsSL --proto '=https' --retry 2 "$RELEASES/$1" -o "$tmp/$1" || die "couldn't download $1"; }

    get latest.json
    local version
    version="$(sed -n 's/^ *"version": *"\([^"]*\)".*/\1/p' "$tmp/latest.json" | head -1)"
    [ -n "$version" ] || die "latest.json has no version"
    info "Downloading thumbdeck $version ($platform)"
    get "thumbdeck-$platform.tar.gz"
    get "thumbdeck-$platform.tar.gz.sig"

    if command -v minisign >/dev/null; then
        base64 --decode < "$tmp/thumbdeck-$platform.tar.gz.sig" > "$tmp/download.minisig"
        local checked
        checked="$(minisign -V -P "$PUBLIC_KEY" -m "$tmp/thumbdeck-$platform.tar.gz" -x "$tmp/download.minisig")" \
            || die "the download isn't signed with thumbdeck's key: not installing it"
        # The version is in the signed part: an older build can't be passed off as this one
        printf '%s\n' "$checked" | grep -Eq "(^|[[:space:]])version:${version//./\\.}([[:space:]]|\$)" \
            || die "the download isn't signed for version $version: not installing it"
        info "Signature checked"
    else
        info "minisign isn't installed, so the signature isn't checked (the download comes from GitHub over HTTPS; thumbdeck checks every update it installs itself)"
    fi

    mkdir "$tmp/x"
    tar -xzf "$tmp/thumbdeck-$platform.tar.gz" -C "$tmp/x"
    case "$platform" in
        linux-x86_64) install_linux "$tmp/x/thumbdeck" "$tmp/x/thumbdeck.svg" "$tmp/x/thumbdeck.png" ;;
        darwin-*) install_macos "$tmp/x/thumbdeck.app" ;;
    esac
}

# ------------------------------------------------------------ which one

# Run from a checkout (not piped from curl) and not asked for a release: build it
script="${BASH_SOURCE[0]:-}"
checkout=""
if [ -n "$script" ] && [ -f "$script" ]; then
    checkout="$(cd "$(dirname "$script")" && pwd)"
    [ -f "$checkout/src-tauri/tauri.conf.json" ] || checkout=""
fi
if [ "${1:-}" = "--release" ] || [ -z "$checkout" ]; then
    from_release
else
    from_source "$checkout"
fi
