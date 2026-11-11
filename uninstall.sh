#!/usr/bin/env bash
set -euo pipefail

echo "==> Menghentikan proses Arwiland yang sedang berjalan..."
pkill -x arwiland 2>/dev/null || true

echo "==> Menghapus instalasi Arwiland dari ~/.local..."
rm -f "$HOME/.local/bin/arwiland"
rm -f "$HOME/.local/share/applications/arwiland.desktop"
rm -f "$HOME/.local/share/icons/hicolor/scalable/apps/arwiland.svg"
rm -f "$HOME/.config/autostart/arwiland.desktop"

echo "==> Arwiland berhasil dihapus dari sistem pengguna."
