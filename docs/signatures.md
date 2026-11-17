# Tanda tangan visual

Implementasi berdasarkan [riset kompetitor](research/pdf-annotations-and-signatures.md)
dan arahan pemilik untuk melanjutkan tanda tangan setelah anotator selesai.

## Penggunaan

1. Dalam mode Baca, buka **Anotasi > Tanda tangan**.
2. Gambar di canvas dengan mouse. **Undo goresan** membatalkan goresan terakhir;
   **Hapus semua** membersihkan draft. Atau pilih **Impor gambar** untuk PNG/JPEG.
3. Klik **Gunakan tanda tangan**, lalu klik atau drag di halaman untuk menempatkan.
   Pembuatan draft tidak menandai dokumen berubah; penempatan baru mengubah dirty.
4. Dalam tool Pilih, drag tanda tangan untuk memindahkannya. Drag kotak sudut kanan
   bawah untuk resize proporsional. Garis batas memperlihatkan posisi/ukuran draft;
   gambar diterapkan saat mouse dilepas. Setiap gesture satu langkah Undo/Redo.
5. **Hapus anotasi** / Delete menghapus tanda tangan terpilih. Esc membatalkan
   penempatan/gesture; melepas mouse di luar halaman membatalkan gesture tersebut.
6. **Simpan sebagai** ke file baru. File hasil simpan dapat dibuka ulang dan tanda
   tangannya tetap dapat dipilih/digeser/di-resize/dihapus di PDF Studio.

Sumber tetap read-only. Close guard, receipt revisi, snapshot, preflight, consent,
validasi dan no-overwrite mengikuti ekspor yang sudah ada. Dua view satu PDF berbagi
objek dan history; draft/pad/selection per view.

## Gambar, kualitas dan privasi

PNG mempertahankan alpha. JPEG mempertahankan latarnya; penghapusan latar putih
belum tersedia. Orientasi EXIF diterapkan. Pinggir transparan dipotong otomatis;
isi gambar tidak dipotong berdasarkan warna. Gambar diperkecil bila perlu ke
maksimal 512 ? 256 piksel; tidak memperbesar gambar kecil. Tanda tangan gambar
akan kehilangan ketajaman jika diperbesar jauh di atas resolusi tersebut.

Input maksimal 8 MiB, dimensi maksimal 8192 per sisi, budget decode 64 MiB. Hanya
PNG/JPEG diterima; file rusak dan gambar sepenuhnya transparan ditolak. Pad maksimal
128 stroke ? 1024 titik; aset raster dibagi dengan Arc pada snapshot/history.
Maksimal delapan tanda tangan dalam model Workspace (termasuk halaman dihapus yang
masih dipertahankan untuk Undo) dan delapan saat inspeksi satu sumber.

Tidak ada upload atau pustaka template permanen. Draft hanya bertahan selama view
masih terbuka; tanda tangan yang ditempatkan tersimpan di PDF hasil ekspor. Metadata
berisi raster tanda tangan agar bisa diedit ulang, sehingga penerima file juga
menerima gambar tersebut.

## Format output dan batas

Tanda tangan disimpan sebagai anotasi PDF **Stamp** pasif, dengan appearance Form,
image RGB dan soft mask alpha. Metadata model milik PDF Studio ikut tersimpan di NM.
Konten halaman asli tidak diratakan menjadi gambar. Viewer/PNG menggunakan kompositor
alpha yang sama; Save As/Extract/Split/Optimize menangkap objek bersama page plan.
Metadata lama tanpa aset gambar tetap dibaca. Policy ekspor v6 menerima subset Stamp
milik aplikasi dengan validasi terbatas dan menolak key aktif/subtype palsu.

Ini tanda tangan visual, tanpa sertifikat kriptografis, validasi identitas atau audit
trail. Dukungan PDF yang sudah ditandatangani secara digital mengikuti pembatasan
lama. Edit Stamp pihak ketiga, SVG, ketikan nama, rotasi objek sendiri, duplicate,
gerak keyboard, template permanen, pressure sensitivity dan background removal belum
tersedia. Rotasi halaman ikut memutar tanda tangan bersama isi halaman.

Lihat [testing](testing.md) untuk hasil aktual dan batas interoperabilitas.
