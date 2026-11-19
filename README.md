# 🏝️ Arwiland — Ultra-Lightweight Pure-Rust Dynamic Island & Living Notch (Linux & Windows)

**Arwiland** adalah aplikasi *Dynamic Island & Living Notch* super ringan dan cepat yang dibangun **100% Pure Rust** (**tanpa GTK4**, **tanpa Electron**, dan **tanpa DLL eksternal**). Menggunakan renderer native (`tiny-skia` + `fontdue`) dengan backend **X11 / D-Bus** di Linux dan **Win32 / WinRT (SMTC & CoreAudio)** di Windows 10/11.

---

## 📥 Download & Instalasi Langsung (Tanpa Perlu Rust / GTK4 / MSYS2)

Cukup unduh file siap pakai langsung dari tab **[Releases](https://github.com/derisamedia/Arwiland/releases/latest)**:

| Sistem Operasi | File Unduhan | Cara Pasang |
| :--- | :--- | :--- |
| 🪟 **Windows 10 / 11 (Installer)** | **[`Arwiland-Windows-x64-Setup.exe`](https://github.com/derisamedia/Arwiland/releases/latest)** | 1. Klik 2x **`Arwiland-Windows-x64-Setup.exe`**.<br>2. Klik **Next -> Install**.<br>3. Otomatis membuat shortcut di Desktop & Start Menu dan langsung siap dijalankan. |
| 🪟 **Windows 10 / 11 (Portable)** | **[`Arwiland-Windows-x64-Portable.zip`](https://github.com/derisamedia/Arwiland/releases/latest)** / **[`Arwiland-Windows-x64.exe`](https://github.com/derisamedia/Arwiland/releases/latest)** | Single standalone `.exe` tanpa perlu instalasi dan tanpa DLL eksternal — langsung klik 2x untuk menjalankan. |
| 🐧 **Linux (Ubuntu / Debian / Mint)** | **[`Arwiland-Linux-amd64.deb`](https://github.com/derisamedia/Arwiland/releases/latest)** | Klik 2x file `.deb` atau jalankan di terminal:<br>`sudo apt install ./Arwiland-Linux-amd64.deb` |
| 🎩 **Linux (Fedora / RHEL / openSUSE)** | **[`Arwiland-Linux-x86_64.rpm`](https://github.com/derisamedia/Arwiland/releases/latest)** | Klik 2x file `.rpm` atau jalankan di terminal:<br>`sudo dnf install ./Arwiland-Linux-x86_64.rpm` |
| 📦 **Linux (Portable)** | **[`Arwiland-Linux-x86_64-Portable.tar.gz`](https://github.com/derisamedia/Arwiland/releases/latest)** | Ekstrak arsip lalu jalankan `./run.sh` (atau `./install.sh` untuk memasang ke menu aplikasi & autostart). |

---

## ⚡ Keunggulan Arsitektur Pure Rust

- **Zero GTK4 & Zero External DLL**: Tidak lagi bergantung pada runtime GTK4/GLib di Linux maupun MSYS2/DLL tambahan di Windows.
- **Sangat Hemat RAM & CPU**: Dioptimalkan untuk penggunaan memori rendah (`MALLOC_ARENA_MAX=2` di Linux) dan render efisien hanya saat ada perubahan UI/animasi.
- **100% Native OS Integration**:
  - **Linux**: X11 Layered Overlay (`x11rb` + `XShape`/`RandR`), **MPRIS** & **Desktop Notifications** via D-Bus (`zbus`).
  - **Windows 10/11**: Win32 Transparent Overlay (`UpdateLayeredWindow`, Per-Monitor DPI Aware), **SMTC** (Media Control), **CoreAudio** (Volume/Mute), dan **WinRT Notifications**.

---

## ✨ Fitur Utama

1. **🏝️ Compact & Expanded Dynamic Island + Living Notch (Atas / Bawah Layar)**
   - **Posisi Fleksibel**: Dapat ditempatkan di **Atas Layar (`Top`)** maupun **Bawah Layar (`Bottom`)**, dengan gaya **Living Notch** atau **Floating Pill** serta *Y-offset* kustom.
   - **Mode Compact (Kapsul/Notch)**: Menampilkan *Mini Audio Spectrum Visualizer*, jam real-time, badge notifikasi belum dibaca, lirik lagu kata-demi-kata (*word-by-word karaoke*), atau hitung mundur waktu sholat berikutnya.
   - **Transisi Morphing Halus**: Otomatis membesar (*expand*) dengan fisika pegas (*spring physics*) saat kursor diarahkan ke notch (atau diklik), dan mengecil (*collapse*) saat kursor keluar (kecuali dikunci dengan tombol **📍 Pin**).

2. **🔔 Live Notification Toast & Pusat Notifikasi (Tab `Notif`)**
   - Menampilkan notifikasi aplikasi & sistem (status cas/baterai, perubahan volume/mute, waktu sholat tiba, Pomodoro selesai, alarm berbunyi, target hidrasi, dll.) langsung ke dalam Notch lengkap dengan ikon kategori, *ambient glow*, teks berjalan (*marquee*), dan *countdown progress bar*.
   - Riwayat lengkap notifikasi di tab **Notif** dengan lencana warna sesuai kategori/urgensi, tombol **Tes di Notch**, **Baca Semua**, dan **Bersihkan**.

3. **🎵 Media Player, Seek Bar & Lirik Sinkron (Tab `Media`)**
   - Terintegrasi langsung dengan pemutar media sistem via **MPRIS** (Linux) dan **SMTC** (Windows) — mendukung Spotify, Chrome, Firefox, VLC, dan lainnya.
   - Dilengkapi **Draggable Media Duration Seek Bar** (klik atau geser langsung untuk melompat ke durasi lagu), **Live Audio Spectrum Visualizer**, kontrol `Prev`, `Play/Pause`, `Next`, serta **Interactive Volume HUD**.
   - **Live Synchronized Lyrics** kata-demi-kata (*word-bounce* / *karaoke*) dari multi-provider (**LRCLIB**, **NetEase**, **KuGou KRC**, **Musixmatch RichSync**, dan **YouTube Auto-Captions**) lengkap dengan pengaturan *offset* sinkronisasi.

4. **⏱️ Fokus, Alarm Pengingat & Hidrasi (Tab `Fokus`)**
   - **Pomodoro Timer Kustom**: Durasi Fokus (`1–180 menit`, default `25 menit`) & Istirahat (`1–60 menit`, default `5 menit`) dengan tombol cepat (`-5m`, `-1m`, `+1m`, `+5m`), penghitung sesi selesai, dan notifikasi otomatis.
   - **Alarm Pengingat**: Atur jam dan menit alarm langsung di tab **Fokus** lengkap dengan notifikasi di Notch dan peringatan suara.
   - **Pengingat Hidrasi (Air Putih)**: Lacak target minum harian (`8 gelas / hari`, reset otomatis setiap hari baru).

5. **✅ Daily To-Do List (Tab `Todo`)**
   - Tambah, centang selesai, dan hapus daftar tugas harian langsung dari dalam Dynamic Island.

6. **📂 Rak File / File Shelf (Tab `Rak`)**
   - Simpan file sementara dengan cepat lewat tombol pilih file native OS, buka file instan, atau salin path file ke clipboard kapan saja.

7. **🕌 Jadwal Sholat & Hitung Mundur (Tab `Sholat`)**
   - Perhitungan astronomis waktu sholat standar Kemenag RI (*Imsak, Subuh, Dzuhur, Ashar, Maghrib, Isya*) sesuai **Kota / Kabupaten / Kecamatan** di seluruh Indonesia (WIB, WITA, WIT) lengkap dengan pencarian wilayah & *countdown*.

---

## 🛠️ Build Manual dari Source Code (Untuk Developer)

Pastikan toolchain **Rust (`cargo`)** sudah terpasang — tidak perlu menginstal library GTK4 atau dependensi GUI tambahan.

### Linux
```bash
git clone https://github.com/derisamedia/Arwiland.git
cd Arwiland

# Build & pasang ke menu aplikasi + autostart (~/.local):
chmod +x install.sh run.sh uninstall.sh
./install.sh

# Atau jalankan langsung di background (gunakan --fg untuk foreground):
./run.sh

# Menghapus instalasi dari ~/.local:
./uninstall.sh
```

### Windows 10 / 11 (Rust MSVC)
```powershell
git clone https://github.com/derisamedia/Arwiland.git
cd Arwiland

cargo build --release
.\target\release\arwiland.exe
```

---

## 🎮 Pintasan & Tips Penggunaan

- **Scroll Mouse di Notch**: Mengubah volume sistem secara instan dengan tampilan *Interactive Volume HUD*.
- **Klik / Geser Slider Volume & Seek Bar**: Mengatur persentase volume, klik ikon speaker untuk *Mute/Unmute*, atau geser *seek bar* untuk melompat ke bagian lagu tertentu.
- **Tombol ⚙️ (Pengaturan)**: Mengatur posisi pulau (**Atas / Bawah**), gaya (**Living Notch** vs **Floating Pill**), *Y-offset*, preset animasi pegas, sumber/gaya/offset lirik, durasi notifikasi, serta **Run on Startup**.
- **Tombol 📍 (Pin)**: Mengunci Dynamic Island agar tetap terbuka meskipun kursor mouse berpindah.
