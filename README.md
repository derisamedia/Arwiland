# 🏝️ Arwiland — Cross-Platform Dynamic Island & Living Notch (Linux, Windows, macOS)

**Arwiland** adalah aplikasi *Dynamic Island & Living Notch* ringan dan cepat yang dibangun menggunakan **Rust** dan **GTK4**. Mendukung **Linux**, **Windows**, dan **macOS**.

---

## ✨ Fitur Utama

1. **🏝️ Compact & Expanded Dynamic Island + Live Notification Notch**
   - **Mode Compact (Kapsul/Notch)**: Menampilkan *Mini Audio Spectrum Visualizer*, jam real-time, badge jumlah notifikasi belum dibaca, serta lirik lagu kata-demi-kata (*word-by-word karaoke*) atau waktu sholat berikutnya.
   - **Live Notification Toast di Notch**: Menampilkan notifikasi aplikasi & sistem (status cas/baterai, perubahan volume/mute, waktu sholat tiba, Pomodoro selesai, target hidrasi, & drop file ke Shelf) langsung ke dalam Notch lengkap dengan ikon kategori, *ambient glow*, teks berjalan (*marquee*), dan *countdown progress bar*.
   - **Pusat Notifikasi (`Notif` Tab)**: Riwayat lengkap notifikasi di dalam Island dengan lencana warna sesuai kategori/urgensi, tombol **Tes di Notch**, **Baca Semua**, dan **Bersihkan**.
   - **Transisi Morphing**: Otomatis membesar (*expand*) dengan fisika pegas (*spring physics* 60/120 FPS) saat kursor diarahkan ke notch (atau diklik), dan mengecil (*collapse*) saat kursor keluar (kecuali dikunci dengan tombol **📍 Pin**).

2. **🎵 Media Player & Lirik Sinkron Multi-Provider**
   - Terintegrasi dengan pemutar media sistem (Spotify, Chrome, Firefox, VLC, dll).
   - Dilengkapi **Live Audio Spectrum Visualizer**, kontrol `Prev`, `Play/Pause`, `Next`, **Interactive Volume HUD** (bisa di-*scroll* atau digeser langsung di Notch), serta **Live Synchronized Lyrics** (mendukung LRCLIB, NetEase, KuGou KRC per-kata, Musixmatch RichSync, dan YouTube Auto-Captions).

3. **⏱️ Fokus & Produktivitas (Pomodoro + Hidrasi + To-Do List)**
   - **Pomodoro Timer**: Mode Fokus (25 menit) & Istirahat Pendek (5 menit) lengkap dengan penghitung sesi dan notifikasi otomatis.
   - **Pengingat Hidrasi (Air Putih)**: Lacak target minum harian (`8 gelas / hari`).
   - **Daily To-Do List**: Tambah, centang, dan hapus tugas harian.

4. **📂 File Shelf (Dropzone Sementara)**
   - Tarik & lepas (*Drag & Drop*) file dari File Manager/Explorer/Finder langsung ke dalam tab **Shelf** atau gunakan tombol **Pilih**.
   - Buka file dengan cepat atau salin path file ke clipboard kapan saja.

5. **🕌 Jadwal Sholat & Hitung Mundur**
   - Perhitungan astronomis waktu sholat standar Kemenag RI (*Imsak, Subuh, Dzuhur, Ashar, Maghrib, Isya*) untuk berbagai kota di Indonesia (WIB, WITA, WIT) lengkap dengan hitung mundur (*countdown*).

---

## 📦 Cara Instalasi

### Opsi A: Download Langsung (Tanpa Compile)
1. Buka tab **[Actions](https://github.com/derisamedia/Arwiland/actions)** di repositori GitHub ini.
2. Klik build terbaru yang berwarna hijau (**Cross-Platform Build**).
3. Gulir ke bagian **Artifacts** di bagian bawah halaman, lalu unduh sesuai sistem operasi kamu:
   - 🐧 **Linux**: `Arwiland-Linux-x86_64`
   - 🪟 **Windows**: `Arwiland-Windows-x86_64` (`arwiland.exe`)
   - 🍎 **macOS**: `Arwiland-macOS`

---

### Opsi B: Instalasi dari Source Code

#### 🐧 1. Linux (Ubuntu / Debian / Fedora / Arch)

**a. Pasang Dependency GTK4 & Rust:**
- **Ubuntu / Debian / Linux Mint:**
  ```bash
  sudo apt update
  sudo apt install -y git curl build-essential pkg-config libgtk-4-dev libcairo2-dev libpango1.0-dev libgdk-pixbuf-2.0-dev libglib2.0-dev libx11-dev
  ```
- **Fedora / RHEL:**
  ```bash
  sudo dnf install -y git curl gcc pkgconf-pkg-config gtk4-devel cairo-devel pango-devel gdk-pixbuf2-devel glib2-devel libX11-devel
  ```
- **Arch Linux / Manjaro:**
  ```bash
  sudo pacman -S --needed git curl base-devel gtk4 cairo pango gdk-pixbuf2 glib2 libx11
  ```

*(Jika belum menginstal Rust, jalankan: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)*

**b. Clone & Install Otomatis (1 Perintah):**
```bash
git clone https://github.com/derisamedia/Arwiland.git
cd Arwiland
chmod +x install.sh run.sh uninstall.sh
./install.sh
```
> Script `./install.sh` akan otomatis meng-compile versi rilis, memasang ikon aplikasi ke menu desktop Linux kamu, mengaktifkan fitur *Auto-Start*, dan langsung menjalankan **Arwiland**.

**Cara Uninstall di Linux:**
```bash
./uninstall.sh
```

---

#### 🪟 2. Windows (10 / 11)

1. Unduh dan instal **[MSYS2](https://www.msys2.org/)**.
2. Buka terminal **MSYS2 UCRT64**, lalu instal paket GTK4 dan Rust:
   ```bash
   pacman -S --needed git mingw-w64-ucrt-x86_64-gcc mingw-w64-ucrt-x86_64-gtk4 mingw-w64-ucrt-x86_64-pkgconf mingw-w64-ucrt-x86_64-rust
   ```
3. Clone dan jalankan **Arwiland**:
   ```bash
   git clone https://github.com/derisamedia/Arwiland.git
   cd Arwiland
   cargo run --release
   ```
   File executable siap pakai akan tersedia di `target/release/arwiland.exe`.

---

#### 🍎 3. macOS (Apple Silicon & Intel)

1. Pastikan **[Homebrew](https://brew.sh/)** sudah terpasang, lalu instal GTK4 dan Rust:
   ```bash
   brew install git rust gtk4 pkg-config cairo pango gdk-pixbuf glib
   ```
2. Clone dan jalankan **Arwiland**:
   ```bash
   git clone https://github.com/derisamedia/Arwiland.git
   cd Arwiland
   cargo run --release
   ```

---

## 🎮 Pintasan & Tips Penggunaan
- **Scroll Mouse di Notch**: Mengubah volume sistem secara instan dengan tampilan *Interactive Volume HUD*.
- **Klik / Geser Slider Volume di Notch**: Mengatur persentase volume atau klik ikon speaker di kiri untuk *Mute/Unmute*.
- **Tombol ⚙️ (Pengaturan)**: Mengatur gaya pulau (*Living Notch* vs *Floating Pill*), kecepatan pegas animasi, sumber lirik, offset sinkronisasi lirik, serta *Run on Startup*.
- **Tombol 📍 (Pin)**: Mengunci Dynamic Island agar tetap terbuka meskipun kursor mouse berpindah.
