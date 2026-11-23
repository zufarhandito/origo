# Tanda tangan visual

Implementasi berdasarkan [riset kompetitor](research/pdf-annotations-and-signatures.md)
dan arahan pemilik untuk melanjutkan tanda tangan setelah anotator selesai.

## Penggunaan

1. Dalam mode Baca, buka **Anotasi > Tanda tangan**.
2. Origo membuka **Galeri Template** jika pustaka memuat template. Pilih satu kartu
   untuk melihat sorotannya dan mengaktifkan **Gunakan Tanda Tangan**, atau klik
   ganda pada kartu untuk langsung masuk ke penempatan. **Buat Baru** membuka form.
   Jika pustaka kosong, Origo langsung membuka form pembuatan.
3. Pilih kategori **Tanda Tangan**, **Inisial**, atau **Stempel**. Gunakan **Gambar**,
   **Ketik**, atau **Unggah** untuk tanda tangan dan inisial. Stempel langsung
   membuka unggahan gambar. **Gunakan Tanda Tangan** masuk ke penempatan. Gambar
   tembus pandang mengikuti kursor berbentuk silang di atas halaman dan tetap di
   dalam tepinya. Kursor teks kembali saat penempatan selesai atau dibatalkan
   pada halaman yang memiliki teks.
   Klik, atau tekan lalu geser dan lepas di halaman yang sama, untuk
   menempatkannya. Pratinjau hilang di luar halaman. Tekan Esc untuk batal.
   PDF dan riwayat Undo baru berubah setelah penempatan berhasil.
4. Dalam tool Pilih, kursor tangan menandakan tanda tangan bisa digeser. Kursor
   panah diagonal menandakan sudut bisa ditarik untuk mengubah ukuran secara
   proporsional. Pegangan dan arah panah mengikuti rotasi halaman. Garis batas
   memperlihatkan posisi dan ukuran sementara. Perubahan diterapkan saat mouse
   dilepas dan dapat dibatalkan dengan satu langkah Undo.
5. Pilih tanda tangan, inisial, atau stempel yang sudah ditempatkan, lalu buka
   **Transformasi** di toolbar Anotasi. **Putar kiri 90°** dan **Putar kanan 90°**
   memutar gambar tanpa menurunkan resolusi. **Balik horizontal** menukar kiri
   dan kanan, sedangkan **Balik vertikal** menukar atas dan bawah pada tampilan
   halaman. Setiap aksi pada objek di halaman bisa di-Undo dan di-Redo.
6. **Hapus anotasi** / Delete menghapus tanda tangan terpilih. Esc membatalkan
   penempatan/gesture; melepas mouse di luar halaman membatalkan gesture tersebut.
7. **Simpan sebagai** ke file baru untuk menyimpan hasil transformasi. Setelah
   file hasil simpan dibuka ulang di Origo, tanda tangan tetap dapat dipilih,
   digeser, diubah ukurannya, ditransformasi, atau dihapus.

Sumber tetap read-only. Close guard, receipt revisi, snapshot, preflight, consent,
validasi dan no-overwrite mengikuti ekspor yang sudah ada. Dua view satu PDF berbagi
objek dan history; draft/pad/selection per view.

## Gambar, kualitas dan privasi

PNG mempertahankan alpha. JPEG mempertahankan latarnya. Penghapusan latar putih
opsional dan bisa dikembalikan. Orientasi EXIF diterapkan dan pinggir transparan
dipotong otomatis. Unggahan mempertahankan resolusi aslinya hingga 1024 piksel
per sisi dan 524.288 piksel total. Origo mengecilkan gambar yang melampaui batas
tersebut, dengan rasio aspek tetap. Unggah ulang gambar sumber atau buat ulang
tanda tangan untuk memperoleh resolusi baru pada aset yang pernah disimpan.

Input maksimal 8 MiB, dimensi maksimal 8192 per sisi, budget decode 64 MiB. Hanya
PNG/JPEG diterima; file rusak dan gambar sepenuhnya transparan ditolak. Pad maksimal
128 stroke x 1024 titik; aset raster dibagi dengan Arc pada snapshot/history.
Maksimal delapan tanda tangan dalam model Workspace (termasuk halaman dihapus yang
masih dipertahankan untuk Undo) dan delapan saat inspeksi satu sumber.

Tidak ada upload ke layanan eksternal. Pustaka template lokal tersedia secara opt-in. Draft hanya bertahan selama view
masih terbuka; tanda tangan yang ditempatkan tersimpan di PDF hasil ekspor. Metadata
berisi raster tanda tangan agar bisa diedit ulang, sehingga penerima file juga
menerima gambar tersebut.

## Penghalusan goresan

Goresan baru dari mouse dan goresan yang diterima dari HP memakai jalur yang sama. Sampel yang
sangat dekat digabung dan belokan landai dilunakkan. Titik awal dan akhir,
titik kecil, sudut yang jelas, serta pilihan warna dan ketebalan dipertahankan.
Ujung bulat dan anti-aliasing mengurangi tepi bergerigi dari hasil raster.
Goresan dirender ke 1024 × 512 piksel sebelum area kosong dipotong. Mode Ketik
merender font dengan ukuran dua kali lebih besar sebelum menyesuaikan ukuran
ke batas aset. Pratinjau penempatan dan PDF memakai raster yang dihasilkan.

## Format output dan batas

Tanda tangan disimpan sebagai anotasi PDF **Stamp** pasif, dengan appearance Form,
image RGB dan soft mask alpha. Kedua image stream meminta interpolasi dari
viewer PDF. Viewer Origo dan ekspor PNG memperhitungkan cakupan piksel dan
transparansi saat menskalakan aset. Metadata model Origo ikut tersimpan di NM.
Konten halaman asli tetap utuh. Save As/Extract/Split/Optimize menangkap objek
bersama page plan. Origo tetap membaca anotasi dan template lama. Untuk mengedit
tanda tangan dengan metadata baru, gunakan Origo versi ini atau lebih baru.
Policy ekspor v6 menerima subset Stamp milik aplikasi dengan validasi terbatas
dan menolak key aktif atau subtype palsu.

Ini tanda tangan visual, tanpa sertifikat kriptografis, validasi identitas atau audit
trail. Dukungan PDF yang sudah ditandatangani secara digital mengikuti pembatasan
lama. Edit Stamp pihak ketiga, SVG, rotasi objek sendiri, pressure sensitivity belum tersedia. Rotasi halaman ikut memutar tanda tangan bersama isi halaman.

Lihat [testing](testing.md) untuk hasil aktual dan batas interoperabilitas.

## Rencana lanjutan

[Riset iLovePDF dan roadmap UX](research/signature-ux-roadmap.md), 20 November 2026,
mencatat kandidat type/font, inisial, stempel, pustaka lokal dan QR HP.
Riset awal dibuat sebelum implementasi. Pemilik kemudian mengotorisasi tahap 1-5
untuk pemakaian individu dan lokal, termasuk QR LAN. Perilaku aktual dijelaskan
di bawah; layanan kolaborasi/request/audit/sertifikat ditunda.

## Editor dan galeri (redesign 22 November 2026)

Kategori memakai pilihan tiga kolom dan metode memakai tab bergaris dengan ikon.
Setiap metode menampilkan kontrolnya sendiri. Area gambar menyediakan tombol
Undo/Hapus di kanvas serta pilihan warna dan ketebalan di bawahnya. Mode Ketik
menampilkan pratinjau langsung dengan contoh gaya font dan swatch. Unggah menerima
satu PNG/JPEG dari pemilih file atau seret dari desktop. Sesudah impor, kanvas
berpola kotak memperlihatkan transparansi, dengan pilihan pulihkan warna latar
melalui toggle **Hapus latar putih** dan **Ganti gambar**.

Footer **Batal** dan **Gunakan Tanda Tangan** tetap terlihat saat isi digulir.
Batal menutup editor dan tidak menempatkan objek. Origo mempertahankan draft gambar,
impor, dan pengaturan latar per kategori selama view dokumen aktif.

## Ketik dan inisial (tahap 2)

Pilih Tanda tangan penuh atau Inisial lalu Ketik. Nama dan inisial punya input
terpisah; saran inisial dari huruf pertama nama dapat diedit. Pilih Caveat atau
Dancing Script serta Hitam/Biru/Merah. Font dibundel dalam binary dan berkas lisensi OFL disertakan di assets source
agar bekerja offline. Maksimal 80 karakter satu baris; glyph tidak didukung
menghasilkan error, bukan diganti kotak kosong. Preview dan output memakai aset
raster yang sama. Tidak ada font installation atau upload.

## Stempel dan kualitas (tahap 3)

Jenis Stempel memakai impor PNG/JPEG. Tombol Hapus latar putih bersifat opt-in,
klik lagi mengembalikan aset asli; gambar warna tidak otomatis direcolor.
Stempel mengikuti batas raster yang sama seperti unggahan tanda tangan. Stempel
persegi dapat memakai hingga 724 × 724 piksel, dan rasio aspek gambar dipertahankan.
Template SVG/generator stempel belum tersedia.
Draw kini memiliki warna Hitam/Biru/Merah dan ketebalan Tipis/Sedang/Tebal.
Editor scroll mengikuti tinggi jendela agar kontrol tidak hilang pada layar pendek.

## Pustaka lokal dan penempatan (tahap 4)

Centang **Simpan ke template lokal** jika ingin menyimpan saat menekan
**Gunakan Tanda Tangan**. Kolom nama baru muncul setelah dicentang. Nama kosong
diisi otomatis menurut kategori, misalnya **Tanda Tangan 1**, dengan nomor
pertama yang masih tersedia. Maksimal delapan template dengan nama
1?40 karakter disimpan di `signature-library.json` pada direktori state lokal.
File ini menyimpan gambar aset dan tidak terenkripsi.

Galeri menampilkan dua kolom kartu dengan badge kategori dan pratinjau. Dari menu
ikon pada kartu, pilih **Ubah nama** lalu Enter atau pindah fokus untuk menyimpan.
Esc membatalkan edit nama. **Hapus** membuka konfirmasi di kartu. Menghapus
template mempertahankan objek yang sudah ditempatkan pada PDF. Pustaka memakai
penulisan atomik di background, dengan batas 22 MiB. Jika file rusak, Origo
menampilkan pesan kesalahan dan mempertahankan file lama.

Pilih objek tanda tangan/stempel: Duplikat tanda tangan atau Ctrl+D membuat salinan
bergeser sedikit sebagai satu langkah Undo. Alt+panah menggeser objek terpilih
sedikit per langkah, dibatasi ke halaman. Shortcut berlaku saat canvas fokus,
sehingga mengetik pada input tidak menggeser objek. Batas delapan objek tetap.
Rotasi objek dan terapkan ke banyak halaman belum termasuk checkpoint ini.

## Gambar dari HP lewat QR LAN (tahap 5)

Dalam metode Gambar, pilih **Gambar dari HP**. Pindai QR dengan kamera HP pada
jaringan yang sama atau gunakan **Salin tautan** jika perlu membuka alamat secara
manual. **Batalkan QR** mengembalikan kanvas desktop. Tidak perlu akun, instal aplikasi HP, internet atau server relay.
Halaman lokal menyediakan pad jari/stylus, Undo, Hapus dan Kirim. Origo menerima
preview; PDF baru berubah setelah Gunakan dan placement, bukan saat gambar diterima.
PDF tidak dikirim ke HP. Aset/goresan tidak dikirim ke layanan eksternal.

Listener hanya hidup selama sesi gambar: port IPv4 dinamis pada alamat rute lokal,
token acak 192 bit, sekali pakai dan kedaluwarsa 5 menit. Batal/editor ditutup,
ganti metode/jenis atau view tidak aktif membatalkan sesi. Buat QR lagi untuk
mengulang. Host/origin, ukuran HTTP dan koordinat/goresan divalidasi; tidak ada
endpoint akses filesystem. Payload maksimal 512 KiB, 128 goresan dan 16384 titik.

Transport memakai HTTP lokal tanpa enkripsi: gunakan LAN yang dipercaya. Firewall
Windows mungkin perlu mengizinkan Origo pada jaringan privat. Guest Wi-Fi dengan
client isolation, VPN/rute NIC lain atau jaringan tanpa rute IPv4 dapat menghalangi
koneksi; pemilihan interface manual/HTTPS belum tersedia. Tidak ada kolaborasi,
request email, validasi identitas atau sertifikat digital dalam fitur ini.
