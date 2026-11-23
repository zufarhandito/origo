# OCR lokal

## Mesin pengenalan

`pdf-ocr` menyediakan antarmuka mesin, bahasa, pembatalan, serta hasil berupa
kata, urutan baris, kotak ternormalisasi, dan skor kepercayaan. Mesin pertama
adalah Tesseract 5.5.4 dengan model `tessdata_fast` 4.1.0 Bahasa Indonesia dan
Inggris. Kode membatasi masukan ke 12 juta piksel, keluaran TSV ke 2 MiB, serta
hasil ke 4.096 kata dan 64 KiB teks per halaman.

Build lokal Windows menggunakan `scripts/build-ocr-sidecar.ps1`, MSVC, CMake,
dan Ninja dari toolchain proyek. Skrip memverifikasi SHA-256 sumber dan model,
menyalin lisensi, serta memeriksa versi dan daftar bahasa. Leptonica 1.87.0
ditautkan statis. Build OCR menerima PGM lewat stdin dan mengembalikan TSV
lewat stdout, dengan batas waktu dan pembatalan dari runner proses. Build ini
menonaktifkan integrasi cURL dan OpenMP. Unduhan sumber dilakukan saat setup.

Sumber upstream tersedia di [rilis Tesseract](https://github.com/tesseract-ocr/tesseract/releases/tag/5.5.4),
[rilis Leptonica](https://github.com/DanBloomberg/leptonica/releases/tag/1.87.0),
dan [model cepat](https://github.com/tesseract-ocr/tessdata_fast/tree/4.1.0).

## OCR di viewer

Klik **Butuh OCR** pada halaman scan. Origo memakai Indonesia dan Inggris
bersamaan. Saat mengantre atau berjalan, klik chip lagi untuk membatalkan.
Kegagalan menampilkan alasan dan opsi coba lagi. OCR berjalan satu halaman
per giliran di latar belakang.

Hasil masuk ke Find, seleksi teks, dan salin. Teks asli PDF diutamakan.
Kotak kata berasal dari mesin OCR dan posisi huruf di dalam kata diperkirakan.
Hasil teks mengikuti identitas halaman sumber saat halaman dipindah atau
diputar di workspace.

## Penyimpanan lokal

Origo menyimpan hasil terkompresi dalam `ocr-v1.sqlite3` di direktori state
lokal. Kunci mencakup path, ukuran dan waktu modifikasi sumber, indeks halaman
sumber, bahasa, serta versi mesin. Transaksi per halaman menjaga hasil yang
sudah selesai saat proses terputus. Origo membaca hasil sesuai kebutuhan
melalui anggaran cache teks viewer.

Cache menyimpan hingga 8.192 entri dan 192 MiB data terkompresi. Batas utama
file SQLite 256 MiB. Entri yang paling lama ditulis dibersihkan lebih dulu.
**Menu > Berkas > Hapus hasil OCR lokal** menghapus hasil lokal setelah
konfirmasi. File PDF sumber tetap read-only.

Urutan batch, tray, dan ekspor ada di
[checklist implementasi OCR](plans/OCR_IMPLEMENTATION_PLAN.md).
