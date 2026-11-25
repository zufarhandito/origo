# Daily PDF Utilities (Offline)

Empat utilitas PDF harian tersedia di menu **Alat** dan bekerja 100% lokal (offline) tanpa mengganti file asli:

## 1. Nomor Halaman (`Alat → Nomor Halaman…`)

- Menyisipkan nomor halaman pasif (`/FreeText` dengan font lokal IBM Plex Sans) pada semua halaman atau rentang khusus (misalnya `2-10` untuk melewati halaman sampul).
- Format: `1`, `1 / N`, atau `Halaman 1 dari N`.
- Posisi: 6 preset (Kiri/Tengah/Kanan × Atas/Bawah), nomor awal kustom, ukuran 10–16 pt, dan pilihan warna.
- Diterapkan dalam **1 langkah Undo/Redo** (`Ctrl+Z` / `Ctrl+Y`) via `Command::AnnotateMany`.

## 2. Watermark Teks (`Alat → Watermark…`)

- Menyisipkan watermark teks semi-transparan (preset `RAHASIA`, `DRAFT`, `SALINAN`, `LUNAS`, atau teks bebas hingga 64 karakter).
- Pilihan sudut (`-45° Diagonal`, `0° Mendatar`, `45° Diagonal`), opasitas (`15%`, `25%`, `40%`, `60%`), ukuran (`Kecil`, `Sedang`, `Besar`), warna, dan target halaman.
- Seluruh penempatan berbagi **1 aset gambar stamp yang sama** (deduplikasi `/XObject` dan `/NM` ref) sehingga ukuran PDF hasil tetap efisien, dan dapat dibatalkan sekaligus dengan `Ctrl+Z`.

## 3. Lindungi PDF dengan Password (`Alat → Lindungi PDF (Password)…`)

- Enkripsi PDF lokal **AES-256** menggunakan sidecar `qpdf 12.4.2`.
- Password dikirim hanya lewat file manifest `--job-json-file` sementara yang langsung dihapus setelah proses selesai (tidak pernah muncul di argumen command line).
- Mendukung password buka PDF, password pemilik/izin opsional, serta pengaturan izin dokumen (cetak, salin teks, ubah dokumen).
- Menulis ke file PDF baru secara atomik (*no-clobber*); file sumber tetap utuh.

## 4. Buka Kunci PDF (`Alat → Buka Kunci PDF…`)

- Menghapus proteksi password PDF terenkripsi ke file baru secara lokal (bekerja meskipun belum ada tab PDF yang terbuka).
- Memverifikasi password secara deterministik (`--requires-password`), memvalidasi PDF hasil (`--is-encrypted` & `--check`), dan menyediakan opsi membuka hasil langsung di tab Origo.

## Verifikasi

```powershell
. .\scripts\use-build-environment.ps1
cargo test -p qpdf-adapter -p pdf-application --locked --offline
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\daily-utilities-ui-smoke.ps1 -Profile debug
```
