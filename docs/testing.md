# Pengujian Origo

## OCR halaman aktif dan cache lokal (23 November 2026)

Smoke `pdf-smoke --ocr-viewer-smoke .research/ocr-fixtures` mengenali PDF scan
sintetis dari path Unicode. Pengujian memeriksa Find, seleksi dan salin, kotak
teks pada empat orientasi, teks native di PDF campuran, cache setelah restart
koneksi, serta kesamaan byte PDF sumber. Tes cache memeriksa perbedaan
fingerprint dan bahasa, entri korup, serta penghapusan lokal.

Fmt, Clippy modul OCR/desktop/smoke, 3 tes OCR, 5 tes platform, dan build
release lulus. Smoke UI Windows memverifikasi chip, pembaruan Find yang sudah
terbuka, teks native di PDF campuran, buka ulang dari cache, dan hash sumber.
Bukti lokal `.research/ocr-ui-243b1ab9fc9348918288d093e608833e`.

## Baseline mesin OCR lokal (23 November 2026)

Format, Clippy, 2 tes modul OCR, 5 tes platform, dan smoke Tesseract pada Windows
lulus. Gunakan `scripts/new-ocr-fixtures.ps1`, lalu
`pdf-smoke --ocr-engine-smoke .research/ocr-fixtures`.

Pada gambar sintetis A4 300 DPI, runner debug memberi hasil tepat untuk tiga
fixture teks bersih dan nol kata untuk gambar kosong. Contoh buram dan miring
terbaca. Urutan dua kolom sederhana terjalin per baris, sehingga akurasi layout
dua kolom masih terbatas. Versi lokal diuji dengan proxy yang tidak dapat
dihubungi. Build tidak menyertakan cURL. Koneksi Windows mesin uji tetap aktif.

Waktu dari raster siap hingga hasil OCR berada di kisaran 1,16–1,56 detik per
halaman pada mesin uji ini. Puncak working set runner debug 48,9 MiB dan puncak
worker yang teramati 66,4 MiB, masing-masing diukur terpisah. Angka ini belum
mencakup viewer. Mesin/model hilang, timeout, batal, dan path runtime Unicode
ikut diverifikasi. Audit impor PE mencatat hanya `kernel32.dll`.

## Kursor untuk geser dan resize tanda tangan (22 November 2026)

Format, Clippy desktop tanpa warning, 23 unit test, dan build release lulus.
Tes antarmuka di latar belakang memverifikasi penempatan, pemindahan, dan resize
proporsional. Pemeriksaan bentuk kursor Windows saat hover, drag, dan lepas
tersedia di `scripts/signature-ui-smoke.ps1`. Pemeriksaan ini perlu jendela uji
di depan dan belum selesai pada sesi ini karena jendela lain berada di depan.

## Kursor penempatan dan resolusi tanda tangan (22 November 2026)

Format, Clippy workspace tanpa warning, 79 unit test, build release, dan smoke
ekspor lulus. Smoke anotasi memverifikasi delapan aset 1024 × 512 yang sulit
dikompresi dapat disimpan, dibuka, dihapus satu, dan disimpan kembali. Aset yang
tersisa dibaca ulang dengan piksel RGBA identik. Byte PDF sumber tetap sama.

Pada PDF sintetis berteks, tes UI Windows memeriksa kursor teks sebelum
penempatan, kursor silang saat ghost aktif, dan kursor teks kembali setelah
Esc atau klik. QPDF mengonfirmasi bahwa **Simpan sebagai** menulis goresan uji
berukuran 624 × 214 piksel setelah crop, dengan dimensi sama dan interpolasi
pada gambar RGB serta mask alpha. Tes ini juga membuka ulang hasil simpan dan
menghapus tanda tangan.

Semua 22 skenario editor serta smoke galeri dan QR LAN lulus. QR diuji dengan
klien lokal. Bukti di `.research/signature-ui-45a7a19397a54c139e532ae7d03ddc24`,
`.research/signature-library-ui-cef5dea78a8b444a957f9c8ac8100810`, dan
`.research/signature-phone-ui-aa9b23578f284e58bf21c83393b61f42`.

## Penghalusan goresan tanda tangan (22 November 2026)

Goresan mouse dan QR berbagi geometri 2:1. Sampel subpiksel digabung, belokan
landai memakai kurva kuadratik, dan titik serta sudut tajam dipertahankan.
Kanvas memakai ujung serta sambungan bulat. Aset menggunakan raster 4x dan
cakupan alpha per piksel pada hasil 512 x 256 sebelum crop. Warna, ketebalan,
format aset dan kompatibilitas template tetap dipertahankan.

Format, Clippy tanpa warning, 23 unit test desktop, build release, 21 skenario
editor UI, dan smoke QR LAN lulus. Tampilan ghost saat drag dan hasil sesudah
lepas diperiksa. Bukti editor
`.research/signature-ui-daa07e111d4b4d8ba2a3f4d1cf97b359`
dan QR `.research/signature-phone-ui-7560cb5725de47c2ac3e7040c382b8a4`.
QR diuji dengan klien lokal. Pemindaian kamera dan sentuhan HP fisik belum diuji.

Build gabungan dipasang untuk run.cmd setelah executable utama dipastikan tidak
aktif. SHA-256 build dan hasil pemasangan identik
`3C607F97ACDFE622CD8FFCCEFE9A59ECE19EEAFF47423FCA171D7B0C92F90C15`.
Commit awal untuk ghost adalah `d3394a7`.


## Pratinjau penempatan tanda tangan (22 November 2026)

Tanda tangan, inisial, dan stempel muncul semitransparan di bawah kursor saat
menentukan posisi di atas PDF. Pratinjau dan penempatan berbagi perhitungan
ukuran dan batas halaman. Rotasi mengikuti orientasi PDF. Klik atau lepas setelah
drag pada halaman yang sama membuat satu perubahan yang dapat di-Undo. Esc dan
lepas di luar halaman membatalkan penempatan.

Tes UI memeriksa hover, batas tepi, kecocokan posisi klik dan drag, Esc, serta
Undo dan regresi Save As/buka ulang. Bukti lokal
`.research/signature-ui-fd9f94dc238d4807b1e7659fc0dd2e3a`.
Tes unit memeriksa batas dan rasio aspek pada empat orientasi halaman.


## Redesign tanda tangan, inisial, dan stempel (22 November 2026)

Editor membuka galeri dua kolom jika pustaka berisi template. Form pembuatan
terpisah menampilkan hanya aksi untuk Gambar, Ketik, Unggah, atau QR aktif.
Stempel membuka unggahan langsung. Footer tetap memuat Batal dan Gunakan.
Klik ganda pada kartu langsung masuk ke penempatan di halaman. Nama template
opsional, dengan nama otomatis per kategori. Edit nama dan konfirmasi hapus
berlangsung di kartu. Pustaka lokal tetap kompatibel dengan file versi sebelumnya.

Format, Clippy tanpa warning, 20 unit test desktop, dan build release lulus.
Suite editor 17 kasus memeriksa pergantian mode, preview, drawing, placement,
undo/redo, PNG lewat picker, serta Save As dan buka ulang. Suite galeri
memeriksa nama otomatis, restart, klik ganda, rename Enter/blur/Esc, dan hapus.
Suite QR memeriksa clipboard, kirim goresan LAN, Save As, dan pembatalan.
Pad HP lulus tes JavaScript. Hash PDF sumber fixture tetap sama.

Bukti UI lokal:
- Editor `.research/signature-ui-22eb79e702f94076ba1110e8f2fc1679`
- Galeri `.research/signature-library-ui-d38fd39c881a423598d2e723f8b5c246`
- QR `.research/signature-phone-ui-15f6077a6e6241eb95152c9ecc09deb1`
- Footer tetap terlihat saat scroll di viewport 860 x 500
  `.research/signature-small-window-ui-cfd6a1baabfd469cb592c5069f94f0d4`

Drag-and-drop file dari Explorer belum terverifikasi secara end-to-end karena
simulasi native tidak menghasilkan drop yang dapat dinilai. QR diuji dengan
klien lokal. Scan dan sentuhan HP fisik belum diuji.

Build teruji dipasang ke target/release/pdf-studio.exe untuk run.cmd setelah
target dipastikan tidak aktif. SHA-256 build dan hasil pemasangan identik
`32FC4AFD8090ECB38B8BA96C5D12BF55C2136C6B8CCC9F969E65E284BB75E7DD`.


## Tanda tangan tahap 5: QR LAN individu (20 November 2026)

QR membuka web pad lokal di browser HP, tanpa akun/relay/upload PDF. Listener
IPv4 pada alamat rute lokal dan port dinamis, token CSPRNG 192 bit, sekali kirim,
TTL 5 menit. Kiriman masuk sebagai draft/preview, tidak dirty sampai placement.
Batal/tutup/ganti metode/jenis/view membatalkan sesi. HTTP lokal tanpa TLS;
gunakan LAN tepercaya, firewall privat/client isolation/VPN dapat menghalangi.
Body/host/origin/koordinat dibatasi, tidak ada endpoint filesystem. qrcode 0.14.1
(no image feature) dan getrandom 0.3.4 dipin. Kolaborasi/email/sertifikat ditunda.

Fmt, Clippy desktop/all-targets denied warnings, **20 desktop unit tests** dan
release build lulus. Tes jaringan mencakup token/origin/payload invalid, single
receive, expiry/cancel release serta fragmentasi header/body. Windows accepted
socket awalnya mewarisi nonblocking listener; kini koneksi diterapkan blocking
dengan read/write timeout 2 detik, dan tes fragmented body lulus.
`scripts/phone-signature-pad-smoke.cjs` lulus enam pemeriksaan logika JS dengan
DOM/canvas stub (bukan browser/perangkat nyata). UI LAN lulus GET pad, cancel,
QR baru, POST/draft tanpa dirty, placement, Save As dan dismiss:
`.research/signature-phone-ui-cbfa647683c643d2aba2244ac5377627`.
**18 kasus regresi signature UI** lulus setelah QR ditambah:
`.research/signature-ui-e5e08885076546948e985266d01a9e8a`.
Capture QR/preview diperiksa visual. Source hash fixture tetap utuh. Tombol QR
cancel dipindah ke atas agar tidak bergantung scroll. Template restart/reuse sudah
lulus tahap 4. Scan kamera, sentuhan HP fisik, dua perangkat sungguhan dan variasi
firewall/NIC belum diuji; tidak mengklaim interoperabilitas perangkat tersebut.

Build teruji sudah dipasang ke target/release/pdf-studio.exe setelah target
dipastikan tidak aktif. Tidak ada proses/dokumen pengguna ditutup. Hash kedua exe:
`CEA3185B5D53D723D46CC7280D1110E4C7B45B5D16F898361999CD3C770762AA`.
run.cmd memakai build ini. Tahap 1-4 telah di-push: 126d81b, 74353d3, 30b4c92,
ef32651. Rotasi objek, multi-page apply dan SVG tetap follow-up; fase layanan
kolaborasi di luar cakupan eksplisit pemilik.


## Tanda tangan tahap 4 (20 November 2026)

Pustaka lokal opt-in (maksimal delapan template), rename/delete/reuse tersedia.
Storage signature-library.json dibatasi 10 MiB, serial/atomik di background;
corrupt data menonaktifkan write. Menghapus template tidak mengubah objek PDF.
Duplicate Ctrl+D/toolbar dan Alt+panah aktif saat canvas fokus, satu Undo/gesture;
perhitungan keyboard mengikuti rotasi halaman. Footer aksi tetap terlihat.
17 desktop unit tests, Clippy denied warnings, build release, 18 kasus signature
UI serta suite restart/reuse/delete template lulus. Bukti lokal:
`.research/signature-ui-ad4b950637e04b4386e86a9b4692f644` dan
`.research/signature-library-ui-ba0aa8075fc144aaa1ad30d90ee06de4`.
Run awal menemukan footer terdorong keluar viewport dan fokus keyboard tertinggal
di tombol; kedua masalah diperbaiki dan rerun lulus. PDF sumber tetap utuh.
Rotasi objek sendiri/multi-page apply masih follow-up; QR LAN tahap berikutnya.


## Tanda tangan tahap 3 (20 November 2026)

Stempel impor, hapus latar putih opt-in/reset, draw color/thickness dan editor
scroll tersedia. Raster dibatasi total 131072 piksel dan 512 per sisi; stempel
persegi sekitar 362 x 362, tanpa menaikkan budget 512 KiB/aset atau delapan objek.
Metadata lama tetap valid; build lama bisa menolak aset dengan tinggi >256 px.
Fmt, Clippy desktop/domain denied warnings, 15 desktop + 10 domain tests, release
build dan 14 kasus UI lulus. Suite kini menempatkan stempel persegi berwarna,
menghapus latar, Save As dan reopen/delete/reopen dengan sumber utuh. Bukti:
`.research/signature-ui-8b6b948ebcc245e4a799bc334c777730`.
Tahap pustaka lokal/placement dan QR LAN berikutnya; binary masih isolated.


## Tanda tangan tahap 2 (20 November 2026)

Ketik memakai font Caveat/Dancing Script OFL yang dibundel untuk offline, tiga
warna, input nama dan inisial terpisah, saran inisial editable. Raster font punya
alpha dan warna konsisten; input 1-80 karakter, kontrol/glyph unsupported ditolak.
Signature/stamp PDF tetap memakai format visual existing. Fontdue 0.9.5 ditambah.
14 unit tests dan Clippy denied warnings lulus. Build release lulus; 14 kasus UI
signature termasuk typed font/color dan inisial place/undo lulus; screenshot font
diperiksa. Bukti `.research/signature-ui-586caa41a311438a80b9368f9c734404`.
Cleanup graceful suite tertahan pada window uji; cleanup kini hanya boleh
mengakhiri process yang cocok exe+path fixture milik suite, bukan proses pengguna.
Batas dan fitur aktual ada di signatures.md. Tahap 3-5 masih berlangsung.


## Tanda tangan tahap 1 (20 November 2026)

Pemilik mengotorisasi implementasi per tahap dengan commit terpisah. Cakupan
individu/lokal: desktop dan QR LAN; kolaborasi/request/email/sertifikat ditunda.
Editor kini memisahkan Gambar/Dari gambar, lebih lega (500 px), punya Batal,
dan invalidasi cache pada Undo. Draw/import/placement tetap tanpa dirty sebelum
objek ditempatkan. Fmt, Clippy denied warnings, build release dan 12 kasus UI
signature lulus: termasuk metode/cancel, drop outside, move/resize Undo/Redo,
import invalid/alpha, split view blocking, Save As/reopen/edit/delete. Bukti:
`.research/signature-ui-08592b449cc1400f942b25df59b537e2`.
Build masih isolated target/tab-background-check; instalasi final setelah tahap
berikut selesai dan proses utama dipastikan tidak aktif. Tahap 2-5 masih berjalan.


## Recent files dua baris dan lebar moderat (19 November 2026)

Header/konten Empty State memakai max-width 40 rem (sekitar 640 px), meningkat
dari 24 rem dengan padding bawaan tetap lega. Item recent kini dua baris: nama
file/bintang; lalu lokasi fleksibel di kiri dan modified date di kanan. Lokasi
panjang ellipsis, tanggal tidak menyusut, minimum tinggi item turun 64 ke 52 px.
Tooltip dan accessible names tetap lengkap; welcome tetap scroll bila perlu.

Fmt, Clippy desktop/all-targets denied warnings dan release build lulus. Semua
13 kasus recent-files UI existing lulus (termasuk reopen/restart/favorit/metadata/
clear dan hash sumber tetap sama). Capture terang/gelap diperiksa visual:
`.research/recent-files-ui-7294cc59e9bb4075b6fab9400dd5b99d`.
Pemeriksaan layout tambahan lima item, missing-date fallback dan jendela pendek
lulus dengan capture diperiksa:
`.research/recent-metadata-layout-b55e7958634043c5a7e3d99599c044f2`.
Tidak menambah unit test atau mengulang suite PDF untuk perubahan layout ini.

Build teruji sudah dipasang ke target/release/pdf-studio.exe setelah target
dipastikan tidak aktif. Tidak menutup proses pengguna. Hash isolated/terpasang:
`8BDAF25F08753F4B39C5B14BCAFC7126F86D552B90547EF5253BFA1A23E34CD7`.
run.cmd memakai build ini.


## Metadata dan favorit pada recent files (19 November 2026)

Recent menampilkan bintang dari state favorit tab yang sama, folder penyimpanan,
dan tanggal modifikasi filesystem lokal (dd-mm-yyyy HH:MM), dengan informasi kecil
di bawah nama rata kiri. Tidak memakai table. Tooltip memuat path penuh dan tanggal.
Pembacaan metadata lima item berjalan di background saat load/record recent,
dengan revision guard agar hasil lama tidak muncul setelah clear/reorder. Tanggal
bukan last opened, tidak dipersist ulang dan tidak diawasi live. File hilang/tidak
terakses menampilkan tanggal tidak tersedia. Format recents.json tetap path-only;
tidak membaca konten PDF untuk metadata. Welcome page scroll saat tinggi terbatas.

Fmt, Clippy desktop/all-targets denied warnings, 13 unit tests dan release build
lulus. Suite recent-files UI kini **13 kasus** lulus, termasuk bintang, folder,
tanggal sesuai filesystem/zona waktu lokal, reload metadata + favorit setelah
restart, unstar sync, clear, dedup/order dan hash PDF sintetis tetap sama.
Capture terang/gelap diperiksa visual. Bukti lokal:
`.research/recent-files-ui-bf21cb4083e040179717329b36114d6a`.
Pemeriksaan tambahan lokal dengan lima path sintetis (satu hilang) memverifikasi
fallback tanggal dan scroll ke item terakhir pada jendela pendek:
`.research/recent-metadata-layout-654e97223e4f44268942803adaca7434`.
Suite native PDF tidak diulang karena perubahan pada metadata/UI welcome.

Build teruji sudah dipasang ke target/release/pdf-studio.exe setelah dipastikan
target tidak aktif. Tidak ada proses pengguna ditutup. Hash isolated/terpasang:
`B2887B503449414EADE1A14DD8F226E779F6EB91D429A6835EBD5982C2AAF32C`.
run.cmd memakai build tersebut.


## Sinkron recent dan banner pemulihan sesi (19 November 2026)

Tawaran pemulihan ditutup pada jalur open_paths bersama, sehingga recent/file
picker/reopen tidak meninggalkan banner setelah pengguna memilih dokumen lain.
Flag keputusan per window juga mencegah load_session async yang terlambat
memunculkan tawaran kembali. Cancel picker mempertahankan tawaran. X menggantikan
Mulai kosong, melewati pemulihan dan menyimpan snapshot sesi kosong, bukan
menghapus recent/favorit/PDF. Proses restore posisi/fingerprint tetap seperti semula.

Banner lebih ringkas: background amber transparan dan border amber, teks kecil,
heading/jelaskan PDF asli tanpa pemulihan edit belum disimpan, satu tombol
Pulihkan Sesi di kanan dan X paling kanan. Tema terang/gelap diperiksa visual.
Fmt, Clippy desktop/all-targets denied warnings, 13 desktop unit tests dan release
build lulus. **12 kasus session-restore UI** lulus: reading-only metadata, layout
compact/right actions, cancel picker, recent dismiss + no resurrection, picker
dismiss, restore active/order/position tanpa edit, partial + changed source,
last-tab empty session, corrupt metadata/clear, X remembered, write failure/exit,
dan source bytes unchanged. Bukti lokal:
`.research/session-restore-ui-f529c8ed14eb402fae03bd6097d036ba`.
Tes restart kini mengaktifkan window fixture sendiri; run awal berhenti karena
aktivasi UI otomatis setelah cancel dialog belum stabil, lalu rerun lulus.
Suite native PDF tidak diulang karena perubahan ini pada state/UI sesi baca.

Build isolated teruji: target/tab-background-check/release/pdf-studio.exe.
SHA-256: `799B364A52EE3AC4F5F101CDA7CAC7FAF797DEDCAA8D3C6E978CDB22CB9358D5`.
Pemilik menutup aplikasi; setelah target dipastikan tidak aktif, build teruji
sudah dipasang ke target/release/pdf-studio.exe. Hash terpasang cocok dengan
isolated di atas. run.cmd memakai build baru; tidak ada proses pengguna ditutup
oleh agen.


## Daftar isi membungkus dan collapse awal (19 November 2026)

Judul outline mengikuti lebar panel dan membungkus menjadi beberapa baris dengan
tinggi item otomatis. Panah berada di bagian atas baris; item tanpa anak memakai
spacer tanpa panah palsu. Saat model outline sumber pertama kali dimuat, semua
parent ditandai collapsed: hanya level teratas terlihat. Membuka satu parent
menampilkan anak, sedangkan parent di bawahnya tetap collapsed. State expand tetap
per view dan tidak direset saat beralih Thumbnail/Daftar isi; source/history tetap.

Fmt, Clippy desktop+smoke/all-targets denied warnings dan release build lulus.
Native --outline-print-smoke lulus untuk regresi generator fixture/destination/
print flags. UI outline **10 kasus** lulus: root-only initial, hierarchy/actions,
expand/collapse, named target, crop/rotasi, switch panel, delete/Undo, split view,
empty state dan judul panjang multiline tanpa overlap + navigasi judul tersebut.
Bukti lokal `.research/outline-ui-41d6e3e2c8fb4ff1a2d95ee08503bbf0`.
Capture judul panjang diperiksa visual dan hash sumber sintetis tetap sama.
Run awal menemukan width baris mengikuti min-content sehingga wrapping belum
terjadi; width baris kemudian dikunci ke lebar panel dan rerun lulus.
Tidak ada unit/PDF export suite lain yang diulang untuk layout ini.

Build teruji sudah dipasang ke target/release/pdf-studio.exe saat target tidak aktif.
Hash isolated/terpasang identik:
`DF6AD5D380A1B8883530E8BCD4BC1B077FDB16AF8C28A424965C7266C87F52DF`.
run.cmd memakai build baru; tidak ada proses pengguna yang ditutup oleh agen.


## Ikon panel navigasi dan tooltip (19 November 2026)

Segmented bertulisan diganti dua tombol ikon: LayoutGrid (Thumbnail), ListTree
(Daftar isi), selected background untuk pilihan aktif. Label visual dihilangkan;
accessible name dan tooltip tetap Thumbnail/Daftar isi. Model/perilaku outline,
mode Organize dan state per view tetap dipertahankan.

Fmt, Clippy desktop/all-targets denied warnings dan release build lulus. Delapan
kasus outline UI lulus (tree/local destination, switch, delete/Undo, split, empty):
`.research/outline-ui-9ebe218cfa624769a2d3bcf5ef1f1801`.
Tooltip Thumbnail diamati lewat hover dan capture lokal:
`.research/nav-icon-hover-3daa43964d9c4717aaa3d51e7b584544/thumbnail-tooltip.png`.
PDF sintetis tidak berubah; unit/native PDF suite tidak diulang untuk styling ini.
Build teruji sudah dipasang ke target/release/pdf-studio.exe saat target tidak aktif.
Hash isolated/terpasang identik:
`0CDE59F8FBE0D05FAA1D463723ED2D5E40019B82CD8A2F4979FB3D8EFADD14F4`.
run.cmd kini memakai ikon panel; tidak ada proses pengguna yang ditutup oleh agen.


## Chevron kiri, urutan toolbar dan segmented tabs (19 November 2026)

Pemilih Daftar tab kini tombol ikon chevron down/up di paling kiri baris tab,
tanpa label visual; nama aksesibel tetap Daftar tab. Popup dibuka dari sisi kiri,
dengan tooltip Buka/Tutup daftar tab. Tab dan + mengikuti tombol chevron.
Toolbar utama diawali Panel navigasi, Buka PDF, Simpan sebagai, Cetak PDF, Anotasi.
Pemilih Thumbnail/Daftar isi memakai TabBar::segmented() GPUI-kit (state per view).
Dalam mode Organize, Thumbnail ditandai aktif sesuai konten yang ditampilkan.

Fmt, Clippy desktop/all-targets denied warnings dan release build lulus. 16 kasus
UI lulus: 8 workspace (termasuk posisi/ukuran picker, popup kiri dan urutan toolbar)
dan 8 outline (nested/local destination, switch segmented, delete/Undo, split dan
empty outline). Bukti lokal:
`.research/workspace-ui-f224a3dd66004101a6b35369a336c42a` dan
`.research/outline-ui-80090f07d9f3474e8efd1288563e3666`.
Capture terang, gelap dan chevron terbuka diperiksa visual. PDF sintetis tidak
berubah. Unit/native PDF suite tidak diulang untuk perubahan presentasi ini.

Build telah dipasang ke target/release/pdf-studio.exe setelah memastikan target
tidak aktif. Hash isolated/terpasang identik:
`C3E91F73F8BB3A6C703B311399028BF4BA90F75A90C48D4E93BFFB7787833203`.
run.cmd memakai build baru; tidak ada proses pengguna yang ditutup oleh agen.


## Koreksi header tiga baris (19 November 2026)

Arahan pemilik mengembalikan multi-tab ke baris kedua: baris 1 hamburger + ruang
drag + kontrol jendela, baris 2 tab/+ /Daftar tab, baris 3 toolbar ikon dokumen.
Saat kosong, baris tab tetap disembunyikan. Dropdown tab kini berawal di y=68
logical px agar tidak menimpa baris kedua. Perapian contextual annotations,
undo/redo, separator zoom/search, label canvas dan OCR coming soon tetap berlaku.

Fmt check, Clippy desktop/all-targets denied warnings dan release build lulus.
16 kasus UI lulus: 8 workspace (termasuk posisi tiga baris dan anchor tab picker)
dan 8 caption/menu/minimize/maximize/restore/close. Bukti lokal:
`.research/workspace-ui-8df7183a57da4780b55c3d6274f82996` dan
`.research/compact-titlebar-165b0aa88c9e4d2b827f41f5d14866b7`.
Capture diperiksa visual; sumber sintetis tidak berubah. Unit/native PDF suite
tidak diulang untuk koreksi layout ini.

Build teruji sudah dipasang ke target/release/pdf-studio.exe setelah memastikan
target tidak aktif. Hash isolated/terpasang identik:
`B0C21C81082A7081C4190FFE18C46CA3AF171B5B3121D24590BFFFDBFF952F6B`.
run.cmd kini memakai susunan tiga baris. Tidak ada proses pengguna yang ditutup.


## Workspace ringkas dan toolbar kontekstual (19 November 2026)

Menu hamburger, tab/+ /Daftar tab dan kontrol jendela kini satu header. Menu lama
tersedia sebagai submenu; area drag terpisah dari tombol/tab. Pemicu Anotasi di
toolbar utama; baris alat hanya ada saat anotasi terbuka pada mode baca. Empat
panah navigasi dihapus dari toolbar, input nomor/total halaman tetap. Undo/Redo
muncul saat Organize/anotasi atau history ada. Search dan zoom diberi separator.
Label Halaman di bawah tab Thumbnail dan Halaman N di atas kertas dihapus; nomor
thumbnail tetap. Status tanpa selectable text memakai chip Butuh OCR disabled,
tooltip OCR segera hadir, tanpa engine/proses OCR.

Fmt, Clippy desktop/all-targets denied warnings, 13 unit tests desktop dan release
build lulus. UI terpilih **36 kasus** lulus:
- 8 workspace: header satu baris, submenu action, input halaman, contextual tools,
  undo/redo di mode baca, narrow/dark, empty state, OCR disabled. Bukti lokal:
  `.research/workspace-ui-fd83e57059d741708febe800828c5df6`.
- 8 caption/menus: `.research/compact-titlebar-cfbf4948f7444be6b7f6254a504d7d67`.
- 9 annotation: `.research/annotation-ui-1208716d589e4c67af2bc8ac51b188b6`.
- 11 signature: `.research/signature-ui-449c0bb2cf34499c8faeb0cf41264bf8`.

Anotasi/tanda tangan mencakup close-cancel, Save As/reopen/delete dan shared views.
Hash PDF sintetis tetap. Snapshot workspace diperiksa visual. Tes tanda tangan
awal berhenti karena selector pemicu Anotasi masih berada di dalam pane; selector
diubah untuk fokus pane lalu memakai pemicu toolbar bersama dan rerun lulus.
Menu-Action pada helper tes kini mendukung hamburger serta executable UI lama.
Suite native PDF tidak diulang untuk perubahan presentasi ini. Snap flyout,
touch, mixed DPI/multi-monitor dan interaksi stylus belum diuji pada checkpoint ini.

Build isolated teruji: target/tab-background-check/release/pdf-studio.exe.
SHA-256: `33A1FBBC22389913696DEEFA471040023CEA33A39BA8F10EB8115943B977095D`.
Build telah dipasang ke launcher setelah pemilik menutup aplikasi. Hash terpasang
identik dengan build isolated yang diuji.


## Perataan kiri recent files (19 November 2026)

Ikon dan nama file recent memakai baris konten selebar tombol, rata kiri pada
posisi yang sama. Nama panjang tetap ellipsis; accessible name dan tooltip path
utuh dipertahankan. Tombol Empty State utama tetap terpusat.
Fmt, Clippy desktop/all-targets denied warnings dan release build lulus. Delapan
kasus existing recent-files UI lulus, dengan capture terang/gelap diperiksa visual:
`.research/recent-files-ui-84c1dd9530d8401f8e5ddd8efc5d6bd6`.
Tidak ada test/unit PDF baru atau suite PDF diulang untuk styling ini.
Build teruji sudah dipasang ke target/release/pdf-studio.exe setelah target tidak
aktif. Hash isolated/terpasang identik:
`D3A927C356924824BFE399C269F85EB813868E4784E50E1878642A0DF86B2A5E`.


## Terakhir dibuka di Empty State (19 November 2026)

Lima recent PDF tampil di bawah tombol Buka PDF; maksimal 20 path disimpan dalam
recents.json lokal, newest-first dan deduplikasi path Windows. Klik membuka lewat
inspeksi biasa, tooltip menunjukkan path, Hapus riwayat membersihkan recent tanpa
menghapus PDF/favorit/sesi. Hanya pembukaan berhasil dicatat, termasuk restore.
Tidak memuat isi PDF otomatis untuk daftar ini. File metadata rusak/oversize
menonaktifkan write agar data lama aman. Write atomik/serial; close menunggu prefs.

Fmt, Clippy desktop/all-targets denied warnings, 13 desktop unit tests dan release
build lulus. Unit baru memverifikasi round-trip/clear serta invalid/corrupt metadata.
Delapan kasus UI lulus: persist successful open, recent reopen, dedup, newest-first,
promote reopened, light/dark, reload after restart dan persistent clear. Hash kedua
PDF sintetis tetap sama. Bukti lokal:
`.research/recent-files-ui-21c999dd92c34eb5a5d2bbd104ab0983`.
Suite native PDF tidak diulang untuk riwayat UI. Missing/moved file memakai error
pembukaan biasa; kasus tersebut belum diuji UI khusus pada checkpoint ini.

Commit fitur tersedia: **c6e49ea**. Build isolated teruji:
`target/tab-background-check/release/pdf-studio.exe`, SHA-256
`BA57376005728AA8FA1D36A0D67E33ECBC46D58FB33613DD07E5367CAD564401`.
Build sudah dipasang ke target/release/pdf-studio.exe setelah pemilik menutup
aplikasi. Hash terpasang identik dengan build isolated di atas. Status push diverifikasi
terpisah; cache origin/main tidak membuktikan keberhasilan pengiriman.


## Empty State tanpa baris tab (19 November 2026)

Saat tidak ada tab dokumen, baris multi-tab/+ /Daftar tab disembunyikan. Konten
kosong menggunakan Empty dan slot header/media/title/description/content GPUI-kit,
dengan tombol Buka PDF. Tab muncul lagi setelah membuka PDF; close terakhir dan
cancel picker kembali/tetap kosong. Header menu dan kontrol jendela dipertahankan.

Fmt, Clippy desktop/all-targets denied warnings dan release build lulus. Enam
kasus UI lulus: tutup terakhir menyembunyikan tab, empty terang, cancel picker,
buka dari Empty State mengembalikan tab, tutup terakhir lagi dan empty gelap.
Bukti lokal `.research/empty-state-ui-b27654110c31463fb7df528bef2560d3`.
PDF sintetis tidak berubah. Suite PDF/unit tidak diulang untuk perubahan layout ini.
Build teruji: `target/tab-background-check/release/pdf-studio.exe`, SHA-256
`658E97BA0B7A3FBE79E8748D9211C6F96B89A4E49770CAA975D5DF565BEC4DD4`.
Build sudah dipasang ke launcher setelah pemilik menutup aplikasi; hash terpasang
identik dengan build isolated di atas.


## Header menu dan kontrol jendela (19 November 2026)

Baris logo/nama dan menu digabung menjadi satu header 34 logical px. Menu di kiri,
ruang drag di tengah, kontrol minimize/maximize-restore/close custom GPUI di kanan.
Hit area Windows dipisahkan sehingga menu dapat diklik tanpa menyeret jendela.
Kontrol memakai jalur native Windows; tab tetap di baris berikutnya. Identitas
Origo native/taskbar/About tetap. Detail perilaku: [multi-tab](multitab.md).

Fmt dan Clippy desktop/all-targets denied warnings lulus. Release build lulus.
`compact-titlebar-ui-smoke.ps1` lulus delapan pemeriksaan: single-row/menu-client-hit,
drag-caption-hit, control-hit, menu/tema terang-gelap, maximize/restore, minimize,
dan close. Label Restore/Maximize ikut diperiksa. Sumber sintetis tidak berubah.
Bukti lokal `.research/compact-titlebar-3cb73ca7dd984184aa290275d71d2ef4`.
Suite PDF/unit tidak diulang untuk perubahan layout ini. Snap flyout, touch dan
multi-monitor/mixed DPI belum diuji secara interaktif.

Build sudah dipasang setelah pemilik menutup aplikasi utama. Hash isolated dan
`target/release/pdf-studio.exe` sama:
`4197F57A052118A5ABFC2CF902B6BA2207B5B584B9D864278ED65E669359DC3F`.
`run.cmd` kini memakai build header baru. Status Git/push lihat checkpoint akhir
sesi dan git log; autentikasi remote sebelumnya masih terhalang.


Riwayat build cetak/outline sebelum perubahan header: build teruji dipasang ke `target/release/pdf-studio.exe`
setelah memastikan tidak ada proses memakai target. `run.cmd` memakai lokasi ini.
SHA-256 build isolated dan terpasang identik: `F0877E523A922AD0BFACEA662DCBC00798A19F134FF44FD5BDEB95F7F5A2C460`.
Tidak ada proses/dokumen pengguna yang ditutup untuk pemasangan.

## Cetak dan daftar isi (19 November 2026)

Fmt, Clippy workspace/all-targets denied warnings dan workspace tests lulus:
**66 unit tests** (26 application, 10 domain, 5 platform, 12 desktop, 13 QPDF).
Tes baru mencakup grammar/range workspace dan penolakan publikasi cetak ke sumber
atau tujuan yang sudah ada, tanpa mengubah byte keduanya.

Release workspace dibangun terisolasi di `target/tab-background-check/release`.
Native `--outline-print-smoke` lulus: nested/Unicode/parent tanpa destination,
named/local destination, CropBox/rotasi, external action exclusion, stale-source,
4200 bookmark dibatasi ke 4096 dengan indikator, snapshot range dan source bytes.
Render print intent menghilangkan anotasi nonprintable sambil mempertahankan yang
printable; render viewer tetap menampilkan keduanya.

Native `--annotation-smoke` dan suite default lulus: editable Ink/Highlight/Stamp,
alpha dan rotasi, Save As/Extract/Split/Optimize/PNG, teks/search, consent/unsafe
rejection, staging/no-clobber/cancel serta source preservation. Tidak ada printer
fisik yang dipakai suite native.

UI outline awal delapan kasus lulus di
`.research/outline-ui-fe9c4bc1725a4530ac5e80284ae9de61`: nested/disabled external,
expand/collapse, named destination/clean, rotated crop, sidebar switch,
delete/Undo target, split view dan empty state. Build akhir juga lulus delapan kasus yang sama di
`.research/outline-ui-d1fd84053d844490943d23cea22ebce9`.

UI cetak build akhir lulus lima kasus di
`.research/printing-ui-5fff1930646043869fd39accbc061a1d`: rentang invalid tetap di
modal, preview cancel mempertahankan edit, pilih printer Microsoft Print to PDF,
publikasi file baru tetap dirty, dan reopen hasil current-page menjadi satu halaman.
`--verify-printed-pdf` memverifikasi 1067 piksel anotasi biru yang belum disimpan
pada PDF virtual tersebut. Hash sumber tetap sama. Tes memeriksa keberadaan kontrol
Properties; interaksi properti driver fisik tidak diklaim sudah diuji.

Uji awal menemukan loop helper yang tidak selesai setelah dialog printer serta
pemanggilan dialog bertumpuk oleh tes otomatis. Helper akhir memakai STA khusus,
dialog printer klasik dan guard reentrancy. Tes menunggu owner enabled sebelum
submit dan hanya mengirim job setelah label Microsoft Print to PDF dikonfirmasi.
Run gagal sebelumnya bukan bukti lulus. Suite akhir tidak mengirim job fisik.

Batas yang belum teruji: printer fisik/offline/duplex/warna dan hasil ukuran di
kertas; semua driver/port Windows; kebijakan enterprise yang memblokir PowerShell;
interoperabilitas UI vendor; keyboard tree khusus, malformed cycle/depth dan cache
besar di UI. Node count besar diuji native, bukan klaim semua PDF rusak didukung.
Tidak ada instalasi driver atau job ke printer fisik. Kode tidak memanggil API
perubahan default printer; Windows dapat mengelola default dari printer terakhir.


Pemasangan branding ke lokasi launcher: build Origo teruji disalin ke
`target/release/pdf-studio.exe` setelah dipastikan tidak aktif. Hash identik dengan
build isolated: `3008C73397F0562A9592C6EDE332F4C0CFA8A005D975E78663EF414F0AA64283`.
Branding smoke pada executable utama lulus; bukti lokal
`.research/branding-ui-7ed58dbff8a4411a9adeb54f02da2354/result.json`.
`run.cmd` kini meluncurkan binary berikon tanpa perubahan script.


## Ikon/nama Origo (17 November 2026)

Fmt, Clippy desktop/all-targets denied warnings dan isolated release build lulus.
`branding-ui-smoke.ps1` lulus, bukti lokal ignored:
`.research/branding-ui-6f5664a6be524a25a5c2a8771276bf8d/result.json`.
Pemeriksaan meliputi group icon resource ID 1 sembilan ukuran, ProductName dan
FileDescription Origo, native window title, nonzero class HICON dan kesamaan piksel
ikon native 32px terhadap PNG sumber turunan. Class icon merupakan sumber native
untuk taskbar/Alt+Tab; UI task switcher Windows tidak direkam secara terpisah.
Capture terang/gelap/About diperiksa visual. Fixture PDF tidak berubah dan window
uji ditutup normal. Script regenerasi ikon menghasilkan hash aset yang identik.
Unit/native PDF suite tidak diulang untuk pemasangan resource/label UI ini.


## Tanda tangan visual (17 November 2026)

Fmt, Clippy workspace/all-targets denied warnings dan release workspace build lulus.
Workspace **64 unit tests** lulus: 25 application, 10 domain, 5 platform, 11 desktop,
13 QPDF. Setelah hardening popup/mouse-up, 11 desktop tests lulus lagi. Tes mencakup
asset metadata round-trip/malformed bounds, pad kosong/crop, PNG alpha, JPEG reduction,
file rusak/transparan dan Stamp action/forged-subtype rejection.

Native `--annotation-smoke` lulus pada plain dan CropBox nonzero + sumber terotasi:
objek Stamp terlihat tanpa overlay, alpha/model pulih saat reopen, penghapusan pada
Extract tidak terlewat copy shortcut, empat orientasi output, flag render dipulihkan,
searchable source text, Optimize/Split appearance, PNG pixels dan source bytes utuh.
Perbandingan rotasi memakai dimensi raster yang sama melalui exact rendering agar
pembulatan satu piksel pada preview biasa tidak mengaburkan pemeriksaan posisi.
Default native M1/text/search/Save As/Extract/Split/Optimize/PNG juga lulus, termasuk
consent, compatibility blockers, cancellation, source changes dan no-clobber/race.

UI signature memakai `scripts/signature-ui-smoke.ps1` dengan executable terisolasi.
Input PDF/gambar sintetis; tidak membuka file privat. Kasus: pad kosong/Undo/preview
non-dirty, drop di luar halaman batal, placement satu objek, move Undo/Redo,
resize proporsional, invalid image, alpha PNG preview, shared views, popup guard,
close guard, Save As/reopen/edit dan delete/save/reopen. Run final: **11 kasus signature lulus** di
`.research/signature-ui-5ede606183b54cc68540f53ff1d12136`; regresi anotator **9 kasus**
di `.research/annotation-ui-b4a3c978b93b4707ac5064032ebb115e` (total 20 UI).
Perbaikan outside-drop dan outside-click popup diverifikasi ulang pada build final. Capture dua panel diperiksa visual; gambar/thumbnail konsisten dan
PNG alpha tidak membawa kotak latar. Native/source resource bounds bukan benchmark
performa tanda tangan padat. UI JPEG/EXIF, stylus/touch dan viewer vendor belum diuji.


## Toolbar anotasi dropdown (17 November 2026)

Fmt, Clippy desktop/all-targets denied warnings dan release build lulus.
UI anotator **9 kasus lulus**: slider pen 12 pt, preview/color, pengaturan tanpa dirty,
Hanya teks menonaktifkan slider, dua view, Undo/Redo, close guard, Save As/reopen/delete.
Bukti ignored: `.research/annotation-ui-bc2f2b61719b4f36b5190d01e4f77d02`.
Preview diperiksa visual; metadata PDF hasil simpan mengandung lebar pen 12 pt.
Semua input sintetis. Suite unit/native checkpoint sebelumnya tidak diulang;
format PDF dan model ekspor tidak berubah.


## Anotator inti ? 17 November 2026

`cargo clippy --workspace --all-targets --locked --offline -- -D warnings` dan
`cargo test --workspace --locked --offline` lulus: **61 tests** (25 application,
9 domain, 5 platform, 10 desktop, 12 QPDF). Tes tambahan memeriksa transaksi highlight
multi-page/history/PageId, metadata bounded, multiply highlight tanpa penggelapan pada
self-crossing, dan penolakan action pada anotasi inline maupun indirect.

`target/tab-background-check/release/pdf-smoke.exe --annotation-smoke` lulus:
objek Ink/Highlight asli terlihat via native renderer tanpa overlay, model pulih saat
reopen, penghapusan tersimpan (termasuk full-source Extract), dirty/history/receipt,
render flag isolation, kecocokan raster editor vs output pada empat orientasi, dan
hash/bytes sumber tetap. Default native smoke juga lulus: M1, text, search, Save As/
Extract, tagged/whole-source cases, Split, Optimize, PNG termasuk failure/cancel/race.
Semua input pengujian sintetis; tidak membuka PDF privat pengguna.

Build release workspace terisolasi lulus di `target/tab-background-check/release`.
UI anotator **7 kasus lulus** (`scripts/annotation-ui-smoke.ps1 -Profile release
-ExecutablePath target/tab-background-check/release/pdf-studio.exe`): draw/dirty,
Undo/Redo per goresan, stabilo/highlight teks, shared history dua view, close-cancel,
reopen/erase dan save/reopen deletion. Bukti lokal ignored:
`.research/annotation-ui-1d60b5ae446f40a5a87d9b4fcd4ca0e1/cases.json`.
Capture dua panel diperiksa secara visual; tool row membungkus dan kedua view serta
thumbnail menampilkan anotasi yang sama. Clippy akhir setelah penguatan smoke rotasi
juga lulus. Script UI menerima executable terisolasi tanpa mengganti proses pengguna.

Regresi UI release juga lulus: **9 kasus same-document** di
`.research/same-document-ui-e015b4fcc9434f8292dc251428f87ea5` dan **14 kasus Find**
di `.research/search-ui-bf37e2c4ddc348d8ae3a7be8b93e947d`. Total UI sesi ini 30 kasus.
Suite comparison/session-restore/profil performa tidak diulang pada fitur ini.

Uji ini tidak membuktikan editing anotasi eksternal atau interoperabilitas UI di
Edge/Chrome/Firefox/Acrobat. Tanda tangan tidak diimplementasikan atau diuji.


- Docs-only checkpoint from source HEAD `70287a7`; no product build/tests were rerun.
- Rechecked recorded manifests: same-document 9, comparison 10, Find 14 and session
  restore 8 cases; all entries passed. Same-document profile summary still reports
  passed, 3 rounds/31 samples, 330,465,280 peak private bytes and 679,936 warmed growth.
- Primary release executable exists and its hash matches the isolated build used by
  the background-tab verification below. This does not establish remote CI or a
  current portable release.
- Git started clean on main, local tracking origin/main `a1647e0`, HEAD `70287a7`,
  1 ahead/0 behind before the docs commit. No fetch/push/server check in this handover.
  New progress summary/current handoff replace obsolete next-step instructions;
  old handoff is preserved in the explicitly historical archive.
- Checked 188 local documentation links and diff whitespace. The archived payload
  matches the `70287a7` source blob after reversing its relative prompt-link adjustment.

# Verification — active-tab background fix, 17 November 2026

- Restored the theme `tab_bar` background on the custom document strip. Standalone
  folder tabs already paint `tab_active`; the missing strip background made the
  active shape blend into the surrounding content.
- Fmt, denied-warning desktop Clippy, debug build and isolated release build passed.
  Visual inspection plus pixel sampling passed for both A/B active tabs in both
  themes at `.research/tab-background-3024590b2f5243dabfebe801e3f7a255` (4 captures).
  Light active/inactive colors were #ffffff/#f5f5f5; dark #0a0a0a/#171717. The
  colors swapped with focus and fixture hashes stayed unchanged.
- Active release executable was preserved while building separately. After a fresh
  process check found no user instance using it, the verified build was copied to
  `target/release/pdf-studio.exe` and hashes matched. No user process was closed.
  No unit tests added for this styling-only change; previous functional coverage remains.

# Verification — multi-tab tahap 4B, 17 November 2026

- Final `scripts/check.ps1` passed: fmt, denied-warning Clippy, **57 unit tests**
  and release workspace build. The shared-model test checks one history/selection,
  unique import IDs, cross-view busy state and shared invalidation without cloning
  Workspace/history. No engine or export-preservation policy change.
- Debug same-document suite passed 7 cases at
  `.research/same-document-ui-e057a4dfe82a4707afa540ed7d01ae56`.
- Final release same-document suite passed **9 named cases** at
  `.research/same-document-ui-11845abf68784585b3e9d7b47e1bcb43`: independent reading/
  zoom/Find/text selection, shared selection/edit/Undo and deleted-page fallback,
  imports from both views with unique IDs and search invalidation, PNG/Split dialog
  routing, secondary Save As while closing the actor's view, duplicate Open/reuse,
  preferred-view session metadata, document-close cancel/Discard/Save with only the
  secondary visible, metadata-only reopen and source-change invalidation of both.
  Source bytes stayed unchanged. Initial harness trials were corrected for virtual
  thumbnail realization, completion of duplicate Open and the unsaved-dialog button's
  explicit accessibility label before this successful run.
- Release two-document comparison regression passed all 10 cases at
  `.research/comparison-ui-979321e5a5fd431ab6de1315784094d3`.
  Final splitter-hover hardening also passed all 10 cases at
  `.research/comparison-ui-5fa8d8f85c3445538c3ed1650443ed24`: a hover move without
  a pressed-button flag during a held drag cannot release the minimum-width guard.
  Fmt/workspace Clippy, the 10 desktop unit tests and release build passed again
  after this focused change; no resource-policy change followed the profile.
- Release existing tabs passed at `.research/tabs-ui-787dbdea328048b8959664106bf88821`;
  search passed 14 cases at `.research/search-ui-01091d02874a4c8fb226b7816ce1216e`.
- Session restore passed 8 cases at
  `.research/session-restore-ui-d17e5cc02d9c4ec9b88788a950649502`; organizer/M2 passed
  at `.research/m2-ui-c3a472c58b6e4722b846c9fb336c7670`. The session fault-injection
  harness now waits for the background failure notice before requesting exit,
  avoiding a race where that Close was already the permitted retry. The product
  failure/close policy was unchanged.
- Same-document memory trial (8 open PDFs, 1,000 pages each, 3 rounds, two views of
  the vector PDF at 400% simultaneously) passed at
  `.research/same-document-memory-cbb1098da0d74b69a3b68705aafb9854`: 31 samples,
  maximum private bytes 333,180,928, warmed growth 569,344 bytes, handle growth 9.
  The rerun after preserving pan across shared edits passed at
  `.research/same-document-memory-f5c14c5bbdac4dc9bccc0501df005bfd`: **31 samples**,
  maximum private bytes **330,465,280**, warmed growth **679,936 bytes**, handle
  growth **10**, GPU growth **−36,864 bytes**. Idle caches/native residency returned
  to zero and corpus hashes stayed unchanged. This corpus differs from the two-document profile;
  these numbers are not a direct performance comparison.
- No new physical mixed-DPI, clean-machine, hostile-native isolation, remote CI or
  public binary-release claim. Group tabs/vertical tabs remain backlog.

# Verification — multi-tab tahap 4A, 17 November 2026

- Two different PDFs use the native GPUI Kit 0.9.1 Dock and real splitter; one
  shared toolbar/organizer follows focus. Hidden sessions release raster/text/GPU
  caches; each visible pane receives half the existing window budgets.
- Final `scripts/check.ps1` passed: fmt, denied-warning Clippy, **56 unit tests**
  and release workspace build.
- Final release comparison suite passed **10 named cases** at
  `.research/comparison-ui-54f5ce182f384dffbfaa018d990996b6`: independent navigation/
  search, selection/history/input focus, splitter movement and minimum width while
  dragging, cross-document page-drop rejection, narrow dark drawer, exit/reentry,
  guarded dirty close, export receipt origin, source-change isolation and unchanged
  source bytes. Initial trials exposed cached Dock accessibility and transient
  resize issues; fixed before this successful run. No toolchain sources were edited.
- Release regressions passed: tabs `413f4894b02d4d20859697fb6275b559`, hardening
  `66229f4a8e4a4c38b776fd5e783e3cfc`, M2 `73f615f04b2944eb96dadfcf28dae27e`,
  search (14 cases) `a98cb02f93f44929aef3de400164760f`; each prefix identifies its
  matching ignored `.research/*-ui-*` or `.research/tabs-hardening-*` fixture directory.
- Generated-book profile (8 PDFs × 1,000 pages, 3 rounds, high zoom) passed at
  `.research/comparison-memory-3b7feb0803a144d3893c703e26bf8a59`: 28 samples,
  maximum private bytes 485,273,600, warmed private growth 966,656 bytes, handle
  growth 8. The stricter final profile holds both panes at 400% simultaneously:
  `.research/comparison-memory-947dca5ad8a348abab627931a42da1e7`, **31 samples**,
  maximum private bytes **533,323,776**, warmed growth **1,433,600 bytes**,
  handle growth **31**, GPU growth **−4,096 bytes**. All configured guards passed,
  idle caches/native residency returned to zero, and source hashes were unchanged.
  Budgets are cache limits, not process caps; this is a bounded three-round trial.
- No new physical mixed-DPI, hostile-native isolation, remote CI or binary publication claim.

# Verification — multi-tab tahap 3, 17 November 2026

- `scripts/check.ps1` passed: fmt, denied-warning Clippy, **55 unit tests**, release
  workspace build. New metadata test covers all zoom modes, Unicode paths, fingerprint
  roundtrip, invalid schema/ranges/relative paths, 8-tab/1-MiB bounds, atomic replacement
  failure and cleanup without overwriting previous metadata.
- Debug session suite passed 7 cases at `.research/session-restore-ui-2f2550b0b1a941c89bad5091e07c2e20`.
- Final release session suite passed **8 named cases** at
  `.research/session-restore-ui-2abe3c4aca3741728aad875ff445bf42`: dirty/multi-source
  reading metadata, offer/order/active/positions without edit recovery, partial/changed
  sources, last-tab empty session, corrupt preservation/explicit clear (including
  recording disabled with a PDF open), declined restore, visible write failure/retry
  exit, and unchanged source bytes. Source mtime was deliberately changed on its own fixture.
- Release recovery suite passed at `.research/tab-recovery-ui-85cf2928db1b4a608fef25037c7da3c7`.
- Release existing multi-tab suite passed at `.research/tabs-ui-83ab36c36c314f499f662bc1663ff474`.
- Release empty-startup chrome passed at `.research/chrome-ui-e5045b225b324be595448bb8e5110bea`.
- Harnesses now explicitly isolate UI state under their own fixture directories;
  existing test cases remain intact. Windows PowerShell scripts use UTF-8 BOM for
  non-ASCII menu labels. An initial debug trial had an incorrectly decoded ellipsis;
  that harness issue was fixed before successful runs.
- No new physical mixed-DPI, hostile-native isolation, remote CI or binary-publication claim.

# Verification — multi-tab tahap 2, 17 November 2026

- `scripts/check.ps1` passed: fmt, denied-warning Clippy, **54 unit tests**, release
  workspace build. Desktop-only behavior; no PDF engine/export policy changes.
- Debug recovery suite passed at `.research/tab-recovery-ui-00fcc1e9828a4a0b93e9dff2b9c73b47`.
- Release recovery suite passed **6 named cases** at
  `.research/tab-recovery-ui-b46eedf6a76f40c48bd6d5844b57baf4`: reading position/zoom,
  case-insensitive search/empty/keyboard, missing-source retry/changed-source reset,
  close cancel/Discard semantics, duplicate reopen and unchanged source hashes.
- Release stage 1 suite passed with shared existing test helpers at
  `.research/tab-management-ui-7b845ce8fd1b412b9f22bfa835d8419f`; all stage 1 assertions retained.
- Release existing multi-tab suite passed at `.research/tabs-ui-41b2416e87ed48ba8e70403bcb0f08c9`.
  State/history/export/close/cache regressions remain covered with the searchable picker.
- Test support accepts `-IsolatedState` to keep synthetic preferences under the fixture.
  Initial debug trial found an inaccessible empty-result label; fixed and retested.
  No new physical-input, hardware DPI, remote CI or public binary-release claim.

# Verification — multi-tab tahap 1, 17 November 2026

- `scripts/check.ps1` passed: fmt, denied-warning Clippy, **54 unit tests**, and
  release workspace build. New test covers atomic favorite replacement and corrupt input.
- Release `tab-management-ui-smoke.ps1` passed at
  `.research/tab-management-ui-f13f40c143784158b1e400954c92e095`: plus placement,
  persistent stars including process restart, inactive context target, menu reorder,
  real drag/cancel, dirty batch cancel/discard, source hashes. State directory isolated.
- Release existing tabs suite passed at `.research/tabs-ui-3d85455896204a9c9803d2203cb79ed0`:
  isolation, viewport/history, duplicate/multi/partial/capacity Open, Save As, Optimize,
  close batch and cache limits. Original test coverage retained.
- Release chrome suite passed at `.research/chrome-ui-fd05eef2ed4a480cbe9a7303230c9728`:
  reader/organizer, two themes, four widths, toolbar containment and page input.
- Earlier trial runs failed in the test launcher/drag harness; one run failed to find
  a native picker after empty state. Final restart test and existing empty-state/Open
  regression passed. Failed runs are not counted as successful evidence.
- User's existing release process preserved; its executable renamed within target/release
  before build. It continues using the older build. No physical mixed-DPI claim.

# Latest verification - active-PDF text search, 17 November 2026

Owner approved Find in the active PDF, with an in-tab compact bar. The explicit
contract is default case-insensitive matching and contiguous phrases: `hasil penelitian`
must not match scattered words. See [search](search.md).

- Workspace formatting, denied-warning all-target Clippy, **53 unit tests**, and
  release workspace build passed. Four new matcher/navigation tests cover phrase
  whitespace/linebreaks, case/whole-word independence, Unicode character offsets,
  query/result limits, non-overlapping occurrences and wrapping in both directions.
- Release `--search-smoke` passed actual PDFium extraction: phrase/case matching,
  same-page wrapped phrases, scattered-word rejection, four intrinsic rotations and
  virtual geometry rotations, empty-page text, unchanged bytes and changed-source
  rejection. The full release native smoke also passed M1/text, search, M3, tagged/
  whole-source Extract, Split, Optimize and PNG, including their failure/cleanup cases.
- Final release `scripts/search-ui-smoke.ps1` passed **14 named cases**:
  `.research/search-ui-4e4935cb28a548e88e110e53a2d9ea66`.
  It covers bar position below tabs, default phrase matching, rotated-result reveal,
  next/previous/Enter/F3/wrap, case/whole-word options, empty/missing/oversized queries,
  rapid replacements, per-tab state, Escape/reopen, rotate/import/delete/Undo,
  no-text feedback, close with pending search and unchanged source hashes.
  Its 1000-page case observed progressive results, paused an incomplete scan by
  switching tabs, resumed to 1000 matches and checked shared/inactive cache bounds.
- Release chrome regression passed both themes at 860/1000/1200/1600 px, toolbar
  containment, drawer input/Escape and page entry:
  `.research/chrome-ui-8b6d7ecc2a954fd581726d239ebb9a32`.
  Search screenshots were inspected: orange active hit, yellow other visible hits,
  correct phrase bounds and rotated vertical text.
- An intermediate UI run exposed a visible search-status label missing from the
  accessibility tree. Adding its semantic Group role fixed it; the final complete
  suite passed afterward. Buttons also restore search-field focus for Enter navigation.
- Numeric diagnostics include counts/options/progress only, excluding queries/text.
  All UI tests use owned synthetic fixtures/processes; the pre-existing user app was
  preserved by renaming its locked old EXE within target/release before rebuilding.
  No new dependencies, export-policy change, public package, remote CI or physical
  modifier/assistive-technology acceptance is claimed. Prior release gates remain open.

---

# Latest verification - PDF to PNG only, 16 November 2026

Baseline `2c3033c`. The owner approved PNG only, all/selected pages and 150/300 DPI;
JPEG/TXT are deferred. See [PNG](png.md) for behavior, limits and the physical-DPI
metadata limitation. No AI, OCR, network service or other converter was implemented.

- `scripts/check.ps1` passed formatting, denied-warning workspace/all-target Clippy,
  **49 unit tests** and release workspace build. Two new unit tests cover exact DPI
  rounding/resource rejection and immutable selected order/rotation/revision.
- Full native smoke passed Save As/Extract, M1/text, tagged/whole-source preservation,
  Split and Optimize regressions, plus PNG. The final release `--png-smoke` also passed
  with mixed page geometry across sources: 150/300 DPI, all 16 source/virtual rotation
  combinations, full decoded RGBA equality against independently rotated native source
  rasters, selected pages, counts/progress, metadata/bookmarks and complex-input blockers.
  Existing file/folder, mid-batch encoder failure/cancel, output cap, destination race,
  referenced-source changes and staging/source cleanup were exercised on generated PDFs.
- Release PNG UI passed **11 named cases**:
  `.research/png-ui-266cd6a2b9ca4c2eae9aa8c809386038`.
  Configuration/default selection, disabled empty selection, dialog/picker cancellation,
  exact selected 150/300 dimensions and rotation, all-page export, existing-image hash,
  tab origin, cancellation of a running 100-page batch from another tab, dirty-close
  waiting for PNG then asking for unsaved edits, source hashes and cleanup all passed.
  The dialog screenshot and emitted PNG were visually inspected.
- Regression UI passed tabs (`.research/tabs-ui-ac342587e39f47b197165530473de2e8`),
  all **25 M3 cases** (`.research/m3-ui-a7f1205c6ba94b6c92efe69c5f0bc25a`),
  Split (`.research/split-ui-6cd872c2aa874e639a9f473c427acd36`), and Optimize
  (`.research/optimize-ui-89d891ae2b1644d19f6fcca76bc00de4`). The exact observed PASS
  output is retained in `.research/png-split-regression.log` and
  `.research/png-optimize-regression.log`.
- Internal portable candidate:
  `dist/pdf-studio-windows-x64-candidate-2c3033c-0442227c.zip` (dirty source marker).
  ZIP/file SHA-256, **1103 files**, pinned QPDF and x64 import closure passed. Five
  negative integrity/import tests rejected changed contents, extra files, missing CRT
  and incorrect ZIP checksum, then restored/revalidated the owned extraction.
  All **11 PNG UI cases passed again** against the extracted executable under restricted
  PATH at `.research/png-ui-ec4e718920c24b1b84dffe9cbd292337`, using
  `.research/portable-658643116fb74ea1ae3b55be4e9646d6/PDF Studio 日本 & test`.
  Candidate inventory retains the existing three notice gaps and release-review gates;
  the lockfile adds image dependencies to platform/smoke without changing package versions.
- Intermediate test corrections: the source-change injection initially targeted an
  unreferenced source, correctly leaving a selected-only export unaffected; it now
  mutates a referenced source. The harness initially discarded only one dirty fixture
  tab; cleanup now walks the full owned close batch and requires process exit.
  Clippy's fixed-size chunk lint was corrected; no assertions or export policy were relaxed.
- Markdown links, PowerShell syntax, source diff/whitespace and per-case manifests were
  rechecked. The old M3 snapshot remains intact. All UI/process tests use owned synthetic
  fixtures. Mixed-DPI/physical accessibility, clean Windows and distribution review
  remain open; local candidate success is not a public release or remote CI result.

---

# Latest verification - distribution candidate and final regression, 16 November 2026

Baseline `06c2a39`. Follow-up runtime/notice inventory, internal portable ZIP and
package verification are implemented; public release remains gated. See
[distribution audit](distribution-audit.md) and [release checklist](release-checklist.md).

- Exact `anyhow` pin/lock upgraded from 1.0.100 to 1.0.103 for
  [RUSTSEC-2026-0190](https://rustsec.org/advisories/RUSTSEC-2026-0190.html).
  `scripts/check.ps1` passed fmt, denied-warning all-target Clippy, **47 unit tests**
  and optimized workspace build after the dependency change.
- Full native smoke passed pinned engines, M1, text/rotations, tagged/whole-source
  preservation, M3 consent/blockers/order, Split and Optimize, including failure,
  cancellation, collision/race, source hashes and staging cleanup.
- Release tabs UI passed: `.research/tabs-ui-3a16a718227d44839daa8e0becb8975b`.
- Final release M3 passed **25 unique named cases**, all passed and none omitted:
  `.research/m3-ui-c207be7c0bf14866be3d5ab609d83a4f`. Intermediate rechecks exposed
  native picker teardown timing and premature clipboard reads. The harness now
  waits for its owned window/dialog lifecycle and exact clipboard message with
  bounded timeouts; existing behavioral assertions remain mandatory. A missing
  picker also captures its screenshot/state before cleanup. Failed attempts are
  not counted as final acceptance; the complete suite was rerun after these changes.
- Final candidate: `dist/pdf-studio-windows-x64-candidate-06c2a39-d88070f1.zip`.
  Build info honestly records the pre-commit revision and dirty state. The ZIP and
  its adjacent checksum were verified, then extracted/relocated to
  `.research/portable-ed8cb41bc495428f93adf525aa32d985/PDF Studio 日本 & test`.
  **1103 manifest entries** matched actual paths, sizes and SHA-256. Standard/delay
  x64 import closure and pinned QPDF under restricted PATH passed.
- Package negative checks passed independently: unexpected file, modified contents,
  missing app-local runtime import, runtime membership/hash mismatch and ZIP checksum
  mismatch. Owned test edits were restored and the original package passed again.
- Final extracted-package UI passed:
  `.research/portable-ui-de017dec506b49e5bdd44e94526d6c51`.
  Restricted-PATH launch, local PDFium/MSVC modules, rendering/text, edit/Save As/reopen,
  exact page count/rotation, Unicode/space/ampersand output path, source hash and staging
  cleanup passed. The reopened rotated PDF screenshot was visually inspected.
- Final inventory: `.research/distribution-audit-c576e07ce22d407b9d67cb2f004554c5`.
  469 desktop normal/build records: 459 notice-collected, three missing-evidence,
  one MPL source-review, six project-license-unset. Eighteen supplemental notice
  texts have exact upstream-commit URL and SHA-256 provenance; hashes were checked.
- OSV rescan queried all **463 registry versions**, validating batch counts and
  pagination. Anyhow's match is gone; four unmaintained notices remain (instant,
  paste, rustybuzz, ttf-parser). Native-engine/security disposition and redistribution
  review remain open, as do clean Windows and physical-device acceptance.
- First candidate ZIP attempt rejected upstream Unix-epoch timestamps; only owned
  package timestamps are now normalized. Initial hidden test-window launch was
  corrected to an interactive window. One cleanup/test command's automatic review
  timed out; the narrower retry targeted only the verified owned fixture process.
- Source/docs/notice diff, local Markdown links, all 29 PowerShell script parses,
  supplemental hashes and final per-case manifests were reviewed before commit.
  CI now builds/verifies an internal candidate without uploading binaries. No remote
  CI result is claimed. The ignored ZIP is not part of Git commit/push.

---

# Latest verification - M3 migration and device/checkpoint gates, 16 November 2026

Baseline `582cd67` (`feat: tab`), clean main/cache origin/main at start. Rechecks were
performed per stage/case; this checkpoint does not claim completion of hardware
acceptance that the agent cannot physically perform.

- `scripts/check.ps1` passed fmt, denied-warning workspace/all-target Clippy,
  **47 unit tests** and release workspace build after adding numeric DPR/viewport
  diagnostics. No PDF engine/export policy changed.
- Final full native smoke passed M1/text/M3/tagged/whole-source/Split/Optimize,
  including order/raster/text/source identity, consent/blockers, failure/cancel,
  output collisions/races and owned cleanup.
- Migrated release `scripts/m3-ui-smoke.ps1` passed **25 distinct named cases**:
  `.research/m3-ui-84af4549a79a49538215f5dd9ce695f3`.
  The saved `cases.json` was rechecked against the explicit 25-case manifest with
  no missing, duplicated or non-passed case. This includes a real native existing-file
  selection/overwrite confirmation followed by application refusal and unchanged hash.
  [Coverage map](m3-ui-coverage.md) maps replacement-era transitions to Open/tab/window
  behavior while retaining native output/consent, error copy, hashes and staging checks.
- The historical script was copied before migration and verified byte-identical;
  its Git-filtered blob equals `582cd67:scripts/m3-ui-smoke.ps1`.
  `scripts/archive/m3-single-workspace-ui.ps1.txt` is intentionally non-runnable.
  Automatic review initially rejected broad rewrite without a backup; the safe
  migration preserved this verified snapshot and used targeted edits plus a coverage map.
- Final release device audit passed four automated checks:
  `.research/device-acceptance-2e02b16c1741410684633492f73f15e8`.
  One connected display was observed (1920x1200, Windows DPI 96); GPUI scale/viewport
  agrees, required controls are visible, TabItem/close roles and ValuePattern work,
  targeted Enter/PageUp/PageDown navigate correctly, Tab reaches a named control,
  and Escape returns from the narrow drawer. The 100% screenshot was visually inspected.
- **Six device/human cases remain pending**, not counted as pass: actual 150%, actual
  200%, mixed-monitor transition, physical modifier shortcuts, physical trackpad
  pinch/scroll/pan and screen-reader announcements. No global DPI setting or global
  keyboard input was changed/injected. [Manual matrix](device-acceptance.md) and the
  `-LeaveOpen` fixture command make the remaining acceptance actionable.
- Release general multi-tab and hardening regressions passed:
  `.research/tabs-ui-ada8dcd591ab4dc99e9a9e5483c54ea0`,
  `.research/tabs-hardening-3f1081877b4d457ab077839874d135d5`.
- Intermediate existing-file attempts exposed transient native IFileDialog UIA
  invalidation. Native Button enumeration restricted to the owned PID replaced that
  fragile lookup; the actual no-overwrite/hash check remains mandatory. A subsequent
  device attempt read accessibility focus before projection settled; the final audit
  waits for input focus and a named target rather than relaxing its focus assertion.
  Failed intermediate attempts are not acceptance; owned failed windows were closed.
- Final review passed all 132 local Markdown links across 31 documents, syntax of
  all 22 current PowerShell scripts, diff whitespace and historical snapshot fidelity.
  Per-case evidence was independently rechecked: 25 distinct expected M3 passes,
  four automated device passes and six explicitly pending hardware/human cases.
  These checks precede the authorized local checkpoint commit.
  No remote CI/fetch/push, package publication, user PDF or private research note was used.
  Physical priority 4 remains open even after the priority 5 local checkpoint.

---
# Latest verification - unified tabs, hardening and memory profile, 16 November 2026

- Final hardening re-run also passed after the shared harness was strengthened to
  identify its own PID by executable path and unique fixture GUID:
  `.research/tabs-hardening-0d2ae36c34e84b52a6ec5e73e38ed533`.
- Additional three-round high-zoom profile passed with 8 loaded books, 400% zoom/fit
  on both source kinds each round, 61 samples and the same resource guards:
  `.research/tabs-memory-723a63a16ec146108b0826cc2c276314`.
  Peak working set **509.99 MiB**, sampled private maximum **410.25 MiB**, GPU shared
  maximum **185.26 MiB**, warmed private growth **0.875 MiB**, GPU growth **0 MiB**.
  Page-cache sampled maximum **120.31 MiB** stayed below 128 MiB. Inactive residency,
  GPU release errors and empty-idle native documents remained zero. Driver pools
  retain a warmed baseline; handle counts 744/758/764 stayed within the documented
  guard, not claimed perfectly constant or proof against unbounded-session leaks.
- Final long-filename containment checks passed in both themes/four widths;
  × stays inside its TabItem: `.research/chrome-ui-1ccd5db683e141a99d4eaafbde08f8d2`.
  Release M1 empty startup, lazy 100-page startup, boundary navigation, thumbnail,
  continuous scroll, rapid 50–400% zoom/stale rejection and resize/fit passed:
  `.research/m1-ui-49d288ac2dc04c3cbdfa1e3589cab7b1`. Its owned window was closed normally.
- Release Split and Optimize regressions also passed on this implementation:
  `.research/split-ui-1074a25c42ea4325b5a80c82eea3373a`,
  `.research/optimize-ui-7654370ba1ff4f29bd57a3680dbc7f29`.

- `scripts/check.ps1` passed fmt, workspace/all-target Clippy with denied warnings,
  **47 unit tests** and release workspace build. Debug desktop build also passed.
  New copy-buffer coverage checks Unicode byte limits, separators and rejection
  without partial append. Full native M1/text/M3/tagged/whole-source/Split/Optimize
  smoke passed, including source identity, consent/blockers, failure/cancel/races
  and owned transaction cleanup.
- Unified GPUI Kit folder tabs put the × suffix inside the TabItem. Release hardening
  checked actual accessibility bounds and targeted HWND close clicks, including
  closing an inactive clean tab without switching the active document.
- Release hardening passed offscreen selected-text copy after cache eviction,
  repeated fast switch/close, isolated source change/reopen, Save As completion
  while another tab is active, cancellation and the cross-tab export-slot/dirty-close
  cancellation race. Clipboard was materialized/restored and fixtures were owned.
  Evidence: `.research/tabs-hardening-acd78bfb87b146d49bc7612a780f88a3`.
  The earlier successful debug run was
  `.research/tabs-hardening-ffa63e19c38f423ea62985ddbcf08f5c`.
  The integrated tab/list-control screenshot was visually inspected.
- Release memory profile passed **3 identical rounds / 8 sources x 1000 pages**,
  four generated vector and four simulated scan books, with two sweeps per round,
  close-to-empty, native idle timeout and unchanged hashes. 55 samples recorded
  process private/working set, handles/threads/GDI/User objects and real Windows GPU
  process-memory counters, filtered to the test PID.
  Evidence: `.research/tabs-memory-27c7c4d8de0d4a1dab31cbfe93da46cb`.
  Peak working set **209.53 MiB**, sampled maximum private **162.81 MiB**, GPU shared
  maximum **70.50 MiB**, dedicated **0 MiB**. Warmed endpoint private growth was
  **0.438 MiB**, GPU growth **-0.035 MiB**; handles **785 -> 786 -> 792**, GDI/User
  objects stable **77/37**. CPU caches/GPU residency/native documents were all zero
  at every empty-idle endpoint; native documents never exceeded one and GPU release
  errors were zero. Driver/allocator baseline is retained, not claimed as zero.
  See [protocol and limits](multitab-hardening.md); this is not a universal leak,
  physical-memory cap or PDFium-only heap measurement.
- Existing release multi-tab suite passed:
  `.research/tabs-ui-edd0fd023b424750bb7ecb6363274f7f`.
  Release light/dark/four-width drawer/Enter/hit-testing/Esc chrome passed:
  `.research/chrome-ui-6197f6a4142c408ba26eeb3596c2d4f1`.
- Release text selection passed single/double-click, jitter, forward/reverse drag,
  zoom coordinate reuse, reader/organizer selection preservation, all four intrinsic
  rotations, cross-page copy, clear, byte budget and source hashes:
  `.research/text-ui-27903420cb0249b4996e1959bef876cb`.
  Its historical mode labels were updated to Panel alat/Kembali membaca; the removed
  Konversi placeholder is now asserted absent rather than treated as navigable.
- An intermediate Windows PowerShell run interpreted a BOM-less literal bullet as
  ANSI in a test selector; the harness now constructs that marker with its Unicode
  code point. It was a failed intermediate run, not acceptance. The owned pending
  job was cancelled and its synthetic viewer closed normally before retrying.
- No user PDF, global input, remote CI/fetch/push or portable release claim. Existing
  user process was preserved while the old release EXE was renamed within target
  for rebuilding. Broader corpus/duration, physical mixed-DPI/trackpad and native
  isolation remain separate gates. The original M3 single-workspace UI suite remains
  intact and was not claimed as migrated or passing.

---
# Latest verification - desktop UI and multi-tab, 16 November 2026

- `scripts/check.ps1` passed fmt, denied-warning workspace/all-target Clippy,
  **46 unit tests**, and release workspace build. Debug desktop/workspace builds
  also passed during implementation. No domain/export preservation contract changed.
- Final full native smoke passed M1/text/M3/tagged and whole-source Extract/Split/
  Optimize, including consent/blockers, source/raster/text identity, no-clobber races,
  cancellation/failure and owned cleanup.
- Final release multi-tab UI passed: last-tab close to empty state and reopen,
  isolated edit/history/viewport, duplicate Open preserving state, mixed valid/corrupt
  multi-file Open, capacity refusal and eight tabs, Save As cleaning only its owner,
  Optimize retaining dirty, cancelled save picker, discard-then-cancel retaining all
  tabs, save-then-cancel retaining earlier clean receipt, final window close,
  active cache bounds, zero GPU-release errors and unchanged source hashes.
  Evidence: `.research/tabs-ui-81d435fb01574f91a955fd2c061a7fd2`.
- Debug chrome passed light/dark at 860/1000/1200/1600 px, non-overlapping fixed
  toolbar, focused page draft/Enter, actual HWND click on drawer Rotate and Esc
  returning to reading. Evidence: `.research/chrome-ui-7e012d11ea8c4e7693594601e6b7eb00`.
  Physical DPI was 96; this is not mixed-monitor acceptance. Dark narrow-drawer and
  light eight-tab screenshots were visually inspected.
- Final release chrome, including actual drawer hit-testing/Esc, passed with settled
  mode/accessibility checks: `.research/chrome-ui-ce32122af63e4c07b5e16faa37d53137`.
  Final release Split passed validation/preview, per-N/manual, native folder picker,
  order/rotation, collisions/cancel, batch loss consent/Ink, close-wait, hashes and
  cleanup: `.research/split-ui-b5a4775bedaa4a77aa63704914828e27`.
- Final release Optimize passed picker cancellation, edited multi-source plan,
  exact size reporting, unchanged dirty/history, loss consent, close-wait guard,
  blockers/JPEG, hashes and cleanup:
  `.research/optimize-ui-90c0f46c82214b50993f2d404ac13a0d`.
- Debug M2 UI passed drag, rotate/move/delete, empty/history, native import,
  multi-source order and unchanged-source checks:
  `.research/m2-ui-cec52a4635594f499cb8429698b62d6b`.
  Debug M1 passed no-argument startup/close, lazy 100-page startup, boundary jump,
  thumbnail/continuous scroll, rapid 50–400% zoom with stale-result rejection and
  resize/fit: `.research/m1-ui-503dddfaf2734145be1be4f51a04cb69`.
  Empty desktop chrome also passed:
  `.research/chrome-ui-139c3484698e421786381f09e560bdc5`.
- Debug M2 drag/rotate/move/delete/empty/history/native import/multi-source/source
  hashes passed: `.research/m2-ui-cec52a4635594f499cb8429698b62d6b`. Debug M1 empty
  startup/100-page navigation/thumbnail/scroll/rapid 50-400% zoom/resize passed:
  `.research/m1-ui-503dddfaf2734145be1be4f51a04cb69`. These ran before the final
  drawer hit-test guard; the added guard was then checked by the chrome suite above.
- Final release Chrome/Split/Optimize evidence is recorded in the follow-up below.
- Intermediate UI runs found a close guard blocking the shell's own close request,
  duplicate control labels confusing test actions, and stale UIAutomation projections.
  The final code fixes the guard/labels. Chrome acceptance waits for both numeric
  mode state and accessibility projection instead of assuming a fixed delay.
  Mouse-based save activation during dialog animation was replaced with the existing
  UIAutomation Invoke pattern. Failed intermediate attempts are not acceptance.
- The old `m3-ui-smoke.ps1` remains intact, with historical single-workspace
  Open-replacement assertions. It was not claimed as passing. Automatic approval
  review rejected replacing that suite because it would remove existing coverage;
  the safe alternative adds `tabs-ui-smoke.ps1` and preserves the older test.
- Original user viewer was preserved. To rebuild release without closing it, its
  old executable was moved within `target/release` to `pdf-studio.previous-*.exe`;
  `run.cmd` starts the newly built release. Owned passing test windows were closed.
- No push/fetch/remote CI, real user PDF, converter, OCR, portable certification or
  physical mixed-DPI/trackpad claim. Full native/GPU/process memory profiling and
  broader corpus/accessibility acceptance remain release gates. CPU cache bounds
  are not a total process-memory limit. See [multi-tab](multitab.md).

---
# Latest verification - lossless Optimize, 16 November 2026

- scripts/check.ps1 passed formatting, workspace/all-target denied-warning Clippy,
  **45 unit tests** and release workspace build. Workspace debug build also passed.
  New tests cover optimized capture/receipt never marking saved, smaller/equal/larger
  results and zero-size rejection before commit, plus exact lossless-only QPDF options.
- Full debug native smoke passed existing M1/text/M3/Split and new Optimize checks:
  multi-source edited order/rotations/deletion, exact same-plan before/after byte sizes,
  source text and exact 512px RGBA versus an ordinary export of the same plan; blank,
  simulated scan and JPEG cases; original JPEG encoded bytes retained; unchanged
  dirty revision; metadata/bookmark consent and blocked forms/signatures/tags/active
  content; cancellation/failure after writing; destination race and changed sources;
  owned baseline/staging cleanup. No new general performance/fidelity claim.
- Release Optimize UI passed cancelled picker, transformed multi-source page plan,
  exact size receipt and accessible result text, Undo/Redo, metadata refusal/acceptance,
  Close waiting then retaining unsaved edits, conservative blockers, blank/JPEG output,
  unchanged source hashes and cleanup. Evidence:
  .research/optimize-ui-5b6e871cd8b14f77bdbb69626adf3a7a.
  optimize-result.png was visually inspected. A valid tiny blank PDF grew from its
  431-byte ordinary-export baseline to 521 bytes; the UI correctly reported growth.
  Small JPEG output also grew (1571 -> 1615 bytes), while the edited multi-source case
  shrank (3019 -> 1892 bytes). These are generated fixtures, not reduction guarantees.
- An intermediate blank fixture lacked a Resources dictionary, so clean QPDF preflight
  correctly blocked repair warnings. The fixture was corrected; production preflight
  was not loosened. The earlier interrupted suite is not counted as acceptance.
- Final regressions passed on the Optimize build: release M3/unsaved guard
  (.research/m3-ui-612af32f2ad1469cbf4aef08de67390b), release Split
  (.research/split-ui-1fc72d1242b74d978835dd0613cab04a), and debug M1 empty startup,
  100-page navigation/thumbnail/scroll, rapid 50-400% zoom and resize
  (.research/m1-ui-93de63cbfd69453991750c92596dc122). The owned final viewer was
  closed normally afterward; existing user sessions were preserved.
- No new dependency/runtime/sidecar or real user PDF. The tiny JPEG is a generated
  color pattern (scripts/new-optimize-fixture.ps1), committed for repeatable tests.
  No portable release, remote CI result, overwrite, image downsampling, OCR or broader
  form/tag/signature preservation is claimed. See optimize.md for current limits.

---
# Latest verification - Split PDF, 15 November 2026

- Final scripts/check.ps1 passed formatting, workspace/all-target Clippy with denied
  warnings, **42 unit tests** and the release workspace build. The three new tests
  cover per-N/manual parsing and invalid input, folder-name safety, and actual Windows
  directory no-replace behavior for existing empty directories/files and new targets.
  The workspace debug build also passed.
- Full debug native smoke passed existing M1/text/M3 checks plus Split: multi-source
  order/rotations/text/sample raster identity; per-N/manual groups; progress; unchanged
  dirty baseline; refusal/acceptance of tag-loss consent; passive Ink/page appearance;
  byte-identical whole-source empty-form/tagged copy; a blocked later signed part;
  unused-source exclusion; existing destination/race refusal; changed originals;
  injected failure on the second writer call; cancellation after the first validated
  part; no partial final folder and owned staging cleanup.
- Release Split UI passed validation/preview, per-N and manual groups, real native
  folder picker, deterministic names, QPDF count/rotation checks, unchanged workspace,
  existing-output hashes, cancelled picker/consent, accepted batch tag loss/Ink,
  Close waiting for Split followed by unsaved guard, source hashes and staging cleanup.
  Evidence: .research/split-ui-16cd253457374468a719b5847afcfb4e.
  split-dialog.png was visually inspected. Only owned synthetic fixtures/windows used.
- Release M3 + unsaved-guard regression passed on the same final build:
  .research/m3-ui-cb2003c199b84fffada56484845c959a. Final debug M1 UI passed empty
  startup/clean close, lazy startup, 100-page navigation, thumbnail/scroll, rapid zoom
  and resize: .research/m1-ui-1c2c841922ad4edea931c984d30e105f. Its fixture window
  was closed normally afterward; the existing user viewer remained untouched.
- Early checks exposed replacing std::fs::rename behavior, retained PDFium output
  handles preventing directory moves, an error cleared on input blur, and the native
  folder filename control differing from the file picker (Edit 1152). These were fixed
  and final checks above passed. A negative fixture initially used an empty AcroForm,
  which is correctly allowed by whole-source copy; the blocked-part test now uses a
  synthetic signature structure. Intermediate attempts are not passing acceptance.
- Added pinned Windows-only atomicwrites 0.4.4 and its MIT notice; no new toolchain,
  native sidecar or external conversion runtime. Viewer session reuse is unchanged;
  inspect_output validation releases its document before publication/cleanup.
- No real user PDFs, remote CI, push, portable release, large-batch performance claim,
  process isolation or crash-recovery claim. See split.md for current limits.

---
# Latest verification - unsaved-change guards, 15 November 2026

- Added a single desktop Open/Close transition with candidate inspection, a final
  fingerprint recheck, explicit Save As/discard/cancel and exact-receipt continuation.
  Page mutations/history/import/additional exports are locked while leaving. No new
  domain/export interfaces, dependencies or broader PDF preservation claims.
- Formatting, workspace/all-target Clippy with denied warnings and **39 unit tests**
  passed. The two new tests cover export completion/current-receipt requirements and
  cancellation tokens. Existing source/snapshot/receipt/no-clobber tests remain green.
  Final scripts/check.ps1 and workspace debug build passed, producing both profiles.
- Full debug native smoke passed after generating synthetic export fixtures: M1,
  text/16 rotations, M3, tagged extraction and byte-identical whole-source extraction.
- Release M3 UI passed cancellation/invalid Open, candidate changed after inspection,
  Batal focus, Escape, repeated close, disabled mutations, empty-plan save disabled,
  cancelled Save As, successful save-before-Open, explicit discard/close, pending
  export wait/cancel-intent, Extract remaining dirty, failed save keeping the old
  workspace, consent refusal and successful save-before-close. Existing organizer,
  export/popup/select-copy/source-hash/staging regressions passed too. Evidence:
  .research/m3-ui-d31dabaa7f3d421896d095761ccad295 (final executable).
  The unsaved-prompt screenshot was visually inspected. An earlier attempt exposed
  missing accessible title/ambiguous save controls; a later stronger assertion was
  corrected to expect Undo to restore the selection immediately before Delete.
  These intermediate attempts are not counted as final acceptance.
- Release M2 drag/history/import/multi-source UI passed, including dirty-window cleanup:
  .research/m2-ui-d3733ea6ea414e4196ec75c063662db6.
  Release chrome UI passed both themes, four widths and focused page entry:
  .research/chrome-ui-74577653e9854d4c9a586b1a8f7afbc0.
- Final debug M1 UI passed empty/no-argument startup and clean close, lazy 100-page
  startup, navigation/thumbnail/scroll, rapid 50-400% zoom and resize:
  .research/m1-ui-1b8b478590194c9ea5e455ec7010ca4b. Its owned fixture window was
  closed normally after acceptance. No new 1000-page stress/physical-DPI test is claimed.
- All UI input/captures used only owned synthetic fixtures and test HWNDs. No real
  user PDFs, global SendKeys, new installations, remote CI or push were involved.
  Autosave, crash recovery, forced termination and external-source recovery remain
  outside this feature; see unsaved-changes.md.

---
# Latest verification - partial tagged Extract, 15 November 2026

- Formatting, workspace/all-target denied-warning Clippy and **37 unit tests** passed.
  New tests cover the passive Ink closed schema and selective accessibility JSON
  patching, preserving annotation data and stream dictionaries. Existing unsafe/
  schema/identity/snapshot/no-clobber tests remain green.
- Full debug native smoke passed. New three-page synthetic fixture has tagged content,
  metadata/viewer settings and passive Ink on page 3. Partial pages 1 and 3 require
  consent; without consent no output exists. With consent, page count/text and full
  512px RGBA rasters match. Whole-source byte-copy and existing M1/text/M3 checks passed.
- Explicit debug AND release checks of the user's supplied three-page document passed
  for both page 1 and annotated page 3: QPDF/PDFium validation, exact full-raster equality,
  text, consent refusal and unchanged source bytes. Original source SHA-256 remains
  unchanged. No actual document text, screenshots or PDF copies enter tracked fixtures.
  Temporary verification outputs were owned/removed.
- Release workspace build passed. Expanded M3 UI passed Batal/Simpan tanpa tag, tag
  removal from catalog/page objects, preserved Ink subtype/InkList on extracted page 3,
  unchanged source hash and the existing organizer/Save As/Extract/history/metadata/
  bookmark/popup/select-copy/import/Open/cleanup checks. Evidence:
  .research/m3-ui-fd8b8252153d4570bed5b349905dea15.
- No new dependency/runtime/installation, user-file overwrite, remote CI or push.
  Existing user sessions were preserved. Tag preservation/remapping, other annotation
  types/actions/appearances, active forms/signatures and broader fidelity are not
  newly claimed; see ADR-0003 and compatibility for this deliberately scoped policy.

---
# Latest verification - whole-source Extract, 15 November 2026

- Read-only inspection of the user's supplied one-page invoice found an empty
  AcroForm (only Fields, zero field summaries, no XFA), tags, viewer preferences and
  page-level StructParents/Tabs. QPDF check returned 0. No user contents or file copy
  were added to tracked fixtures; outputs lived only in owned temporary directories.
- Workspace/all-target denied-warning Clippy, formatting and **35 unit tests** passed.
  New cases cover original-order/full-selection eligibility, rejection after partial
  selection/reorder/rotation, copy-only tag acceptance and rejection of active fields,
  XFA, signature flags, encryption and document actions. Immutable plan fields prevent
  strategy changes after the source preflight.
- Full debug native suite passed existing M1/text/M3 checks plus a matching synthetic
  tagged/empty-form whole-source Extract. Output equals every original byte, and
  rotated tagged extraction remains blocked rather than stripping structures.
- Explicit debug and release verification against the supplied invoice passed:
  identical output bytes, unchanged source bytes, QPDF/PDFium checks, page text and
  sampled raster comparison. Temporary outputs were removed. Source SHA-256 checked
  independently before/after and remained unchanged. No actual invoice screenshot or
  contents were captured in acceptance evidence.
- Release workspace build and expanded headed M3 suite passed, including UI selection
  and Extract of the matching one-page fixture, identical output/source hashes, and
  existing Save As/Extract/history/consent/error-popup/copy/recovery/cleanup checks.
  Evidence: .research/m3-ui-e453bedfe89b411cab9338b41a136ca6.
- Existing user processes were retained during rebuild. No installation, new native
  engine, format converter, remote CI or push occurred. This is a narrow complete-
  source extraction path, not general form/tag-preserving structural editing.

---
# Latest verification - selectable error text, 15 November 2026

- Popup error body now uses GPUI Kit SelectableText for plain literal text. The
  Salin pesan button copies the complete original reason without dismissing the
  dialog. Tutup has a direct accessible callback and restores the modal guard.
- Desktop all-target Clippy with denied warnings and the release build passed.
  No backend/domain/dependency changes were made; full unit/native-only suites
  were not repeated for this UI-only fix.
- Final release M3 UI suite passed: drag selection over the error body, visible
  highlight (screenshot inspected), exact clipboard text after Salin pesan, popup
  remaining open, Tutup/repeated error recovery, plus existing Save As/Extract/
  history/bookmark consent/catalog/import/Open/source-hash/staging checks.
  Evidence: .research/m3-ui-dbb145fcc2394193b732bb34be6e2fb5/error-popup.png.
  Original clipboard formats were restored. Input targeted only the test's HWND.
- Tests wait for modal entrance/hover before dragging and modal exit before further
  input. Open waits for a new workspace generation/source identity, including when
  recovering from an empty state. Intermediate failed attempts exposed an accessible
  footer callback issue and stale/empty-state assumptions in the harness; they are
  not counted as passing acceptance. The final rerun above completed successfully.
- Existing user windows were preserved by an in-workspace executable rename before
  rebuilding. run.cmd launches the new release; existing windows keep old code.
  This change does not alter export preservation policy or resolve the user's
  separate unknown-catalog PDF. No remote CI, installation or push was performed.

---
# Latest verification - GPUI Kit error popups, 12 November 2026

- scripts/check.ps1 passed fmt, workspace/all-target denied-warning Clippy,
  **33 unit tests** and release workspace build. After adding the modal text-input
  guard, desktop Clippy plus debug/release builds passed again.
- Release native smoke passed the complete existing M1/text/M3 suite. Catalog
  diagnostics now name actual known blockers; a unit case checks tags, MarkInfo,
  page labels, viewer preferences, layout, color intents and unknown attribute names.
  No unsupported features were newly allowed or removed.
- Expanded release M3 UI passed GPUI Kit error popups for export forms, tagged
  catalog, page labels, unknown catalog, failed import and failed Open, including
  exact reasons, Tutup and recovery. Existing organizer/Save As/Extract/history,
  metadata/bookmark consent, source-hash and owned-staging checks also passed.
  Final evidence: .research/m3-ui-84551693d1c14fcc91d903420c224a9d.
  error-popup.png was visually inspected: centered modal, icon/title, reason,
  dimmed document and Tutup. Earlier pre-guard run passed as well:
  .research/m3-ui-ae36ecd010e444b386ea4f8ba22cce3b.
- Final release text selection regression passed single-click/jitter, double-click,
  forward/reverse drag/copy, zoom reuse, mode/conversion selection preservation,
  four rotations, cross-page copy, clear/cache bounds and source hashes. Clipboard
  restored. Evidence: .research/text-ui-eae2e837b2c547ff8611b739a495889c.
- One intermediate headed attempt failed because Windows created an unowned Save
  picker while the test app was inactive. The harness now also finds top-level
  #32770 pickers using the test's own PID; it never targets another app's dialog.
  The failed attempt is not counted as acceptance. Only its own synthetic process
  was cleaned up. Production handling was not weakened to work around automation.

The user's PDF/path was requested but has not been supplied. Its exact catalog
attribute and appropriate export support remain unconfirmed. This change implements
popup feedback and precise diagnostics; it does not claim to fix export of that
specific document. All acceptance used synthetic files. No installation, converter,
remote CI or GitHub push occurred. Existing user windows/edits were preserved.

---
# Latest verification - bookmark export confirmation, 12 November 2026

This supersedes the blanket bookmark rejection in the earlier M3 records.

- scripts/check.ps1 passed: formatting, workspace/all-target denied-warning Clippy,
  **32 unit tests**, and release workspace build. Current debug desktop also built.
- Debug and release native smoke passed. Bookmark cases include direct destinations,
  local GoTo actions, legacy and name-tree destinations, nested/sibling outlines and
  combined Info metadata. Without consent, publication is refused and staging cleans
  up. With consent, output actually has no bookmark structures, retains page content/
  sampled raster and passes QPDF/PDFium validation. Rotated selected-page Extract
  requires its own consent and does not advance the clean baseline.
- JavaScript, remote and chained bookmark actions remain blocked. Unit cases also
  cover invalid parents/targets, cycles, unknown keys, missing summary items, unrelated
  annotations and unvalidated orphan actions. Existing source-change/no-clobber and
  other unsafe-PDF checks remain in the native suite.
- Headed release scripts/m3-ui-smoke.ps1 passed the existing two-source organizer/
  Save As/Extract/history/native-cancel/metadata/form checks plus bookmark-only,
  named-bookmark and combined bookmark+metadata dialogs. It asserts warning text,
  Batal retaining dirty with no output, Simpan tanpa bookmark producing a clean full
  export, omitted outlines, expected 90-degree rotation, unchanged source hashes and
  staging cleanup. Evidence: .research/m3-ui-4acc3841c85344269a4cd64f4be11b84.
- PowerShell parsing and whitespace checks passed. The interrupted attempt was not
  counted as completed release/UI evidence; the resumed checks above finished.

Only synthetic fixtures were used. The user's specific PDF was not supplied or
inspected, and complex bookmark actions/other unsupported document features may
still block export. Bookmark preservation/remapping is not implemented: policy v2
allows omission only after explicit snapshot-specific consent. Existing reader/text/
chrome regression results below are historical and were not rerun for this change.
No new engine/runtime, installation, remote CI, converter or push was performed.

---
# Latest verification - M3 safe PDF export, 11 November 2026

This supersedes status claims in historical sections below. Current implementation:
M1 viewer, M2 organizer, M3 PDF Save As/Extract and presentation-only conversion home.

- scripts/check.ps1 passed formatting, workspace/all-target denied-warning Clippy,
  **30 unit tests**, and the release workspace build. Debug workspace build also
  passed. Additional native regression cases were then added; formatting/Clippy and
  debug/release builds passed again. No product logic changed after release UI checks.
- Debug and release native smoke passed: existing M0/M1/text cases plus production M3
  two-source reorder/rotate/delete Save As and selected Extract. It compares source
  text and sampled rasters, tests all 16 intrinsic/virtual rotation combinations,
  Unicode/spaces/ampersand/leading-hyphen paths, metadata detection and consent,
  form/signature-marker/tag/bookmark/annotation/JavaScript/attachment/label/unknown
  rejection, real encrypted input, malformed input, actual QPDF recovery exit 3,
  unused-source filtering, source changes before commit and staging cleanup.
- Unit failure coverage includes no-clobber of an existing hardlink, a competing
  destination created after staging, source changes, no metadata consent, fake engine
  failure, cancellation and output validation mismatch. Real child-process fixtures
  exercise capture overflow, a 100 ms timeout, cancellation and exit code 7. QPDF
  missing/version mismatch and JSON schema/cycle/count/unsafe-object cases are tested.
  The synthetic signature marker is not a cryptographically valid signature fixture.
- Release M3 UI passed: import two sources, move/rotate/delete, Save As seven pages,
  QPDF count/rotation verification, clean baseline plus Undo/Redo, Extract one page
  retaining dirty status, native save cancellation, metadata decline/accept, blocked
  form export retaining a readable viewer, source hashes and staging cleanup.
  Evidence: .research/m3-ui-7bbee9c601fc4ffaaf510ca304f3d33c.
  Debug M3 UI also passed: .research/m3-ui-7d6625f4d15e4e8e9670fc10d9fff22d.
- Release chrome passed Baca/Atur/Konversi, both themes, long filename,
  860/1000/1200/1600 widths, navigation/zoom non-overlap and focused input/Enter.
  Evidence: .research/chrome-ui-2e567cbc4cbf4b9f9879823d4cbf8475.
  Conversion without a document passed after readiness was made explicit:
  .research/chrome-ui-48846d95a2cf4538ae237726790301a4.
- Release M2 UI passed drag/cancel, rotation/move/delete/empty/history/native import,
  exact multi-source mapping and source hashes:
  .research/m2-ui-bf41773522f842cdb9cfbecc49532035.
- Release M1 UI passed empty startup, lazy 100 pages, navigation/boundaries/thumbnails,
  scrolling, rapid 50-400% zoom, fit/resize and unchanged sources:
  .research/m1-ui-010a6ccb74954136ae5bbcb09cf79c32.
- Release text UI passed single-click/jitter, double-click, forward/reverse drag/copy,
  zoom reuse, mode/conversion selection preservation, four rotations, cross-page copy,
  clear/cache bounds and source hashes. Clipboard restored:
  .research/text-ui-a031084328064388a4efd606e56f873f.
- PowerShell parsing and git diff whitespace checks passed. CI includes expanded native
  smoke automatically, but no remote CI execution is claimed.

Harness fixes: native Save filename is Edit ID 1001, Open is Edit ID 1148; do not
match the address toolbar or Search Box. TaskDialog's CCPushButton HWND control ID
is not its logical answer ID; use posted TDM_CLICK_BUTTON with pinned GPUI's negative
answer indices. Chrome waits for accessible readiness. Rapid zoom can disable its
button between UIA IsEnabled and Invoke; the test handles that race and still requires
exact final numeric bounds. Earlier failed attempts are not passing evidence.

All data was synthetic and UI input targeted the test's own HWNDs; no global SendKeys.
Physical mixed-DPI/trackpad/modifiers, broader accessibility, real complex PDF fidelity,
ACL-specific errors, hard native cancellation/isolation, crash recovery and portable
clean-machine/license acceptance remain manual or future gates. No 2000+ page or
universal preservation claim is made. Sources remain read-only; converters remain
Akan datang. See ADR-0003 and m3-safe-export-plan.md for exact export limits.

---

# Historical verification records
# Verification log

Results below were accumulated during the implementation sessions of 9–10 November
2026. See [arsip handoff](archive/PROJECT_HANDOFF_HISTORY.md), recorded 11 November 2026,
for the consolidated status. Earlier sections retain historical test counts and
“left open” observations; these do not establish current process state. No suites
were rerun for the documentation-only handoff.

Current implementation: M1 viewer. Earlier M0 and GPUI Kit rendering checks were
superseded by the checks below; broader product-release acceptance remains pending.

## Selection click behavior (newest results)

Single click no longer selects one character. Drag activation requires movement to
another character beyond 4 logical pixels. Double click selects a sentence, falling
back to the extracted line for unpunctuated labels. Whole-sentence drag extension is
wired; punctuation/decimal/wrapped-line logic has unit coverage.

Workspace tests: 16 passed. Workspace/all-target denied-warning Clippy and formatting
passed; desktop debug/release builds updated. The headed release selection test passed
single click, jitter, double click/copy, pressed-drag hover resilience, reverse drag,
zoom reuse, four source rotations, cross-page copy and clearing. Evidence:
.research/text-ui-9beba3501152498bad944cc76a8d252f. Existing clipboard formats restored.
The test viewer is left open. Sentence segmentation is heuristic; abbreviations and
complex extracted reading order remain documented limitations.

## Text selection follow-up (newest results)

Workspace tests now total 15, all passed; denied-warning workspace/all-target Clippy,
formatting and debug/release builds passed. Real release native smoke includes text
content/newlines, four rotations and raster alignment, empty scan and changed-source
rejection. Headed release selection/copy/zoom/cross-page checks passed; see
[text-selection.md](text-selection.md) for exact scope, evidence and manual gates.

## Trackpad zoom follow-up

The pinned GPUI Windows source exposes native PinchEvent through Direct Manipulation.
The viewport now consumes those events and Ctrl+scroll (capture phase); unmodified
scroll remains navigation. Fractional accumulation, limits, reset/invalid input and
vertical reading-anchor math have unit coverage. Workspace tests passed (12 total),
denied-warning Clippy passed, and current debug/release desktop builds succeeded.
The anchor padding was corrected to include both the 24px label and 8px gap; the
application tests passed again after that correction.

Headed release regression passed: empty startup, lazy rendering, page 100/boundaries,
thumbnail activation, ordinary continuous wheel scrolling, 50-400% button zoom and
resize. Evidence: .research/m1-ui-e96429e64ad84083bbd9b33afaac9fd2. Latest viewer left open.

Physical trackpad pinch and Ctrl+scroll acceptance remain manual: automated checks
do not inject global keyboard input or claim that hardware gestures were exercised.

## Startup fix (newest check)

The no-argument debug executable reproduced a main-thread stack overflow before its
empty window appeared. Document-only release tests had missed this startup path.
The desktop build script now reserves 8 MiB Windows MSVC main-thread stack (committed
on demand). Both debug/release PE headers were checked for this reserve, and both
executables rebuilt. Formatting and desktop all-target Clippy with denied warnings passed.

The headed debug UI test now first opens/closes the empty viewer, then runs its normal
PDF/navigation/zoom/resize checks: PASS. Evidence:
.research/m1-ui-0b2ce65ff7a14524b79e70ac9a34e479. The UI script retains this empty-start
regression check for future runs. All PowerShell scripts parsed successfully.

run.cmd now prefers release, falls back to debug, sets the project working directory
and forwards arguments. Invoking the actual batch file without arguments from a different
working directory produced a responsive release window: PASS. That release window is
left open. Existing book stress results below remain applicable to viewer behavior.

## Book follow-up (newest results)

- Current debug/release builds, formatting, denied-warning Clippy and workspace tests:
  PASS (now 10 unit tests, including preview key isolation).
- Current real release PDFium/QPDF smoke: PASS.
- Eight generated native book cases (100/300/500/1000 vector and image-only pages),
  576 renders, session reuse/recycle/idle/source protection and admission limits: PASS.
- Current release UI with a 1000-page image-only fixture: PASS, all navigation/zoom/
  resize checks and two passes through 100 evenly-spaced destinations each.
- See [books.md](books.md) for exact measurements, reproducible commands and limitations.
  The newer UI evidence uses an isolated fixture copy; its release viewer is left open.

## Original M1 checks before the book follow-up

- Rust 1.98.1, Windows x64 MSVC, GPUI Kit 0.9.1, PDFium Chromium8125, QPDF 12.4.2.
- cargo fmt --all; Clippy workspace/all targets with denied warnings: PASS.
- cargo test --locked --workspace: PASS, nine tests, none ignored. Covers stable
  identities/geometry, metadata-only opening, byte-LRU ownership/eviction, scheduler
  priority/generation/cap, fit/DPR math, navigation bounds and QPDF argument tokens.
- Current debug executable and optimized release workspace build: PASS.
- Real release pdf-smoke.exe: PASS, QPDF selection/order/rotation, pixel identity,
  Unicode/spaces/ampersand paths and unchanged source bytes.
- M1 native integration: PASS, 100-page metadata/workspace, mixed sizes and rotation,
  selected first/last/middle renders, max dimensions/pixels, configured page limits,
  invalid index and modified-source rejection. Synthetic native probe took 73 ms in
  the release run; this is not a general PDF performance claim.
- Headed debug GPU UI: PASS, lazy startup (3 page/4 thumbnail attempts), jump to 100,
  boundary disabled state, thumbnail activation, continuous wheel scrolling, rapid
  50-400% zoom, stale-result rejection and fit recomputation on resize.
- Two debug passes requesting all 100 pages: PASS, bounded 128/16 MiB CPU caches.
  Working set 296.8/302.9 MiB after each pass; final 295.5 MiB, peak 303.8 MiB.
  Final shared GPU counter about 48 MiB, zero GPU-release errors, empty work queue.
  Shared GPU accounting can overlap working set; do not add these figures.
- Changed-source UI probe on its own copied synthetic file: pageCount became zero,
  both caches cleared and rendering stopped. The original fixture was untouched.
- Final headed release UI and two-pass 100-page stress: PASS, including resize.
  Working set after the two passes was 303.6/314.7 MiB; final 306.1 MiB,
  peak 318.9 MiB. Both CPU caches stayed within 128/16 MiB. Evidence:
  .research/m1-ui-093513d7c18f42f3888d2442dedf797f. The release viewer is left open.

Machine: Intel Core i7-13620H, about 15.7 GiB RAM, Windows build 10.0.26300.
Synthetic debug stress evidence: ignored
.research/m1-ui-dd2f60912f2f4aeb98f286e19a7c20b1.

GPU retention was measured during implementation: CPU-only eviction left window
atlas textures resident. Explicit viewport/neighbor GPU lifetimes and deferred releases
with the current Window resolved the growth. See ADR-0005.

## Reproduce

Activate the local toolchains and run scripts/check.ps1 and scripts/smoke-test.ps1.
The native smoke never silently skips missing sidecars. Generate synthetic fixtures
with cargo run --locked -p pdf-smoke -- --write-fixtures. The committed fixtures contain
only generated colored pages and labels, including 100-page and mixed-geometry PDFs.

For an interactive Windows desktop with GPU, built binaries and staged sidecars:

```powershell
./scripts/m1-ui-smoke.ps1 -Profile release -Stress
```

The script targets its own synthetic process via UIAutomation and HWND messages,
never global keyboard injection. It verifies original fixture hashes and leaves its
viewer open. Evidence is numeric JSON and captures of generated fixture pages in
ignored .research. Optional --metrics writes numeric counters asynchronously; it
contains no document path/content. Its parent directory must already exist.

An early release UI run exposed test races at disabled zoom boundaries and a resize
to an already-saved size. The script handles disabled Invoke and selects a different
resize width. These failed runs are not counted as acceptance passes.

## Remaining acceptance

Physical mixed-monitor 100/150/200% DPI is not tested; pure layout/raster DPR math is
covered at 1x/2x. M1 key bindings are wired, but the current UI test exercises native
controls through UIAutomation rather than OS keyboard injection. Earlier M0 keyboard
checks do not substitute for the complete M1 keyboard/accessibility matrix.

Password/complex/hostile PDF breadth, hard native-call cancellation/process containment,
form/signature export preflight, clean-machine portable runtime/license audit and
packaging remain pending. The CI workflow is written but has not run remotely;
headed GPU acceptance is local and is not claimed on GitHub hosted runners.


## M2 organizer - 2026-11-11

Implemented multi-source virtual commands, page selection, reorder/drag, rotate/delete,
import, bounded Undo/Redo, revision/dirty and empty states. See organizer.md for exact
semantics and limitations. Save As/Extract remain M3; sources are never rewritten.

Final checks actually run locally:

- `scripts/check.ps1`: fmt, workspace/all-target Clippy with denied warnings,
  **21 unit tests**, and workspace release build passed.
- `cargo run --locked --offline -p pdf-smoke`: real QPDF/PDFium smoke passed;
  metadata/render/resource/source-change checks passed. Text checks now also cover
  **16 intrinsic/virtual rotation combinations** against actual glyph raster ink.
- `scripts/m2-ui-smoke.ps1 -Profile debug`: native import and organizer UI passed
  before drag coverage was added; evidence `.research/m2-ui-4da7cfa4f77b4ec9a997c78cff6f1176`.
- `scripts/m2-ui-smoke.ps1 -Profile release`: drag/drop, rotate, move, delete,
  Undo/Redo, delete-all/restore, native import, two-source page plan and unchanged
  source hashes passed. Evidence `.research/m2-ui-31aaa601d905498f8c39cdda04d6f371`.
- `scripts/m1-ui-smoke.ps1 -Profile release`: empty startup, lazy 100-page viewing,
  navigation, scrolling, rapid zoom and resize passed. Evidence
  `.research/m1-ui-41a98684860c4b2895595eb58e49e7ce`.
- `scripts/text-selection-smoke.ps1 -Profile release`: click/jitter, double-click,
  forward/reverse, hover while pressed, four source rotations, zoom reuse, cross-page
  copy, clear, cache bound and unchanged source hash passed. Evidence
  `.research/text-ui-3422b39a1186498d8a8443ffd7de6bd7`.

Harness corrections during this session: hidden launch had no usable MainWindowHandle;
visible headed test launch is necessary. UIA import-button invocation can block on the
modal picker. HWND-posted click avoids this. UIA filename traversal incorrectly found
Search Box and Open-button lookup could target another control, causing a wrong fixture
to be selected. Final harness targets native filename Edit ID 1148 and native button ID 1,
and verifies the resulting exact page mapping; failed attempts are not passing evidence.
Only synthetic fixture copies and the test's own window/process handles were used.

The release UI runs preceded the final additional seeded mixed-operation test and
smoke output wording change; production behavior did not change afterward. Final
quality checks and native smoke were rerun after those changes. Test windows were
closed; an earlier hidden/stuck test process required targeted cleanup. No installation,
source rewrite, sidecar upgrade, remote CI run or 2000+ page benchmark was performed.
Physical modifier keys, multi-selection drag/cancel matrix and physical mixed-DPI
acceptance remain manual checks documented in organizer.md.

## Compact Baca / Atur halaman UX - 2026-11-11

Implemented the approved grouped toolbar plan. Header is 44 logical pixels, reading
bar 40, and contextual organizer bar 40 only in Atur halaman. Baca is the default;
mode changes are presentation-only. Theme/import live under Lainnya; Enter replaces
Ke; zoom presets/fit choices live under Zoom; copy appears only for text selection.
Navigation shortcuts and page commands retain their previous domain semantics.

Final local verification:

- `scripts/check.ps1`: fmt, workspace/all-target Clippy with denied warnings,
  **21 unit tests**, and workspace release build passed.
- Native smoke passed earlier in this UX session: pinned QPDF/PDFium, order/rotation,
  resources/source changes and 16 source/virtual rotation glyph alignments. No engine
  or domain behavior changed in the subsequent presentation fixes.
- `scripts/chrome-ui-smoke.ps1 -Profile release` passed: Baca/Atur, both themes,
  long filename, widths 860/1000/1200/1600, navigation/zoom on one non-overlapping row,
  focused page draft surviving scroll and Enter navigation without dirtying content.
  Evidence: `.research/chrome-ui-855a944943694c628b61aa806c636800` (16 mode/theme/width
  screenshots, plus initial views and acceptance.json). Actual test DPI was **96/100%**.
- `scripts/m2-ui-smoke.ps1 -Profile release` passed: mode/selection semantics,
  drag and Escape cancellation without an edit, rotate/move/delete, history,
  empty/restore, native import, two-source mapping and unchanged source hashes.
  Evidence: `.research/m2-ui-3405e25e53f14a6bb6e8b0022b2f3074`.
- `scripts/m1-ui-smoke.ps1 -Profile release` passed: empty startup, lazy 100-page
  opening, Enter navigation, boundary buttons/thumbnail, continuous scrolling,
  rapid 50-400% zoom, resize and unchanged sources.
  Evidence: `.research/m1-ui-d9efab4b715a4d198d96436b3f475e56`.
- `scripts/text-selection-smoke.ps1 -Profile release` passed: single-click/jitter,
  double-click, forward/reverse drag, zoom reuse, mode switch preserving selection,
  all four intrinsic rotations, cross-page copy, clear, bounded cache and source hashes.
  Clipboard was restored. Evidence: `.research/text-ui-383da9ad9f65460d956d07b54e5761a5`.
- Debug viewer regression also passed after the clipping correction:
  `.research/m1-ui-92ea059dfe66490687492eff48630c5a`.

Corrections discovered during validation:

- Raw Button.hover duplicates the Kit's hover style and panics; use the supported
  ButtonCustomVariant instead. Earlier black captures/missing controls were failed
  render evidence, not successful acceptance screenshots.
- Centered navigation exposed an existing text-overlay bug: a scrolled-off page's
  global mouse listener could overlap the toolbar and swallow clicks. Restricting
  down/move to the paint-time content mask fixes this. First/previous navigation,
  scrolling and text-selection regressions passed after the fix. Unrestricted global
  mouse-up is retained. Navigation target math and domain commands were unchanged.
- Page-number projection is deferred outside render, respects focused drafts and
  refreshes on Blur. UIA readiness is awaited; key-down tests use HWND PostMessage
  because GPUI pretranslates queued key messages. Menus without InvokePattern use
  targeted HWND-relative clicks; no global SendKeys/input was introduced.
- The old release executable was in use. Its loaded file was renamed to a unique
  previous-version filename inside target/release, preserving the user's running
  session while the new default executable was built. It was not force-closed.

Physical 150/200% and mixed-monitor DPI checks remain manual; the width/theme matrix
above is not a claim that those physical configurations were tested. Save As remains
M3 and edits remain session-only. Documentation and PROJECT_HANDOFF.md were updated.

## M3 analysis + planned conversion home - 2026-11-11

Scope: analyze safe PDF exporting and future formats; implement only the owner's
explicitly requested Konversi/Akan datang page. M3 Save As/Extract and all conversion
backends remain unimplemented. No engine/model/codec feature/dependency was installed.

- `scripts/check.ps1` passed: fmt, denied-warning workspace/all-target Clippy,
  21 unit tests and workspace release build.
- `scripts/chrome-ui-smoke.ps1 -Profile release -NoDocument` passed: Konversi opens
  without a PDF, all planned cards are present without InvokePattern, and return works.
  Evidence `.research/chrome-ui-fe2186433c9f48ec9b06cd3492d7aed6`.
- `scripts/chrome-ui-smoke.ps1 -Profile release` passed: Baca/Atur/Konversi, light/dark,
  long filename, 860/1000/1200/1600 widths, placeholder-only cards, preserved workspace
  and focused reader input. Evidence `.research/chrome-ui-ce0a270006bb422b9282b55f2a58b833`
  includes 24 theme/width/mode screenshots. Actual DPI remains 96; physical mixed DPI
  is not claimed. Grid uses two/three/four columns as width permits.
- Release M2 UI smoke passed including mode semantics, drag/cancel, history, empty,
  native import, multi-source mapping and unchanged source hashes:
  `.research/m2-ui-b4876d03447d44e0b93905cc1e37da4e`.
- Release M1 viewer UI smoke passed (empty startup, 100 pages, navigation/scroll,
  50-400% zoom, resize): `.research/m1-ui-bc1f43c00fa241f39d2a07147fedc49e`.
- Release text-selection UI smoke passed, now also checking that Konversi hides Copy
  and returning preserves the previous selection, followed by four rotations and
  cross-page copy: `.research/text-ui-e3072931751841588c32518d76a4c1d7`. Clipboard restored.

The analysis also ran bundled QPDF 12.4.2 --job-json-help and structured inspection
on a generated fixture. A synthetic QPDFJob JSON selected A:2,A:1,B:4 and applied
relative rotations, produced three pages passing --check, and retained both source
hashes. Evidence `.research/m3-analysis-289cb38e568e41a597167ce18ae30c7c`. This proves
job-file invocation only; no production export/preflight/atomic-output gate is claimed.
Official reference sources and implementation/quality decisions are recorded in
m3-safe-export-plan.md and conversion-roadmap.md.

A running user release session was preserved by renaming its loaded executable to
`target/release/pdf-studio.previous-dcd498672e83487b849f8cde642b55f9.exe` before building
the new default executable. Acceptance viewers were separate synthetic instances.
