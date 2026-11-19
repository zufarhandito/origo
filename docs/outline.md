# Daftar isi PDF

Pembaruan 19 November: pemicu Panel navigasi adalah ikon paling kiri toolbar utama.
Pemilih Thumbnail/Daftar isi memakai dua tombol ikon GPUI-kit dengan tooltip,
bukan segmented bertulisan; state per view,
klik bab, expand/collapse dan model outline tidak berubah. Saat Organize,
Thumbnail ditandai aktif sesuai konten yang sedang ditampilkan.


Implementasi navigasi MVP, 19 November 2026. Dasar keputusan dan perbandingan vendor:
[riset outline/bookmarks](research/pdf-outline-navigation.md).

Buka **Panel navigasi** di kiri, lalu ikon **Daftar isi** di sebelah ikon
**Thumbnail**. Nama muncul saat hover; ikon aktif diberi latar pilihan.
Bab bertingkat dapat dibuka/ditutup lewat panah; klik judul untuk menuju tujuan lokal.
Zoom pembaca dipertahankan. Destination halaman dan posisi XYZ/FitH didukung;
mode destination lain menuju halaman tanpa memaksakan zoom. Direct dan named
local destination dibaca lewat PDFium. CropBox dan rotasi sumber/virtual diperhitungkan.

Daftar ini berasal dari outline/bookmarks PDF, bukan tulisan daftar isi di halaman.
PDF tanpa struktur tersebut menampilkan pesan kosong; tidak ada inferensi teks/OCR.
Parent tanpa tujuan tetap dapat dibuka untuk melihat anak. External URL/file,
JavaScript, Launch dan aksi nonlokal tidak dijalankan; tujuan yang tidak tersedia
dinonaktifkan. Pembacaan/navigasi tidak mengubah source, dirty, atau Undo.

Setelah reorder/delete/import, tujuan dipetakan dari sumber + halaman asli ke PageId
workspace, bukan nomor posisi lama. Halaman yang dihapus menjadi nonaktif; Undo
mengaktifkannya kembali. Workspace gabungan mengelompokkan outline menurut sumber.
Sidebar, pilihan bab, expand/collapse dan cache metadata bersifat **per view**;
dua split view dapat membaca bab berbeda. Ini adaptasi MVP dari rekomendasi cache
bersama pada riset. Native PDFium tetap lewat worker terserialisasi, tanpa native
handle melewati thread. Hasil worker dengan workspace/fingerprint lama diabaikan.

Daftar dirender virtual. Pembacaan dibatasi 4096 node/sumber, kedalaman 64,
256 karakter/judul dan sekitar 256 KiB teks/sumber; cycle/limit dilaporkan.
Cache satu view dibatasi 16384 node / 2 MiB judul. Indent visual dibatasi delapan
level, sementara hierarki tetap dipertahankan. Struktur rusak dapat menghasilkan
pesan gagal/sebagian dibatasi. Membuka otomatis dari PageMode, penyimpanan state
sidebar antar sesi, authoring bookmark dan dukungan tree-keyboard khusus belum ada.

Fitur ini **tidak** menambahkan preservation outline pada Save As/Extract/Split.
Consent kehilangan bookmark dan pembatasan ekspor yang ada tetap berlaku.
Lihat [pengujian](testing.md) dan [kompatibilitas](compatibility.md).
