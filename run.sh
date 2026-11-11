#!/usr/bin/env bash
set -e

if [ "${EUID:-$(id -u)}" -eq 0 ] && [ -n "${SUDO_USER:-}" ]; then
    exec sudo -u "$SUDO_USER" HOME="$(getent passwd "$SUDO_USER" | cut -d: -f6)" "$0" "$@"
fi

cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
export GDK_BACKEND=x11
export GSK_RENDERER=cairo

# Hentikan instance lama jika masih berjalan agar versi terbaru langsung aktif
pkill -x arwiland 2>/dev/null || true
sleep 0.15

# Sinkronkan paket aplikasi desktop ke ~/.local agar muncul di App Launcher / Menu Aplikasi
if [ -f "./target/release/arwiland" ]; then
    mkdir -p "$HOME/.local/bin" \
             "$HOME/.local/share/applications" \
             "$HOME/.config/autostart" \
             "$HOME/.local/share/icons/hicolor/scalable/apps" 2>/dev/null || true
    cp -f "./target/release/arwiland" "$HOME/.local/bin/arwiland" 2>/dev/null || true
    chmod +x "$HOME/.local/bin/arwiland" 2>/dev/null || true
    if [ -f "./assets/arwiland.svg" ]; then
        cp -f "./assets/arwiland.svg" "$HOME/.local/share/icons/hicolor/scalable/apps/arwiland.svg" 2>/dev/null || true
    fi
    if [ -f "./assets/arwiland.desktop" ]; then
        cp -f "./assets/arwiland.desktop" "$HOME/.local/share/applications/arwiland.desktop" 2>/dev/null || true
        cp -f "./assets/arwiland.desktop" "$HOME/.config/autostart/arwiland.desktop" 2>/dev/null || true
    fi
    BIN="./target/release/arwiland"
elif [ -f "./target/debug/arwiland" ]; then
    BIN="./target/debug/arwiland"
elif [ -f "$HOME/.local/bin/arwiland" ]; then
    BIN="$HOME/.local/bin/arwiland"
else
    cargo build --release
    BIN="./target/release/arwiland"
fi

if [ "${1:-}" = "--fg" ] || [ "${1:-}" = "--foreground" ]; then
    shift
    exec "$BIN" "$@"
else
    nohup "$BIN" "$@" >/dev/null 2>&1 </dev/null &
    disown 2>/dev/null || true
    sleep 0.25
    echo "Arwiland sudah berjalan di background (aman menutup terminal)."
fi
