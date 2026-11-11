# 🏝️ Arwiland — Dynamic Island & Living Notch untuk Linux (Rust + GTK4)

**Arwiland** adalah aplikasi *Dynamic Island / Living Notch* native untuk Linux yang menggabungkan fitur-fitur unggulan dari **Nusa** (`rama2555.github.io/nusa`) dan **ReyLand** (`techreyy.github.io/ReyLand`), dibangun menggunakan **Rust** dan **GTK4**.

## ✨ Fitur Utama

1. **🏝️ Compact & Expanded Dynamic Island + Live Notification Notch**
   - **Mode Compact (Kapsul/Notch)**: Menampilkan *Mini Audio Spectrum Visualizer*, jam real-time, badge jumlah notifikasi belum dibaca, serta lirik lagu kata-demi-kata atau waktu sholat berikutnya.
   - **Live Notification Toast di Notch**: Menangkap **semua notifikasi desktop Linux** (`org.freedesktop.Notifications`, `org.gtk.Notifications`, dan `org.freedesktop.portal.Notification`) serta event sistem (status cas/baterai, perubahan volume/mute, waktu sholat tiba, Pomodoro selesai, target hidrasi, & drop file ke Shelf) langsung ke dalam Notch lengkap dengan ikon kategori berdenyut, *ambient glow*, teks berjalan (*marquee*), dan *countdown progress bar*.
   - **Pusat Notifikasi (`Notif` Tab)**: Riwayat lengkap notifikasi di dalam Island dengan lencana warna sesuai kategori/urgensi, tombol **Tes di Notch**, **Baca Semua**, dan **Bersihkan**, serta opsi untuk menyembunyikan pop-up notifikasi bawaan GNOME agar tidak tampil ganda.
   - **Transisi Morphing**: Otomatis membesar (*expand*) dengan fisika pegas (*spring physics*) saat kursor diarahkan ke notch (atau diklik), dan mengecil (*collapse*) saat kursor keluar (kecuali dikunci dengan tombol **📍 Pin**).

2. **🎵 Media Player (MPRIS2 Linux) & Lirik Sinkron**
   - Terintegrasi langsung dengan pemutar media Linux melalui D-Bus MPRIS2 (Spotify, Firefox, Chrome, VLC, Amberol, Rhythmbox, dll).
   - Dilengkapi **20-Bar Live Audio Spectrum Visualizer**, kontrol `Prev`, `Play/Pause`, `Next`, slider Volume PipeWire (`wpctl`), serta kartu **Live Synchronized Lyrics**.

3. **⏱️ Fokus & Produktivitas (Pomodoro + Hidrasi + To-Do List)**
   - **Pomodoro Timer**: Mode Fokus (25 menit) & Istirahat Pendek (5 menit) lengkap dengan penghitung sesi dan notifikasi desktop (`notify-send`).
   - **Pengingat Hidrasi (Air Putih)**: Lacak target minum harian (`8 gelas / hari`) ala Nusa.
   - **Daily To-Do List**: Tambah, centang, dan hapus tugas harian. Tersimpan otomatis di `~/.config/arwiland/state.json`.

4. **📂 File Shelf (Dropzone Sementara)**
   - Tarik & lepas (*Drag & Drop*) file dari File Manager langsung ke dalam tab **File Shelf** atau gunakan tombol **➕ Pilih File**.
   - Buka file dengan cepat (`xdg-open`) atau salin path file ke clipboard kapan saja.

5. **🕌 Jadwal Sholat & Hitung Mundur (ReyLand Style)**
   - Perhitungan astronomis waktu sholat standar Kemenag RI (*Imsak, Subuh, Dzuhur, Ashar, Maghrib, Isya*) untuk 12 kota besar di Indonesia (Jakarta, Bandung, Surabaya, Yogyakarta, Semarang, Medan, Palembang, Makassar, Kendari, Denpasar, Balikpapan, Jayapura).
   - Hitung mundur (*countdown*) langsung menuju waktu sholat berikutnya.

6. **⚡ Monitor Sistem & Pengaturan Posisi**
   - Pantau penggunaan **CPU (%)**, **RAM (GB & %)**, status **Baterai / AC Power**, **Volume**, dan **Uptime** secara langsung dari kernel Linux (`/proc`).
   - Atur posisi vertikal (**Y-Offset**) untuk menyesuaikan letak notch terhadap Top Bar desktop kamu.

---

## 🚀 Cara Menjalankan

### Menggunakan Script Launcher
```bash
chmod +x run.sh
./run.sh
```

### Menggunakan Cargo (Rust)
```bash
source "$HOME/.cargo/env"
cargo run
```

### Build Versi Rilis (Optimal & Cepat)
```bash
source "$HOME/.cargo/env"
cargo build --release
./target/release/arwiland
```
