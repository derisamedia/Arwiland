#!/usr/bin/env bash
set -euo pipefail

# Jika dijalankan dengan sudo, jalankan ulang sebagai user asli agar tidak salah membaca /root
if [ "${EUID:-$(id -u)}" -eq 0 ] && [ -n "${SUDO_USER:-}" ]; then
    echo "==> Terdeteksi menggunakan sudo, mengalihkan ke user '$SUDO_USER'..."
    exec sudo -u "$SUDO_USER" HOME="$(getent passwd "$SUDO_USER" | cut -d: -f6)" "$0" "$@"
fi

cd "$(dirname "$0")"

export PATH="$HOME/.cargo/bin:$PATH"
if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
fi

echo "==> [1/4] Mem-build binary rilis Arwiland..."
if command -v cargo >/dev/null 2>&1; then
    cargo build --release
elif [ -f "target/release/arwiland" ]; then
    echo "    (cargo tidak ditemukan di PATH, menggunakan binary target/release/arwiland yang sudah ada)"
elif [ -f "dist/Arwiland/arwiland" ]; then
    echo "    (cargo tidak ditemukan di PATH, menggunakan binary dist/Arwiland/arwiland yang sudah ada)"
    mkdir -p target/release
    cp -f dist/Arwiland/arwiland target/release/arwiland
else
    echo "Error: 'cargo' (Rust) tidak ditemukan dan binary belum tersedia." >&2
    exit 1
fi

APP_BIN_DIR="$HOME/.local/bin"
APP_DESKTOP_DIR="$HOME/.local/share/applications"
APP_AUTOSTART_DIR="$HOME/.config/autostart"
APP_ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
DIST_DIR="$(pwd)/dist/Arwiland"

echo "==> [2/4] Menyiapkan paket aplikasi di $DIST_DIR..."
mkdir -p "$DIST_DIR"
cp -f target/release/arwiland "$DIST_DIR/arwiland"
chmod +x "$DIST_DIR/arwiland"
cp -f assets/arwiland.svg "$DIST_DIR/arwiland.svg"

cat > "$DIST_DIR/Arwiland.desktop" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=Arwiland
GenericName=Dynamic Island & Living Notch
Comment=Dynamic Island, Living Notch, Lirik Real-Time, Jadwal Sholat & Pusat Notifikasi untuk Linux
Exec=env GDK_BACKEND=x11 GSK_RENDERER=cairo $APP_BIN_DIR/arwiland
Icon=arwiland
Terminal=false
Categories=Utility;GTK;DesktopSettings;
Keywords=notch;island;dynamic;lyrics;notifications;sholat;volume;
StartupNotify=false
StartupWMClass=arwiland
EOF
chmod +x "$DIST_DIR/Arwiland.desktop"

echo "==> [3/4] Menginstal binary & ikon ke sistem pengguna (~/.local)..."
mkdir -p "$APP_BIN_DIR" "$APP_DESKTOP_DIR" "$APP_AUTOSTART_DIR" "$APP_ICON_DIR"

# Hentikan proses lama jika sedang berjalan agar file binary tidak terkunci (Text file busy)
pkill -x arwiland 2>/dev/null || true
sleep 0.2

cp -f target/release/arwiland "$APP_BIN_DIR/arwiland"
chmod +x "$APP_BIN_DIR/arwiland"
cp -f assets/arwiland.svg "$APP_ICON_DIR/arwiland.svg"
cp -f "$DIST_DIR/Arwiland.desktop" "$APP_DESKTOP_DIR/arwiland.desktop"
cp -f "$DIST_DIR/Arwiland.desktop" "$APP_AUTOSTART_DIR/arwiland.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APP_DESKTOP_DIR" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
fi

echo "==> [4/4] Menjalankan Arwiland di background..."
if [ -n "${DISPLAY:-}" ] || [ -n "${WAYLAND_DISPLAY:-}" ]; then
    nohup env GDK_BACKEND=x11 GSK_RENDERER=cairo "$APP_BIN_DIR/arwiland" >/dev/null 2>&1 </dev/null &
    disown 2>/dev/null || true
    sleep 0.3
fi

echo "==> Selesai! Aplikasi Arwiland berhasil terinstal dan sedang berjalan di background:"
echo "    • Binary Sistem : $APP_BIN_DIR/arwiland"
echo "    • Menu Aplikasi : $APP_DESKTOP_DIR/arwiland.desktop"
echo "    • Autostart     : $APP_AUTOSTART_DIR/arwiland.desktop"
echo "    • Ikon Aplikasi : $APP_ICON_DIR/arwiland.svg"
echo "    • Paket Portable: $DIST_DIR/"
