# Riset: cetak PDF dari Origo

Tanggal akses: 18 November 2026. Platform target Windows desktop; repo `3fac8d8`.
Pemilik meminta fitur cetak serta riset [panel daftar isi](pdf-outline-navigation.md).
Tahap ini hasil riset/rekomendasi, belum implementasi cetak. Publikasi ditunda.

## Metode dan fakta kompetitor

Dokumentasi resmi dan source dibaca; tidak mengoperasikan printer atau UI vendor.
Help vendor tidak menjamin semua opsi tersedia pada setiap printer/driver/versi.

| Produk | Alur dan pengaturan yang terverifikasi | Batas bukti |
|---|---|---|
| Edge desktop | Ctrl+P membuka cetak; help mencakup PDF, printer, pages/copies, layout, ukuran kertas dan scale. Ada system dialog, Windows Ctrl+Shift+P; preferensi tambahan bergantung driver. [E1] | Bagian help yang hanya membahas webpage, header/footer atau print selection teks tidak digeneralisasi ke PDF. Print preview bisa dipengaruhi policy. [E2] |
| Chrome desktop | File > Print / Ctrl+P, memilih destination/settings, lalu Print. [C1] | Help umum ini bukan bukti semua opsi khusus PDF atau hasil unsaved markup. Jangan mengarang default print annotations Chrome. |
| Firefox desktop | Tombol printer pada toolbar PDF membuka print setup. [F1] | Print service PDF.js generik merender dengan intent print dan printAnnotationStorage; ini source standalone, bukan bukti jalur Gecko/driver identik. [F2] |
| Acrobat Reader desktop | Toolbar/File > Print, printer/copies, All/Current/range, preview, sizing, orientation dan Comments & Forms. Ada grayscale dan duplex jika driver mendukung. [A1] | Dukungan driver menentukan duplex. Fitur lanjutan/properti Acrobat Pro tidak disamakan dengan Reader. |

Acrobat juga membedakan Document, Document and Markups, dan Document and Stamps.
Artinya dukungan pencetakan anotasi harus eksplisit; tampil di viewer belum merupakan
bukti semua jenis anotasi ikut tercetak. [A2]

## Rekomendasi MVP Origo (usulan)

1. Ikon printer pada toolbar, menu Berkas > Cetak dan Ctrl+P. Target adalah dokumen
   aktif; pada dua view satu dokumen, current page mengikuti view yang memanggil.
2. Preview yang sesuai snapshot: seluruh halaman/current page/range (mis. 1-3,5),
   copies, paper size, portrait/landscape, **Fit ke area cetak** / **Ukuran asli 100%**.
   Default usulan Fit agar tidak terpotong oleh margin printer; bukan klaim default
   semua kompetitor. Range berarti posisi dalam susunan Workspace saat dicapture.
3. Sertakan highlight, pen/stabilo dan tanda tangan yang ditempatkan, termasuk yang
   belum Save As. Find highlight, text selection biru, handle dan draft tidak dicetak.
   Opsi mengecualikan anotasi bisa ditambahkan nanti; perilaku harus tertulis jelas.
4. Paper size, color/grayscale dan duplex lewat properti driver jika tersedia.
   Poster/booklet/N-up/custom scale/odd-even tidak wajib pada tahap pertama.
5. Preview/cancel tidak mengirim job. Klik Cetak mengirim snapshot; jangan membuat
   clean atau meminta Save As hanya untuk mencetak. Pesan sukses **Dikirim ke antrean
   printer**, bukan klaim kertas sudah keluar. Cancellation setelah spool dapat
   meninggalkan halaman yang telanjur dicetak; jangan menjanjikan rollback fisik.
6. UI tetap responsif, progres persiapan per halaman, resource bounds dan cleanup.
   Tidak menjalankan print diam-diam saat membuka PDF.

## Pemetaan repo dan pilihan teknis

- Saat ini tidak ada command Ctrl+P, UI printer maupun adapter spooler. PDFium worker
  dapat render_exact, dan kompositor anotasi/tanda tangan serta snapshot ExportPlan
  sudah tersedia. Ini reuse lokal, bukan bukti implementasi cetak selesai.
- Screenshot viewport tidak cukup: resolusinya mengikuti zoom dan bisa memuat UI.
  Cetak butuh raster terpisah sesuai ukuran fisik/DPI dengan white background, crop,
  source/virtual rotation, annotation snapshot dan printable area printer. Print
  intent serta flag printable/hidden anotasi sumber harus diuji; render viewer biasa
  belum membuktikan semantik print flags benar.
- Rute raster per halaman lewat Windows printer adapter adalah kandidat MVP yang
  paling selaras dengan kompositor sekarang. Tradeoff: teks cetak menjadi raster;
  virtual Print to PDF dapat kehilangan searchable text dan memperbesar file.
  Direct/vector PDFium-to-HDC perlu spike terpisah dan pengujian parity markup.
- Windows PrintDlgEx menyediakan property sheet dan page ranges; spooler mengelola
  job. Dokumentasi UI native tidak berarti GPUI pin kita telah memiliki API cetak.
  [W1][W2] Tidak menyamakan PrintDlgEx dengan print preview penuh milik browser.
- Repo memakai `unsafe_code = forbid`. Win32/HDC langsung butuh adapter safe atau
  bridge terisolasi yang ditinjau; jangan diam-diam menurunkan lint workspace.
  Spike harus menentukan cara modal owner, handle lifetime, thread/COM, device
  settings, driver capabilities dan cleanup sebelum memilih binding/bridge.
- Membuka file sumber lewat shell/default viewer sebagai satu-satunya jalur akan
  melewatkan perubahan Workspace yang belum disimpan dan bergantung aplikasi lain.
  Tidak direkomendasikan sebagai implementasi inti.
- Jangan memaksakan preflight struktural ekspor sebagai definisi semua PDF yang
  boleh dicetak: print intent dan preservation berbeda. Namun permission/unsupported
  semantics sumber harus dinilai dan dikomunikasikan secara khusus.
- Virtual printer seperti Microsoft Print to PDF memerlukan tes tambahan pada
  save dialog, cancellation dan sumber yang dipilih sebagai output. Guard source
  read-only/no-overwrite tidak boleh diklaim berlaku otomatis pada dialog milik
  driver. Tentukan rute output yang menjaga sumber sebelum mendukungnya secara penuh.
- Spooler mencetak di luar transaksi file aplikasi. Safe Save As PDF dan Print to
  PDF bukan operasi identik, dan print tidak boleh mengubah receipt/baseline.

## Acceptance dan spike berikutnya

1. Enumerate printer/dialog tanpa mencetak; cancel, tanpa printer, offline/error.
2. Snapshot order, current page, range valid/invalid, copies dan multi-source plan;
   source CropBox/rotation, mixed page sizes, A4/Letter dan margin/nonprintable area.
3. Anotasi/tanda tangan alpha ikut, draft/Find/UI tidak ikut. Preview dan output cocok;
   sumber tidak berubah, dirty tetap; edit setelah snapshot tidak mengubah job.
4. Pengiriman/abort job yang hanya dimiliki aplikasi, progres/error, batas memori/DPI,
   user cancellation dan cleanup. Jangan membatalkan antrean/dokumen pengguna lain.
5. Virtual printer dengan file uji baru; hash source dan no-overwrite. Driver-specific
   save dialog merupakan acceptance tersendiri, bukan asumsi dari Save As.
6. Printer fisik perlu observasi pengguna/perangkat untuk margin/skala/warna/duplex;
   virtual output lulus tidak membuktikan cetak fisik atau semua driver.

Urutan usulan: spike adapter Windows + satu halaman sintetis, kemudian snapshot +
preview/settings, baru regression multi-page/annotations dan acceptance fisik.
Belum ada printer, driver, dependency atau pengaturan sistem yang diubah pada riset.

## Sumber primer

[E1]: https://support.microsoft.com/en-us/edge/print-in-microsoft-edge
[E2]: https://learn.microsoft.com/en-us/deployedge/microsoft-edge-browser-policies/usesystemprintdialog
[C1]: https://support.google.com/chrome/answer/1069693?hl=en
[F1]: https://support.mozilla.org/en-US/kb/view-pdf-files-firefox-or-choose-another-viewer
[F2]: https://raw.githubusercontent.com/mozilla/pdf.js/master/web/pdf_print_service.js
[A1]: https://helpx.adobe.com/reader/desktop/print-pdfs.html
[A2]: https://helpx.adobe.com/acrobat/desktop/print-documents/set-up-and-print-pdfs/print-settings.html
[W1]: https://learn.microsoft.com/en-us/windows/win32/dlgbox/print-property-sheet
[W2]: https://learn.microsoft.com/en-us/windows/win32/printdocs/printing-and-print-spooler-functions
