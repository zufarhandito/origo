# Desktop UI dan multi-tab

PDF Studio memakai satu shell jendela dengan maksimal **8 tab**, termasuk tab yang
sedang dimuat. Menu Berkas, Edit, Tampilan, Alat, dan Bantuan mengelompokkan fungsi;
toolbar baca tetap satu baris. Konversi placeholder tidak masuk navigasi utama.

## Membuka dan berpindah dokumen

- Buka PDF, Ctrl+O, dan tombol + menerima beberapa file. Satu PDF menjadi satu tab;
  tab pertama yang berhasil dibuka menjadi aktif. PDF gagal dirangkum dalam satu pesan.
- Canonical primary-source path Windows yang sama memfokuskan tab lama, tanpa
  mereset edit. Nama sama dari folder berbeda diberi pembeda folder dan tooltip path.
- Impor PDF menambahkan halaman ke workspace aktif, bukan membuka tab baru.
- Permintaan melebihi sisa kapasitas ditolak sebelum inspeksi PDF. Loading memakai
  slot; file gagal melepas slot. Daftar tab dan strip yang dapat discroll menangani overflow.
- Ctrl+Tab/Ctrl+Shift+Tab berpindah; Ctrl+W atau tombol × menutup tab. Tab terakhir
  yang ditutup menampilkan empty state di jendela yang sama.
- Nama PDF dan × berada dalam satu komponen tab; × tetap aksi tutup tersendiri.
  Menutup tab clean nonaktif tidak memindahkan tab aktif. Daftar tab memakai satu
  tombol berlabel dengan caret.

History, dirty revision, seleksi, halaman, zoom, scroll/pan, dan mode alat milik
masing-masing dokumen. Tema berlaku untuk jendela. Save As mempertahankan sumber,
nama tab dan history; nama output dilaporkan pada hasil ekspor.

## Panel dan keyboard

Reader tidak membuka panel alat secara default. Panel alat/Alat → Atur halaman
membuka kontrol seleksi, rotasi, hapus, pindah, impor dan ekstrak di kanan. Kembali
membaca menutupnya dan membersihkan seleksi halaman seperti sebelumnya.

Minimum window 860 × 500 logical pixels. Panel kanan 280 px menjadi overlay drawer
di bawah 1100 px; thumbnail tetap dapat ditutup. Esc dari drawer mengembalikan mode
baca dan fokus dokumen. Kontrol pertama/terakhir masuk menu navigasi pada lebar sempit.
Ini default produk awal, bukan hasil benchmark DPI fisik.

## Close dan pekerjaan latar

Tab dirty meminta Simpan sebagai/Buang perubahan/Batal. Close jendela memeriksa tab
kiri ke kanan; seluruh tab baru dilepas setelah semuanya selesai/disetujui. Cancel
di tengah mempertahankan semua tab. Save yang sudah sukses tetap tersimpan/clean;
Discard sebelumnya belum menghapus workspace sampai seluruh batch disetujui.

Satu ekspor berjalan per jendela. Tab pemilik menunggu ekspor dan cleanup sebelum
ditutup; tab clean lain boleh ditutup. Close yang membutuhkan Save As menunggu slot.
Tetap di aplikasi membatalkan intent close, bukan job ekspor. Open dan mutasi dikunci
selama close batch. Kegagalan/cancel ekspor membatalkan close terkait.

Progress, hasil, error, consent dan receipt terikat entity dokumen asal. Job selesai
tidak memaksa perpindahan tab. Hanya full Save As dengan instance/revision yang cocok
menandai clean; Extract, Split, dan Optimize tetap tidak menandai edit tersimpan.
Kebijakan snapshot/preflight/consent/validasi/no-overwrite tidak berubah.

## Implementasi dan batas

`WindowShell` memiliki tab, identitas `TabId` monotonik, active tab, menu, close batch,
dan koordinasi ekspor/modal. `DocumentSession` adalah entity GPUI tersendiri; callback
memakai weak entity asal dan token/generation yang sudah ada. Indeks tab tidak menjadi
identitas callback.

Semua session memakai clone renderer yang berbagi worker/session PDFium bounded.
Saat nonaktif, session berhenti menjadwalkan render, membuang cache raster/teks dan
melepas tekstur melalui deferred window GPU release. Edit/history/seleksi/viewport
tetap disimpan. Hanya tab aktif mempertahankan cache; default window tetap page
128 MiB, thumbnail 16 MiB dan text 16 MiB, dengan override raster yang sudah ada.
Budget cache bukan batas seluruh process memory. Reload teks setelah switch bersifat
async. Copy dapat memuat ulang halaman seleksi yang keluar dari cache secara bertahap
dengan buffer hasil maksimal 1 MiB; pindah tab membatalkan job copy yang belum selesai.

Batas sumber 1024 halaman/128 MiB berlaku per workspace. PDFium tetap in-process,
tanpa hard native timeout/sandbox. Tidak ada session restore, detach/multi-window,
drag reorder tab, converter baru, OCR, atau content editing.

## Verifikasi

`scripts/tabs-ui-smoke.ps1` menguji isolasi state/history/viewport, duplicate/multi/
partial/capacity Open, Save As dan Optimize, close-batch cancel/save/discard, cache
bounds dan hash sumber dengan fixture/window sendiri. `chrome-ui-smoke.ps1` menguji
dua tema, empat lebar, panel dan input halaman; `m2-ui-smoke.ps1`, Split, Optimize dan
native smoke tetap melengkapi regresi. Lihat [testing](testing.md) untuk hasil aktual.

Suite `m3-ui-smoke.ps1` sudah dimigrasikan ke Open/tab-close/window-close dan tetap
memeriksa ekspor/consent, failure, output identity, hash sumber dan cleanup. Snapshot
single-workspace disimpan sebagai `.ps1.txt` di `scripts/archive/`; padanan kasus ada
di [coverage map](m3-ui-coverage.md). Physical mixed-DPI/trackpad,
profiling corpus/durasi yang lebih luas dan clean-machine portable tetap release gates.

Follow-up hardening, diagnostics seluruh jendela dan protokol hasil profil delapan
PDF besar dijelaskan di [hardening dan profiling](multitab-hardening.md).
