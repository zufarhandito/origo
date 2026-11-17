# Riset: Find teks pada PDF aktif

Tanggal riset: 17 November 2026. Fokus: viewer desktop, terutama Windows.
Metode: dokumentasi resmi dan source/tes resmi yang dibaca melalui browsing.
Tidak ada uji langsung Chrome/Edge/Acrobat/Firefox dalam riset ini; versi aplikasi
terpasang tidak diverifikasi. Uji PDF Studio dicatat terpisah di [testing](../testing.md).

## Tujuan dan kebutuhan pemilik

Pemilik ingin Find seperti PDF viewer lain, hanya pada PDF aktif, dengan input di
bawah/di dalam area tab dan bar ringkas. Keputusan eksplisit setelah riset:
**default tidak case-sensitive**; `hasil penelitian` adalah satu frasa berurutan,
bukan dua kata yang dapat tersebar. Keterbatasan viewer kita bukan dasar menentukan
perilaku produk kompetitor.

## Perbandingan produk

| Produk | Perilaku yang didukung sumber | Sumber primer | Batas bukti |
|---|---|---|---|
| Chrome/Chromium | Find bar dan shortcut; tes memeriksa penghitung, hasil aktif, tombol sebelumnya/berikutnya dan fokus input | [Chrome shortcuts](https://support.google.com/chrome/answer/157179), [tes Find bar Chromium](https://chromium.googlesource.com/chromium/src/%2B/7a4bdf24167528ede7aaa43873025150078b9e70/chrome/browser/ui/views/find_bar_views_interactive_uitest.cc), [engine PDF Find](https://chromium.googlesource.com/chromium/src/%2B/main/pdf/pdfium/pdfium_engine.cc) | Bar browser dan engine PDF adalah lapisan berbeda; parameter case-sensitive pada engine bukan bukti tersedianya opsi UI |
| Microsoft Edge | Search adalah fungsi toolbar PDF; Ctrl+F membuka Find | [PDF reader](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-pdf), [shortcut Edge](https://support.microsoft.com/en-us/edge/keyboard-shortcuts-in-microsoft-edge) | Sumber ini belum menetapkan semua opsi pencocokan khusus PDF atau detail UI tiap versi |
| Adobe Acrobat | Find untuk PDF aktif menyediakan Whole words, Case sensitive, Include bookmarks/comments; Advanced Search menangani banyak dokumen | [Adobe: Searching PDFs](https://helpx.adobe.com/acrobat/using/searching-pdfs.html) | Acrobat desktop; kemampuan Replace/Advanced Search tidak otomatis masuk cakupan reader kita |
| Mozilla PDF.js | Find memiliki input, penghitung, sebelumnya/berikutnya, highlight all, case-sensitive, whole words dan diakritik; input memicu pencarian, Enter/Shift+Enter menavigasi, Esc menutup; ada pending/not-found/wrapped | [Source Find bar PDF.js](https://raw.githubusercontent.com/mozilla/pdf.js/master/web/pdf_find_bar.js) | Source upstream yang dibaca; tidak menetapkan versi Firefox terpasang atau membuktikan uji produk langsung |

## Kesimpulan dan rekomendasi

Rekomendasi hasil riset: bar Find ringkas dengan penghitung kemunculan, sorotan aktif
yang berbeda dari hasil lain, navigasi dua arah dan shortcut. Sediakan opsi kapitalisasi
dan kata utuh melalui menu kecil, seperti kemampuan yang didokumentasikan Acrobat dan
PDF.js. Rekomendasi ini tidak menyatakan bahwa keempat produk memiliki opsi identik.

Default yang dipilih untuk PDF Studio: `data` juga cocok dengan `Data` dan `database`;
opsi kata utuh mengecualikan `database`. Frasa `hasil penelitian` tidak cocok dengan
`hasil lain penelitian`. Keputusan default dan posisi bar dikunci oleh pemilik,
bukan disimpulkan sebagai default universal semua kompetitor.

## Keputusan dan penerapan

Sudah diimplementasikan: bar dalam tab, query/opsi/hasil per dokumen, default
case-insensitive, frasa, penghitung, sorotan, navigasi/wrap, Esc, opsi kapitalisasi
dan kata utuh. Lihat [perilaku aktual dan batas](../search.md).

Adaptasi engineering PDF Studio: debounce 200 ms, pencarian per halaman melalui
worker PDFium yang tersedia, normalisasi whitespace dalam satu halaman, batas
1024 karakter query/10.000 hasil, pause scan pada tab nonaktif dan token hasil usang.
Angka serta kebijakan ini adalah keputusan implementasi lokal, bukan hasil pengukuran
atau batas yang diklaim milik kompetitor. OCR, Replace, regex, lintas tab, bookmark/
komentar dan daftar hasil Advanced Search belum termasuk fitur ini.
