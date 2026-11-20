# Handoff terkini - 19 November 2026

## Riset peningkatan tanda tangan (20 November 2026)

Pemilik meminta analisis iLovePDF dan plan dahulu. Riset primer iLovePDF web dan
Adobe Reader, kondisi Origo serta urutan task/acceptance disimpan di
[roadmap tanda tangan](../research/signature-ux-roadmap.md). QR draw HP terkonfirmasi
panduan resmi; protokol/retensi/default runtime belum diuji. Stempel vendor yang
terbukti berupa upload, bukan generator stempel. Roadmap mencakup UX jenis/metode,
type/font + inisial, stempel/kualitas, pustaka/placement, QR dan epic workflow
request/audit/sertifikat. Semua plan-only; tidak mengubah kode/build aplikasi.
Validasi docs: sumber primer dibaca, tautan lokal dan whitespace diperiksa.
Tidak menjalankan tes produk untuk riset dokumentasi.


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

Commit **cf831e1**: `feat(ui): streamline workspace header and contextual tools`.
Push origin/main berhasil: `66a1a88..cf831e1 main -> main`. Ini konfirmasi pengiriman,
bukan klaim CI remote lulus. Pemilik kemudian menutup aplikasi. Setelah memeriksa
target tidak aktif, build teruji sudah disalin ke target/release/pdf-studio.exe.
Hash terpasang sama dengan build isolated di bawah; run.cmd kini memakai UI baru.
Tidak ada proses/dokumen pengguna yang ditutup oleh agen.

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

Commit **aa85ec1**: `fix(ui): left-align recent file icons and names`.
Push origin/main berhasil, respons `bc00b36..aa85ec1 main -> main`.
Catatan auth/push gagal di checkpoint lebih lama bersifat historis; push ini
mengonfirmasi pengiriman commit sampai aa85ec1, bukan kelulusan CI remote.

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

Commit fitur tersedia: **c6e49ea**. Push origin/main dicoba dan ditolak karena
Git tidak dapat membaca Username GitHub tanpa kredensial (interaktivitas disabled).
Pemilik kemudian menutup aplikasi. Setelah memastikan target tidak aktif, build
teruji disalin ke launcher; hash cocok dan tidak ada proses pengguna yang ditutup
oleh agen. Build isolated teruji:
`target/tab-background-check/release/pdf-studio.exe`, SHA-256
`BA57376005728AA8FA1D36A0D67E33ECBC46D58FB33613DD07E5367CAD564401`.
Build sudah dipasang ke target/release/pdf-studio.exe setelah pemilik menutup
aplikasi. Hash terpasang identik dengan build isolated di atas. Status push diverifikasi
terpisah; cache origin/main tidak membuktikan keberhasilan pengiriman.


## Empty State tanpa baris tab (19 November 2026)

Commit fitur **ac60f1e**: `feat(ui): hide tabs and use GPUI empty state without documents`.
Push origin/main dicoba dan gagal karena Git tidak memiliki kredensial Username
GitHub (interaktivitas dinonaktifkan). Pemilik kemudian menutup aplikasi. Setelah memverifikasi target tidak aktif,
build Empty State teruji sudah disalin ke target/release/pdf-studio.exe. Hash
terpasang identik dengan build isolated yang dicatat di bawah. run.cmd sekarang
memakai build Empty State baru; tidak ada proses pengguna yang ditutup oleh agen.

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

Commit fitur: **6ed7c2c** (`feat(ui): merge menus and custom window controls into one header`).
Push ke origin/main dicoba dan gagal: Git tidak dapat membaca Username GitHub
karena kredensial tidak tersedia dan interaktivitas dinonaktifkan. Tidak ada
konfirmasi pengiriman remote. Build lokal sudah terpasang dan teruji.

Baris logo/nama dan menu digabung menjadi satu header 34 logical px. Menu di kiri,
ruang drag di tengah, kontrol minimize/maximize-restore/close custom GPUI di kanan.
Hit area Windows dipisahkan sehingga menu dapat diklik tanpa menyeret jendela.
Kontrol memakai jalur native Windows; tab tetap di baris berikutnya. Identitas
Origo native/taskbar/About tetap. Detail perilaku: [multi-tab](../multitab.md).

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

Pembaruan 19 November 2026: pemilik mengotorisasi implementasi **Cetak PDF** dan
**Daftar isi** setelah riset. Keduanya kini tersedia. Baca [cetak](../printing.md),
[outline](../outline.md), [testing](../testing.md), serta batas aktual di
[compatibility](../compatibility.md). Publikasi/installer tetap ditunda.

Cetak: toolbar/menu/Ctrl+P, semua/current/rentang workspace, preview, printer/properti,
kertas/orientasi dan fit/100%. Snapshot menyertakan anotasi/tanda tangan tersimpan
maupun belum disimpan. Raster print intent 150 DPI; tidak mengubah baseline dirty.
Microsoft Print to PDF melalui temp + validasi + publikasi file baru tanpa overwrite.
Helper managed PowerShell/.NET di-embed; tidak memakai PDF viewer eksternal.
Port hardware dikenal dan MS PDF saja; file/custom port lain ditolak. Kertas satu
ukuran per job. Output PDF berupa raster; cetak fisik/duplex/warna belum diuji.

Daftar isi: panel kiri Thumbnail/Daftar isi, nested/expand/local direct/named target,
XYZ/FitH/source crop/rotasi dan zoom tetap. Tujuan mengikuti PageId setelah edit;
external actions tidak dieksekusi. Ada empty/error/limit state dan cache bounded per
view. Ini pembacaan navigasi, bukan preservation bookmark pada ekspor. Consent lama
masih berlaku. Tidak membuat daftar isi otomatis dari teks/OCR.

Verifikasi fitur baru: 66 unit tests, fmt, Clippy denied warnings dan release build
lulus. Native outline/print flags/4096 limit, annotation/signature smoke dan default
export/PNG suite lulus. UI akhir: 8 outline + 5 printing lulus; PDF virtual satu
halaman memuat anotasi biru belum disimpan, sumber utuh dan dirty tetap. Bukti lokal
ignored serta gate fisik ada di testing. Helper memakai STA khusus + native dialog
klasik dengan guard dialog bertumpuk, bukan PDF shell verb.

## Git dan pengiriman checkpoint ini

Commit fitur **4736dfbd0d68ada5399f8d856fda01adceb810d0**:
`feat(pdf): add Windows printing and outline navigation`.
Branch `main`, origin terverifikasi `https://github.com/zufarhandito/origo.git`.
Working tree bersih tepat setelah commit fitur; pembaruan status ini docs-only.

**Push belum berhasil.** Percobaan awal tanpa respons dihentikan setelah sekitar
tiga menit dengan menghentikan proses git-remote-https milik sesi ini. Percobaan
ulang dengan interaktivitas dinonaktifkan ditolak:
`fatal: Cannot prompt because user interactivity has been disabled.` dan
`fatal: could not read Username for 'https://github.com': terminal prompts disabled`.
Kartu kredensial aman sudah dibuka dan timeout tanpa jawaban; tidak ada token baru
yang diterima/disimpan. Tidak ada konfirmasi server bahwa commit terkirim.
Jangan menyamakan cache origin/main dengan verifikasi remote; perlu autentikasi
sebelum mencoba push lagi. Tidak ada force-push atau PDF/binary/data privat di commit.

Baseline sebelum implementasi adalah `26a79dd` (riset), yang push sebelumnya juga
terkendala autentikasi. Bagian branding/tanda tangan di bawah adalah riwayat.

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
