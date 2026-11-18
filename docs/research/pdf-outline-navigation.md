# Riset: panel daftar isi PDF

Tanggal akses: 18 November 2026. Target Origo Windows desktop. Dasar repo `3fac8d8`.
Permintaan: pemilik ingin panel daftar isi seperti Edge, dibuka di sebelah kiri.
Tahap ini riset; implementasi belum dilakukan. Cetak menjadi pekerjaan produk yang
juga diminta; lihat [riset cetak](pdf-printing.md). Publikasi/installer ditunda.

## Metode dan istilah

Fakta di bawah berasal dari help resmi dan source upstream yang dibaca, bukan uji
UI/browser terpasang. Dokumentasi vendor tidak mengunci versi binary. Source Chromium
main/PDF.js master dapat berubah; sidebar PDF.js tambahan dibaca pada tag v5.4.394.
Belum ada pengamatan langsung terhadap build vendor atau PDF privat pengguna.

Panel tersebut adalah PDF document outline/bookmarks: pohon judul yang menunjuk
ke destination. Halaman teks berjudul Daftar Isi, link di atas halaman, outline
tertancap, dan tag aksesibilitas merupakan struktur berbeda. Kehadiran halaman
Daftar Isi tidak menjamin ada outline. Panel bukan OCR atau pembuatan daftar isi
otomatis. Bintang favorit dokumen Origo juga berbeda dari PDF bookmarks. [T1][A1]

## Perbandingan fakta bersumber

| Produk | Perilaku terverifikasi | Batas bukti |
|---|---|---|
| Edge desktop | Ikon Table of contents di sisi kiri toolbar membuka panel daftar bab/subbab; klik label menuju bagian, panel tetap terbuka sampai ditutup. [E1][E2] | Help tidak menetapkan semua default auto-open, highlight item saat scroll, perilaku outline rusak, atau shortcut spesifik. |
| Chrome/Chromium desktop | Sidebar memiliki tab thumbnail dan outline; tab outline ditambahkan hanya ketika daftar bookmark tidak kosong. Tree memiliki indent, expand/collapse terpisah dari navigasi, Enter/Space; destination bisa memuat halaman, XY dan zoom. [C1][C2] | Ini pembacaan source, bukan uji rilis Chrome. Source juga mendukung URI; bukan rekomendasi menyalinnya ke Origo. |
| Firefox/PDF.js | Sidebar v5.4.394 memiliki thumbnail/outline dan tab lain, outline nonaktif saat kosong. Outline viewer mendukung hierarchy, toggle tree dan navigasi destination. [F1][F2] | Rilis Firefox yang terpasang tidak diuji; kode PDF.js standalone dan integrasi Firefox tidak boleh disamakan seluruhnya. |
| Acrobat desktop | Bookmarks memiliki destination, hierarchy yang dapat expand/collapse, serta fitur authoring. Destination dapat berada di dokumen sama, dokumen lain, URL, atau action. [A1] | Lokasi panel berbeda menurut UI/versi; tidak menyimpulkan selalu di kiri. Membuat/edit bookmarks di Acrobat bukan otomatis fitur Reader gratis. |

## Rekomendasi UX Origo (usulan)

- Gunakan sidebar kiri yang sudah ada dengan tab **Thumbnail | Daftar isi**.
  Tombol sidebar dan menu Tampilan dapat membuka/menutupnya. Tidak menambah drawer
  kanan. Pembacaan outline tidak mengubah dirty/history.
- Tampilkan tree bertingkat: panah membuka/menutup anak, klik judul menuju target;
  parent tanpa destination tetap bisa dibuka. Judul panjang memakai tooltip/wrap
  yang dibatasi, bukan memperlebar seluruh jendela.
- Jika tidak ada outline, beri empty state **PDF ini tidak memiliki daftar isi
  navigasi**. Ini pilihan UX Origo; Chrome menyembunyikan tab outline kosong,
  PDF.js menonaktifkan tombolnya. Jangan menjanjikan deteksi otomatis dari teks.
- MVP menuju halaman dan posisi jika destination valid; pertahankan zoom pembaca
  sebagai rekomendasi awal. Chromium dapat menerapkan zoom bookmark, sehingga
  keputusan mempertahankan zoom adalah adaptasi kita, bukan perilaku universal.
- UI sidebar/expanded nodes/selected item per view. Dua view satu dokumen boleh
  membuka bab berbeda. Pembacaan model outline dibagi dan tidak menggandakan scan.
- Navigation-only dalam dokumen aktif. External file/URL/JavaScript/Launch/embedded
  attachment tidak dieksekusi. Item tanpa target valid menampilkan alasan, bukan
  menjalankan action. Pembatasan ini rekomendasi produk Origo, lebih sempit dari vendor.
- Default awal tetap thumbnail; pengguna memilih Daftar isi. Auto-open mengikuti
  PDF PageMode/persistensi sesi, sinkronisasi bab aktif saat scroll, pencarian judul,
  edit/tambah/hapus bookmark, serta preservasi outline output adalah tahap terpisah.

## Pemetaan ke repo: inspeksi lokal

- `pdfium-render 0.9.5` pin memiliki document.bookmarks(), root(), title(), action(),
  destination(), iter_direct_children() dan iterator dengan visited-set. Destination
  menyediakan page index/view. API upstream menyatakan caller harus menangani cycle.
  Ini bukti API tersedia, belum spike binary pin membaca semua bentuk destination.
- [Worker PDFium](../../crates/pdf-engine/src/lib.rs) saat ini hanya Inspect,
  InspectOutput, Render, RenderExact dan Text. Perlu operasi Outline baru; handle
  native tidak boleh keluar worker. Kembalikan DTO berisi ID/title/parent/target.
- [Sidebar](../../apps/desktop/src/views.rs) saat ini khusus thumbnail; toolbar di
  chrome.rs menyebut Panel thumbnail. Perlu mode sidebar per view dan tree bounded.
- Target jangan disimpan sebagai indeks viewport. Petakan source page ke PageId
  stabil agar reorder tidak salah bab. Setelah halaman dihapus, target menjadi
  tidak tersedia dan pulih ketika Undo; parent tetap memuat anak yang masih valid.
- Dokumen multi-source: kelompokkan outline menurut sumber, lalu remap ke halaman
  Workspace yang masih ada. Outline tidak sama dengan daftar isi gabungan baru.
- [Preflight bookmarks](../../crates/qpdf-adapter/src/preflight/bookmarks.rs) saat
  ini mengenali navigation-only sebelum membuangnya dengan consent. **Membaca
  outline di sidebar tidak otomatis membuat writer mempertahankannya.** Save As/
  Extract tetap mengikuti consent yang ada; whole-source Extract tertentu memakai
  byte-copy. Preservasi/remap outline dalam output membutuhkan pekerjaan tersendiri.
- Usulan resource bounds awal: 16.384 node, depth 64, judul maksimal 1.024 karakter
  dan total teks 1 MiB; deteksi cycle, partial/error state eksplisit, virtualized rows.
  Angka ini rekomendasi engineering, belum hasil profil/keputusan implementasi.

## Acceptance sebelum dinyatakan selesai

1. Flat/nested/multi-level, Unicode, judul kosong/panjang, parent tanpa destination.
2. Explicit/named destinations, halaman 0 vs label halaman, posisi XYZ/FitH, source
   rotation/CropBox dan zoom; jangan mencampur page label dengan indeks fisik.
3. PDF tanpa outline, teks daftar isi tanpa outline, scan dengan outline, outline
   rusak/cycle/depth besar; satu item gagal tidak membuat UI macet.
4. Keyboard/fokus/expand tidak memicu navigasi tak sengaja, aksesibilitas tree,
   ukuran panel sempit dan dua view/dokumen berbeda.
5. Reorder/delete/import/Undo, stale worker/source changed; tidak dirty, tidak menulis
   source dan tidak menjalankan external action. Ekspor tetap memakai consent lama.

## Sumber primer

[E1]: https://learn.microsoft.com/en-us/deployedge/microsoft-edge-pdf#table-of-contents
[E2]: https://www.microsoft.com/edge/features/pdf-reader
[C1]: https://raw.githubusercontent.com/chromium/chromium/main/chrome/browser/resources/pdf/elements/viewer_pdf_sidenav.ts
[C2]: https://raw.githubusercontent.com/chromium/chromium/main/chrome/browser/resources/pdf/elements/viewer_bookmark.ts
[F1]: https://raw.githubusercontent.com/mozilla/pdf.js/v5.4.394/web/pdf_sidebar.js
[F2]: https://raw.githubusercontent.com/mozilla/pdf.js/master/web/pdf_outline_viewer.js
[A1]: https://helpx.adobe.com/acrobat/using/page-thumbnails-bookmarks-pdfs.html
[T1]: https://pdfium.googlesource.com/pdfium/+/refs/heads/main/public/fpdf_doc.h

Sumber lokal API: .tools/cargo/registry/src/.../pdfium-render-0.9.5/src/pdf/document/
bookmarks.rs, bookmark.rs dan src/pdf/destination.rs. Toolchain tetap lokal/ignored.
