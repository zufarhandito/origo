# Aturan kerja PDF Studio

- Berkomunikasi dengan pemilik dalam bahasa Indonesia. Mulai dari
  [indeks dokumentasi](docs/README.md) dan [handoff terkini](docs/handoff/PROJECT_HANDOFF.md).
- Ikuti [panduan riset dan penyelesaian fitur](docs/feature-development.md).
  Saat diminta research fitur, teliti perilaku produk kompetitor terlebih dahulu:
  Adobe Acrobat/Reader, Mozilla Firefox/PDF.js, Chrome PDF dan Edge PDF sesuai relevansi.
  Jangan menganggap kemampuan/keterbatasan aplikasi ini sebagai bukti perilaku kompetitor
  atau sebagai dasar tunggal menentukan UX. Sertakan sumber primer dan pisahkan fakta,
  pengamatan langsung, rekomendasi, serta hal yang belum terverifikasi.
- Setelah implementasi fitur yang diotorisasi selesai, tes relevan lulus dan dokumentasi
  diperbarui, **buat commit Conventional Commits lalu push**. Ini instruksi berkelanjutan
  pemilik, termasuk fitur pencarian yang sudah selesai. Tujuan repo saat instruksi dicatat:
  `origin` = `https://github.com/zufarhandito/origo.git`, branch `main`.
  Verifikasi remote/branch sebelum push; jangan force-push atau menimpa perubahan orang.
  Instruksi commit/push tidak mengizinkan implementasi permintaan yang masih plan-only,
  publikasi binary, atau pengiriman PDF/data privat.
- Pertahankan perubahan pengguna; stage hanya file/hunk terkait pekerjaan yang ditinjau.
  Catat SHA dan hasil push sebenarnya; jangan menyatakan sukses jika gagal/ditolak.
- PDF sumber tetap read-only; pertahankan snapshot/preflight/consent/validasi/no-overwrite.
  `.research`, PDF pengguna, `.tools`, `sidecars`, `target` dan `dist` tetap lokal/ignored.
  Jangan membaca catatan privat atau mengunggahnya. Jangan menghapus/memindahkan toolchain
  terdaftar atau menutup proses/dokumen pengguna untuk build maupun tes.
