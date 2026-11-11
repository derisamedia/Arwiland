# 🏝️ Arwiland — Cross-Platform Dynamic Island & Living Notch (Windows, macOS, Linux)

**Arwiland** adalah aplikasi *Dynamic Island & Living Notch* ringan dan cepat yang dibangun menggunakan **Rust** dan **GTK4**. Mendukung **Windows (10/11)**, **macOS**, dan **Linux**.

---

## 📥 Download & Instalasi Langsung (Tanpa Perlu MSYS2 / Homebrew / Rust)

Kamu **tidak perlu** menginstal MSYS2, Homebrew, ataupun Rust. Cukup unduh file installer siap pakai dari halaman **[Releases](https://github.com/derisamedia/Arwiland/releases/latest)** (atau tab **[Actions](https://github.com/derisamedia/Arwiland/actions)**):

| Sistem Operasi | File Installer | Cara Pasang |
| :--- | :--- | :--- |
| 🪟 **Windows 10 / 11** | **[`Arwiland-Windows-Setup.exe`](https://github.com/derisamedia/Arwiland/releases/latest)** | 1. Klik 2x **`Arwiland-Windows-Setup.exe`**.<br>2. Klik **Next -> Install**.<br>3. Otomatis membuat shortcut di Desktop/Start Menu, **otomatis berjalan saat Windows dinyalakan (Auto-Start)**, dan langsung menempel di bagian atas layar! |
| 🍎 **macOS** | **[`Arwiland-macOS.dmg`](https://github.com/derisamedia/Arwiland/releases/latest)** | 1. Buka file **`Arwiland-macOS.dmg`**.<br>2. Tarik (**Drag & Drop**) ikon **`Arwiland.app`** ke folder **`Applications`**.<br>3. Buka **Arwiland** dari Launchpad / Applications. |
| 🐧 **Linux (Ubuntu / Debian / Mint)** | **[`Arwiland-Linux-amd64.deb`](https://github.com/derisamedia/Arwiland/releases/latest)** | Klik 2x file `.deb` atau jalankan di terminal:<br>`sudo apt install ./Arwiland-Linux-amd64.deb` |

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

## 🛠️ Build Manual dari Source Code (Opsional / Untuk Developer)

Jika kamu ingin memodifikasi kode sumber atau melakukan build sendiri:

```bash
git clone https://github.com/derisamedia/Arwiland.git
cd Arwiland

# Untuk Linux (otomatis compile, pasang ke menu aplikasi & autostart):
chmod +x install.sh run.sh uninstall.sh
./install.sh
```

---

## 🎮 Pintasan & Tips Penggunaan
- **Scroll Mouse di Notch**: Mengubah volume sistem secara instan dengan tampilan *Interactive Volume HUD*.
- **Klik / Geser Slider Volume di Notch**: Mengatur persentase volume atau klik ikon speaker di kiri untuk *Mute/Unmute*.
- **Tombol ⚙️ (Pengaturan)**: Mengatur gaya pulau (*Living Notch* vs *Floating Pill*), kecepatan pegas animasi, sumber lirik, offset sinkronisasi lirik, serta *Run on Startup*.
- **Tombol 📍 (Pin)**: Mengunci Dynamic Island agar tetap terbuka meskipun kursor mouse berpindah.
