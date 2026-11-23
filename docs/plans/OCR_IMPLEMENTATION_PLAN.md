# Checklist OCR lokal Origo

OCR berjalan offline. Target awalnya teks cetak Bahasa Indonesia dan Inggris pada
PDF scan. Tahap 1–3 membentuk MVP viewer. Tahap 4–7 menambah batch, tray, dan
ekspor. Batas pembaca PDF saat ini 1.024 halaman per sumber, sehingga fixture
1.000 halaman masuk dalam batas itu. Ukur juga kasus tepat di batas.

## 1. Modul OCR dan pengukuran awal

- [x] Buat crate OCR terpisah dari GPUI. Masukan berupa gambar halaman, bahasa,
  batas waktu, dan pembatalan. Keluaran memuat teks, urutan baca, kotak kata,
  dan skor kepercayaan. Integrasi Tesseract berada di belakang antarmuka mesin.
- [x] Siapkan mesin lokal dan model `tessdata_fast` untuk Indonesia, Inggris,
  dan campuran. Verifikasi versi, integritas, serta lisensi paket Windows.
- [x] Buat fixture sintetis dengan teks acuan. Ukur waktu dan puncak RAM awal
  melalui runner uji mesin. Ukur RAM viewer gabungan pada tahap penerimaan.

**Tes.** Pengenalan Indonesia, Inggris dan campuran. Scan kosong tidak
menghasilkan teks. Scan buram, miring, dua kolom, angka dan tanda baca dinilai
terhadap referensi. Uji mesin/model hilang, keluaran rusak, timeout, pembatalan,
dan jalur file Unicode. Pastikan proses berjalan saat jaringan dimatikan.

**Bukti awal.** Teks acuan bersih tepat dan halaman kosong tetap kosong. Scan
buram dan miring contoh terbaca, sementara dua kolom sederhana terjalin per
baris. Detail waktu, RAM, dan batas uji ada di [testing](../testing.md).

## 2. OCR halaman aktif di viewer

- [ ] Aktifkan chip **Butuh OCR** untuk halaman scan. Tampilkan status
  memproses, selesai, tidak ada teks, gagal, serta tindakan batal atau coba ulang.
- [ ] Render di latar belakang, batasi ukuran raster, dan beri viewer prioritas.
  Mulai dari satu worker dan satu halaman aktif. Lepas raster setelah dipakai.
- [ ] Gabungkan teks OCR dengan jalur pencarian, seleksi, dan salin. Petakan
  koordinat OCR ke koordinat halaman dan utamakan teks bawaan PDF.

**Tes.** Cari, pilih, dan salin kata dari scan. Uji zoom, rotasi empat arah,
scroll, dan PDF campuran. Klik berulang hanya menjalankan satu pekerjaan.
Ganti tab dan tutup dokumen saat berjalan. Hasil tidak masuk ke tab atau halaman
lain. Uji halaman sangat besar dan pastikan viewer tetap bisa dinavigasi.

## 3. Cache dan identitas halaman

- [ ] Simpan hasil OCR per halaman secara atomik. Ikat entri pada fingerprint
  sumber, identitas halaman stabil, bahasa, dan versi OCR.
- [ ] Tetapkan batas disk, aturan pembersihan, dan cara menghapus hasil OCR
  lokal. Baca hasil sesuai kebutuhan dengan cache RAM terbatas.

**Tes.** Buka ulang tanpa OCR ulang. Pindah urutan halaman dan pastikan hasil
mengikuti halaman yang sama. Ganti PDF di path yang sama, ubah konfigurasi,
rusakkan satu entri, dan simulasikan disk penuh. Hasil lama yang masih valid
tetap bisa dibaca. Hash PDF sumber identik.

## 4. Batch OCR yang bisa dilanjutkan

- [ ] Tambah **OCR seluruh dokumen**. Lewati halaman dengan teks yang dapat
  dipakai atau hasil OCR valid. Antrekan halaman lain satu per satu dan
  dahulukan permintaan OCR halaman aktif setelah pekerjaan saat ini selesai.
- [ ] Simpan progres per halaman. Sediakan jeda, lanjut, batal, dan coba ulang
  halaman gagal. Tampilkan jumlah halaman siap dicari, gagal, dan tersisa.
- [ ] Tampilkan hasil pencarian bertahap dan estimasi waktu setelah cukup
  sampel. Laporkan cakupan parsial selama batch aktif.

**Tes.** Batch 1.000 halaman scan dan dokumen campuran. Tetap baca, scroll,
zoom, dan cari selama pekerjaan aktif. Jeda dan lanjut, paksa proses berhenti
di tengah halaman, lalu buka ulang. Halaman selesai tidak dikerjakan dua kali.
Satu halaman gagal tidak menghentikan halaman lain. Uji duplikasi batch,
prioritas halaman aktif, sumber berubah, dan disk penuh.

## 5. Tray Windows untuk batch

- [ ] Sediakan **Sembunyikan ke tray** saat batch aktif. Minimize biasa tetap
  membiarkan batch berjalan. Tray menampilkan status dan aksi **Buka Origo**,
  **Jeda/Lanjutkan**, serta **Keluar**.
- [ ] Gunakan alur konfirmasi perubahan dokumen saat benar-benar keluar.
  Hentikan worker, simpan checkpoint, lalu bersihkan ikon tray.

**Tes.** Sembunyikan jendela, periksa progres bertambah, jeda dan lanjut dari
tray, lalu buka jendela yang sama. Uji keluar saat pekerjaan berjalan dan buka
ulang untuk melanjutkan. Pastikan tidak ada ikon atau worker tertinggal. Uji
jendela punya perubahan PDF yang belum disimpan.

## 6. Simpan salinan PDF yang bisa dicari

- [ ] Tambahkan ekspor lapisan teks tak terlihat dengan koordinat yang tepat.
  Jelaskan cakupan jika hasil OCR masih parsial. Lewati halaman yang telah
  memiliki teks yang dapat dipakai.
- [ ] Pakai alur Simpan Sebagai dengan validasi dan no-overwrite. Pertahankan
  urutan, ukuran, rotasi, dan tampilan halaman.

**Tes.** Cari dan salin hasil di Origo serta viewer PDF lain. Bandingkan render
halaman sebelum dan setelah ekspor. Uji rotasi, PDF campuran, hasil parsial,
batal, tujuan sudah ada, dan hash PDF sumber.

## 7. Uji penerimaan dan penyelesaian

- [ ] Ukur durasi, puncak RAM gabungan, ukuran cache, dan responsivitas viewer
  pada Windows dengan fixture 1, 100, 1.000, dan 1.024 halaman. Uji penolakan
  dokumen di atas batas dengan pesan yang jelas.
- [ ] Jalankan batch panjang pada laptop target, termasuk minimize, tray,
  restart, dan ekspor. Tetapkan batas performa berdasarkan hasil pengukuran.
- [ ] Catat hasil dan batas dukungan aktual, lalu commit dan push setiap tahap
  yang selesai sesuai aturan repo.
