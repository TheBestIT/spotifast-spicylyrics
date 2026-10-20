#!/usr/bin/env bash
# Build Spotifast Spicy Lyrics from source on Linux, and install it if you ask.
#
#   scripts/build-linux.sh              build only
#   scripts/build-linux.sh --deps       install the build packages first (uses sudo)
#   scripts/build-linux.sh --install    install for this user (~/.local), no sudo
#   scripts/build-linux.sh --milkdrop   also build the MilkDrop visualiser
#
# Options combine: scripts/build-linux.sh --deps --install
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
deps=0 install=0 milkdrop=0
for argument in "$@"; do
    case "$argument" in
        --deps) deps=1 ;;
        --install) install=1 ;;
        --milkdrop) milkdrop=1 ;;
        -h|--help) sed -n '2,9p' "$0"; exit 0 ;;
        *) echo "Unknown option: $argument" >&2; exit 2 ;;
    esac
done

step() { printf '\033[36m==> %s\033[0m\n' "$*"; }
fail() { printf '\033[31mERROR: %s\033[0m\n' "$*" >&2; exit 1; }

# 1. Build packages.
if [ "$deps" = 1 ]; then
    step "Install the build packages"
    if command -v apt-get >/dev/null; then
        packages="build-essential pkg-config curl libasound2-dev libpipewire-0.3-dev libclang-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev libdbus-1-dev"
        [ "$milkdrop" = 1 ] && packages="$packages cmake"
        sudo apt-get update && sudo apt-get install -y $packages
    elif command -v dnf >/dev/null; then
        packages="gcc gcc-c++ make pkgconf-pkg-config curl alsa-lib-devel pipewire-devel clang-devel libxkbcommon-devel wayland-devel mesa-libGL-devel mesa-libEGL-devel dbus-devel"
        [ "$milkdrop" = 1 ] && packages="$packages cmake"
        sudo dnf install -y $packages
    elif command -v pacman >/dev/null; then
        packages="base-devel curl alsa-lib pipewire clang libxkbcommon wayland mesa dbus"
        [ "$milkdrop" = 1 ] && packages="$packages cmake"
        sudo pacman -S --needed --noconfirm $packages
    elif command -v zypper >/dev/null; then
        packages="gcc gcc-c++ make pkg-config curl alsa-devel pipewire-devel clang-devel libxkbcommon-devel wayland-devel Mesa-libGL-devel dbus-1-devel"
        [ "$milkdrop" = 1 ] && packages="$packages cmake"
        sudo zypper install -y $packages
    else
        fail "Unknown package manager. Install a C compiler and the ALSA, PipeWire, libclang, xkbcommon, Wayland, OpenGL and D-Bus development packages."
    fi
fi

# 2. Rust.
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
if ! command -v cargo >/dev/null; then
    step "Install Rust with rustup (for this user only)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    . "$HOME/.cargo/env"
fi

# 3. Build.
features="--no-default-features"
[ "$milkdrop" = 1 ] && features=""
step "Build the release program (the first build takes 5 to 15 minutes)"
cd "$root"
# shellcheck disable=SC2086
cargo build --release $features
binary="$root/target/release/spotifast"
printf '\033[32mBuilt: %s\033[0m\n' "$binary"

# 4. Install for this user.
if [ "$install" = 1 ]; then
    step "Install to ~/.local"
    install -Dm755 "$binary" "$HOME/.local/bin/spotifast"
    install -Dm644 packaging/applications/spotifast.desktop "$HOME/.local/share/applications/spotifast.desktop"
    install -Dm644 packaging/icons/spotifast.svg "$HOME/.local/share/icons/hicolor/scalable/apps/spotifast.svg"
    command -v update-desktop-database >/dev/null && update-desktop-database "$HOME/.local/share/applications" || true
    printf '\033[32mInstalled. Start "Spotifast" from your applications menu, or run: spotifast\033[0m\n'
    case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *) echo "Note: add ~/.local/bin to your PATH to run spotifast from a terminal." ;; esac
fi
