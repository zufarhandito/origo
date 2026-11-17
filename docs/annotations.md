# Anotator PDF

Implementasi inti, 17 November 2026. Pemilik mengotorisasi anotator terlebih dahulu;
tanda tangan draw/import kini tersedia melalui [Tanda tangan](signatures.md). Dasar UX: [riset kompetitor](research/pdf-annotations-and-signatures.md).

## Penggunaan

Dalam mode Baca, buka **Anotasi** di bawah toolbar dokumen.

- Toolbar menggunakan ikon Pilih, Highlighter, Pen, Penghapus dan Hapus, dengan tooltip.
  Klik ikon untuk mengaktifkan tool; panah di samping pen/highlighter membuka pengaturan.
- Popover berisi swatch warna, preview goresan dan slider ketebalan: pen 0,5?12 pt
  (langkah 0,5), stabilo 4?40 pt (langkah 1). Pilihan disimpan terpisah per tool/per view
  selama dokumen terbuka; perubahan pengaturan berlaku untuk goresan berikutnya.
- Toggle **Hanya teks** pada highlighter memilih sorotan mengikuti kotak teks.
  Slider nonaktif pada mode ini. Jika toggle mati, highlighter menjadi stabilo bebas
  yang juga dapat digunakan pada scan. OCR belum tersedia.
- **Pilih** lalu klik anotasi untuk memilih, mengganti warna melalui popup, atau
  **Hapus anotasi** / Delete. Penghapus menghapus satu objek utuh yang diklik.
- Satu goresan atau satu seleksi teks lintas halaman adalah satu langkah Undo/Redo.
  Esc membatalkan draft dan menutup panel anotasi. Perubahan tersimpan di Workspace
  yang sama dengan pengaturan halaman; dua tampilan satu PDF berbagi anotasi/history.

Perubahan menandai dokumen belum disimpan. Menutup dokumen/jendela memakai guard
**Simpan sebagai / Buang / Batal** yang sudah ada. Simpan sebagai wajib ke nama baru;
PDF sumber tidak ditimpa. Hanya receipt Save As penuh yang cocok dengan revisi aktif
menandai clean. Tidak ada autosave/crash recovery.

## Penyimpanan dan kompatibilitas

Output berisi objek PDF standar **Highlight** (seleksi teks) dan **Ink** (pen/stabilo),
berikut appearance stream vektor. Halaman asli tidak di-raster/flatten. Metadata milik
PDF Studio di `/NM` menyimpan model edit yang bounded. Membuka **file hasil Save As**
memulihkan model tersebut sehingga anotasi dapat dipilih/dihapus lalu disimpan lagi.
File sumber lama tetap tanpa perubahan.

Edit ulang baru didukung untuk anotasi PDF Studio dengan schema metadata v1. Ini
bukan import/editor umum anotasi Edge/Adobe/Firefox. Perubahan oleh editor lain yang
mempertahankan metadata lama belum didukung. Anotasi eksternal mengikuti pembatasan
preflight lama; jangan menjanjikan preservasi atau ekspor semua PDF beranotasi.

Save As, Extract, Split dan Optimize menangkap anotasi bersama page plan; PNG memakai
proyeksi raster yang sama dengan viewer. Source-copy Extract hanya boleh dipakai
jika anotasi masih sama dengan baseline sumber. Anotasi mengikuti PageId saat
reorder/delete/Undo dan transformasi rotasi halaman. Form/signature/encryption dan
struktur lain tetap mengikuti [kompatibilitas](compatibility.md).

Batas: 256 objek/halaman, 4096 titik/objek, 8192 titik/halaman, 65536 titik/workspace;
history maksimal 128 langkah dengan tambahan batas satu juta titik. Highlight panjang
yang melampaui batas ditolak sebagai satu transaksi. Model tidak menyimpan isi teks.

## Belum tersedia

Komentar/sticky notes, kotak teks, shape, underline/strikethrough, pindah/resize anotasi,
partial eraser, OCR, import edit anotasi pihak lain, dan dukungan sertifikat digital. Tanda tangan visual tersedia terpisah.
Kesesuaian penampilan ekspor telah diuji dengan PDFium; pengujian interaktif di
Edge/Chrome/Firefox/Acrobat belum dilakukan. Lihat [pengujian](testing.md).
