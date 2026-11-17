# Handoff terkini - 17 November 2026

Pembaruan aktif: pemilik memilih nama **Origo** dan mengotorisasi pemasangan ikon
sementara lingkaran dari PNG-nya. Resource icon ID 1/metadata produk, title native,
custom title bar dan About kini Origo. Aset di apps/desktop/resources/branding;
script update-origo-icon.py memudahkan penggantian. Nama exe/state/package internal
belum dimigrasikan. Detail docs/branding.md dan hasil tes di testing.md.
Build branding `ad2f79d` sudah dipasang ke target/release/pdf-studio.exe setelah
proses pengguna tidak lagi aktif. run.cmd kini memilih build Origo berikon.
Hash sama dengan build terisolasi teruji:
3008C73397F0562A9592C6EDE332F4C0CFA8A005D975E78663EF414F0AA64283.
Branding smoke dijalankan ulang pada executable utama dan lulus:
.research/branding-ui-7ed58dbff8a4411a9adeb54f02da2354.
Sebelumnya pemasangan ditunda saat executable aktif; masalah peluncuran versi lama
kini diselesaikan tanpa menutup proses pengguna atau mengubah launcher.




Pemilik mengalihkan pekerjaan dari anotator ke tanda tangan setelah toolbar ikon
selesai. Tanda tangan visual draw/import kini diimplementasikan. Baca
[signatures](../signatures.md), [annotations](../annotations.md), [testing](../testing.md)
dan [compatibility](../compatibility.md). Task baru belum dipilih setelah fitur ini.

## Kemampuan dan batas

- Anotator: highlight teks/stabilo, pen, warna, slider pen 0.5-12 pt/stabilo 4-40 pt,
  ikon + dropdown/preview, pilih/hapus/recolor serta Undo/Redo.
- Tanda tangan: canvas mouse dengan Undo stroke/clear, PNG/JPEG lokal dengan EXIF,
  alpha PNG, crop batas transparan, preview, klik/drag penempatan, drag pindah,
  resize proporsional lewat sudut kanan bawah, hapus, Undo/Redo, Save As/reopen edit.
- Satu gesture adalah satu command. Draft tidak dirty; placement/move/resize/delete
  menandai dirty. Esc atau drop di luar halaman membatalkan gesture. Drag/resize
  menampilkan outline draft dan menerapkan gambar saat mouse dilepas.
- Objek/ID/history/revisi dibagi dua view; template draft dan tool per view. Popup
  mengunci canvas seluruh jendela agar tidak menggambar di belakangnya/panel sebelah.
- Aset maksimal 512x256 RGBA, input 8 MiB, sisi 8192 dan decode 64 MiB. Maksimal delapan
  tanda tangan dalam model (termasuk halaman dihapus yang disimpan untuk Undo).
  Tidak ada upload, template permanen, SVG, type/font, background removal, pressure,
  duplicate, gerak keyboard atau rotasi objek independen.
- Output Ink/Highlight + Stamp pasif dengan appearance Form/image/alpha soft mask;
  metadata NM milik aplikasi memulihkan model edit. Hanya anotasi milik PDF Studio
  yang didukung untuk edit ulang. Konten asli/searchable text tidak di-flatten.
- Policy v6 memperluas subset dengan Stamp. Action/subtype palsu ditolak. Certificate
  signature, form/encryption dan struktur lain tetap mengikuti pembatasan lama.
- Sumber read-only; snapshot/preflight/consent/validasi/no-overwrite dan receipt
  revisi tetap wajib. Close guard berlaku. Tidak ada autosave/crash recovery.

## Verifikasi

64 unit tests workspace lulus (25 app, 10 domain, 5 platform, 11 desktop, 13 QPDF).
Fmt dan Clippy workspace/all-targets denied warnings lulus. Native annotation/signature
smoke lulus pada source biasa dan CropBox nonzero + rotasi sumber, empat rotasi
output, alpha, editable reopen/delete, searchable text, Optimize/Split/PNG dan source
bytes. Default native suite Save As/Extract/Split/Optimize/PNG juga lulus.

UI tanda tangan menguji draw pad, placement, move/resize, gambar rusak, PNG alpha,
dua view, close guard, Save As/reopen/delete dan drop di luar halaman. UI anotator
sembilan kasus lulus. Bukti run akhir/angka terkini di testing. Interoperabilitas
interaktif Edge/Chrome/Firefox/Acrobat dan perangkat stylus/touch belum diuji.

## Git/build

Awal task `main` bersih, HEAD `9617dae`, remote origin
`https://github.com/zufarhandito/origo.git`. Riwayat anotator: `8a40b73`, handoff
`7b8a658`, toolbar `9617dae`, semuanya sudah di-push sebelumnya.
Commit fitur **46eba59**: `feat(signatures): add local drawing and image stamps`.
Push ke `origin/main` **berhasil**, respons remote `9617dae..46eba59 main -> main`.
Tidak ada force-push, binary/PDF/data privat yang dikirim. Ini bukan bukti CI remote.
Catatan SHA/push ditambahkan melalui commit dokumentasi sesudah fitur.


Build teruji dibuat di `target/tab-background-check/release`. `run.cmd` memakai
`target/release/pdf-studio.exe`; pemasangan hanya setelah mengecek tidak ada proses
pengguna memakai target. Jangan menutup proses/dokumen pengguna untuk build.
Multi-tab tahap 1-4B tetap tersedia; lihat [multitab](../multitab.md) untuk kontraknya.

Build teruji sudah disalin ke target/release setelah memastikan tidak dipakai proses.
Hash binary terisolasi/terpasang cocok:
`DE500A33722080B96E82628B5B6A928ABAEF3CCA91BBBB762101B6F5091CF946`. UI final: 11 signature + 9 anotator lulus;
path bukti dan batas ada di testing.
