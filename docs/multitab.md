# Desktop UI dan multi-tab

## Tawaran pemulihan sesi di welcome (19 November 2026)

Banner pemulihan kini ringkas, berlatar amber transparan mengikuti tema, dengan
teks kecil, satu tombol Pulihkan Sesi di kanan dan X untuk melewati pemulihan.
Peringatan tetap menjelaskan bahwa PDF asli dibuka kembali dan perubahan belum
disimpan tidak dipulihkan. X menggantikan Mulai kosong dan menyimpan sesi kosong;
daftar recent/favorit dan file PDF tidak dihapus.

Membuka dokumen melalui recent, file picker, reopen atau jalur pembukaan lain
menutup tawaran sesi di satu jalur bersama. Keputusan ini berlaku sepanjang window
tersebut, sehingga hasil load metadata terlambat tidak memunculkan tawaran lagi.
Membatalkan file picker tidak menghitung sebagai pilihan dokumen dan tetap
mempertahankan tawaran. Pemulihan sesi asli tetap memakai validasi fingerprint,
partial restore dan metadata posisi baca yang ada.


## Pemilih panel kiri hanya ikon (19 November 2026)

Pilihan UI terbaru mengganti segmented bertulisan dengan dua tombol ikon ghost:
LayoutGrid untuk Thumbnail dan ListTree untuk Daftar isi. Nama ditampilkan lewat
tooltip saat hover dan tetap dipertahankan sebagai accessible label. Pilihan aktif
memiliki latar selected; perilaku panel/Organize/per-view tidak berubah. Catatan
segmented di checkpoint sebelumnya merupakan riwayat, bukan tampilan terkini.


## Pemilih tab di kiri dan navigasi segmented (19 November 2026)

Baris kedua dimulai dengan tombol ikon ChevronDown/ChevronUp (tanpa label visual),
lalu tab dokumen dan +. Nama aksesibel Daftar tab dipertahankan, tooltip berubah
Buka/Tutup daftar tab, dan popup terbuka dari sisi kiri di bawah baris tab.
Header tiga baris dan Empty State tanpa baris tab tetap berlaku.
Urutan kiri toolbar utama: Panel navigasi, Buka PDF, Simpan sebagai, Cetak PDF,
Anotasi. Thumbnail/Daftar isi memakai TabBar segmented bawaan GPUI-kit, bukan dua
button ghost; pilihan tetap per view. Dalam Organize, Thumbnail ditandai aktif;
memilih Daftar isi kembali ke mode baca seperti sebelumnya.


## Koreksi susunan tiga baris (19 November 2026)

Arahan terbaru pemilik: baris 1 hanya hamburger, ruang drag dan kontrol jendela;
baris 2 berisi tab, + dan Daftar tab; baris 3 berisi toolbar ikon dokumen.
Saat tidak ada PDF, baris tab tetap hilang dan tampil Empty State/recents di bawah
header. Tab picker dimulai di bawah baris tab (68 logical px). Toolbar anotasi
kontekstual dan perapian canvas/status dari checkpoint sebelumnya tetap berlaku.
Uraian satu baris gabungan menu+tab di bawah merupakan checkpoint sebelumnya.


## Workspace ringkas dan menu aplikasi (19 November 2026)

Header sekarang satu baris: ikon Menu aplikasi (hamburger) di kiri, tab/+ jika ada
dokumen, ruang drag, Daftar tab dan kontrol jendela di kanan. Kelompok Berkas,
Edit, Tampilan, Alat dan Bantuan tetap tersedia sebagai submenu hamburger dengan
action/shortcut/disabled guards yang sama. Saat kosong, baris tab/+ /Daftar tab
hilang; hamburger dan kontrol jendela tetap tersedia di atas Empty State/recents.
Tab picker diposisikan tepat di bawah header (34 logical px).

Main toolbar menyisakan input nomor/total halaman; empat panah dan menu navigasi
compact dihapus dari toolbar. Navigasi menu/keyboard/scroll tetap tersedia.
Pencarian dipisahkan dari zoom oleh separator. Pemicu Anotasi berupa ikon pensil
pada toolbar utama; klik membuka/menutup baris alat kontekstual pada mode baca.
Undo/Redo tampil saat Organize, toolbar anotasi terbuka, atau ada history undo/redo,
sehingga kembali ke mode baca tidak menyembunyikan akses perubahan yang ada.

Label Halaman di bawah tab Thumbnail dan label Halaman N di atas canvas dihapus.
Nomor di bawah thumbnail dan input nomor halaman tetap ada. Status halaman tanpa
teks memakai chip Butuh OCR nonaktif, tooltip OCR segera hadir; tidak ada engine
atau proses OCR. Pesan panjang saat klik halaman kosong tidak lagi dipasang.
Keterangan header/menu klasik pada checkpoint di bawah merekam UI sebelumnya.


## Terakhir dibuka (19 November 2026)

Empty State menampilkan maksimal lima PDF terakhir di bawah tombol Buka PDF.
Ikon dan nama file diratakan kiri pada posisi awal yang sama; nama panjang
dipotong dengan elipsis. Klik nama untuk membuka lewat alur inspeksi PDF biasa; path lengkap tersedia di
tooltip untuk membedakan nama yang sama. File hilang/rusak memakai pesan gagal
pembukaan biasa, bukan dibaca diam-diam atau dihapus dari daftar secara otomatis.

Setiap item menampilkan nama, indikator bintang bila path tersebut ada di favorit,
folder penyimpanan, dan waktu modifikasi filesystem dalam zona waktu lokal
(`dd-mm-yyyy HH:MM`). Bintang memakai state favorit yang sama dengan tab dan tetap
terbaca setelah restart; tidak mengubah urutan recent. Lokasi/nama panjang dipotong
dengan elipsis, sementara tooltip memuat path lengkap dan tanggal perubahan.
Area header dan konten Empty State dibatasi 40 rem (sekitar 640 px), mengikuti
lebar jendela dengan padding tetap lega. Item recent memakai dua baris: nama dan
bintang di atas; lokasi fleksibel di kiri serta tanggal di kanan pada baris kedua.
Lokasi panjang ellipsis agar tanggal tetap terbaca, tinggi minimum item 52 px.
Welcome page dapat discroll bila daftar tidak muat pada tinggi jendela kecil.

Tanggal dibaca melalui metadata filesystem di background untuk lima item terlihat,
saat riwayat dimuat atau pembukaan berhasil dicatat. Tidak membaca isi PDF dan tidak
menyimpan tanggal duplikat di recents.json. Bukan waktu terakhir dibuka dan tidak
memantau perubahan eksternal secara live. Sambil menunggu muncul status loading;
file hilang/tidak dapat diakses menampilkan tanggal tidak tersedia. Revisi cache
mencegah hasil lama muncul setelah clear atau perubahan urutan recent.

Riwayat mencatat pembukaan dokumen yang berhasil, terbaru di atas, tanpa duplikat
path Windows (case/separator dinormalisasi). Maksimal 20 path disimpan lokal dalam
`recents.json` di direktori state aplikasi, terpisah dari sesi/favorit. Tidak ada
isi PDF, thumbnail atau edit belum tersimpan di metadata ini. Restore sesi yang
berhasil membuka dokumen juga dihitung. Aktivasi tab saja tidak mengubah urutan.

Hapus riwayat mengosongkan daftar recent di memori dan disk; tidak menghapus PDF,
favorit atau metadata pemulihan sesi. Pembukaan berikutnya kembali dicatat.
Baca/tulis state berjalan di background, write atomik dan serial berdasarkan
revisi; close menunggu pekerjaan preferensi. Pembukaan saat metadata masih loading
digabung dengan riwayat lama. Metadata rusak/oversize/path invalid menonaktifkan
penyimpanan riwayat dan menampilkan pesan agar file lama tidak tertimpa.


## Tanpa dokumen terbuka (19 November 2026)

Jika daftar tab kosong, seluruh baris multi-tab (termasuk tombol + dan Daftar tab)
disembunyikan. Area utama memakai komponen GPUI-kit Empty dengan EmptyHeader,
EmptyMedia/Icon, EmptyTitle, EmptyDescription dan EmptyContent untuk tombol Buka PDF.
Menu header tetap tersedia. Membuka PDF memunculkan tab; menutup tab terakhir
mengembalikan Empty State. Batal pada picker tetap mempertahankan keadaan kosong.


## Header ringkas (19 November 2026)

Menu Berkas, Edit, Tampilan, Alat dan Bantuan digabung dalam satu baris setinggi
34 logical px dengan kontrol minimize/maximize-restore/close custom GPUI di kanan.
Logo/nama Origo di baris itu dihilangkan; identitas native/taskbar/About tetap.
Ruang kosong menjadi hit area Drag Windows (termasuk double-click maximize),
sedangkan menu tidak menjadi area drag. Kontrol memakai native Min/Max/Close hit
areas, sehingga jalur close Windows tetap melewati guard dokumen yang ada.
Tab dokumen tetap pada baris berikutnya. Tidak ada perubahan model/history PDF.


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
- Bar tab memakai warna tema `tab_bar`; bentuk folder tab aktif memakai `tab_active`.
  Latar keduanya dibedakan pada tema terang maupun gelap agar tab aktif terlihat jelas.

## Pengelolaan tab — tahap 1

- Drag badan tab untuk menyisipkannya sebelum tab tujuan; area sesudah tab terakhir
  menerima drop ke akhir. Tepi strip menggulir saat pointer drag bergerak di sana.
  Escape/drop di luar sasaran tidak mengubah urutan. Ctrl+Tab, Daftar tab dan close
  jendela mengikuti urutan visual. Loading boleh dipindah; modal/close batch mengunci reorder.
- Bintang outline/kuning menandai prioritas tanpa mengubah dirty state, isi atau
  urutan PDF. Favorit permanen lokal berdasarkan canonical path Windows; file yang
  dipindah/rename dianggap berbeda. Daftar tab juga menampilkan bintang.
- Favorit berada di `%LOCALAPPDATA%/PDF Studio/favorites.json`, maksimum 4096 entri
  dan 1 MiB. I/O background, penggantian atomik dan close menunggu penulisan pending.
  Metadata rusak/versi asing tidak ditimpa; tombol bintang dinonaktifkan dengan pesan.
  Gagal simpan mempertahankan penanda sesi dan menampilkan pemberitahuan.
- `--state-dir <absolute-directory>` mengisolasi preferensi untuk pengujian; alternatif
  environment `PDF_STUDIO_STATE_DIR` berlaku jika argumen tersebut tidak diberikan.
  Metadata lokal memuat lokasi file; tidak menjadi bagian diagnostics atau upload.
- Tombol + langsung setelah deretan tab; tetap di luar area scroll saat overflow.
  Pada kapasitas penuh + dinonaktifkan; Open menu/shortcut tetap dapat memfokuskan duplikat.
- Klik kanan tidak mengaktifkan tab. Menu menargetkan identitas tab yang diklik:
  Bintangi/Hapus bintang, Pindahkan ke awal/akhir, Tutup tab, Tutup tab lainnya,
  Tutup tab di kanan. Close banyak tab memakai guard transactional yang sama.

History, dirty revision, seleksi, halaman, zoom, scroll/pan, dan mode alat milik
masing-masing dokumen. Tema berlaku untuk jendela. Save As mempertahankan sumber,
nama tab dan history; nama output dilaporkan pada hasil ekspor.

## Reopen dan pencarian tab — tahap 2

- Berkas → Buka ulang tab tertutup, menu konteks, atau Ctrl+Shift+T membuka tab
  terakhir yang berhasil ditutup. Riwayat metadata dibatasi 20 entri, hanya selama
  proses berjalan; close batch yang dibatalkan tidak menambah riwayat.
- Reopen membuka sumber utama dari disk dengan identitas tab baru dan posisi strip
  sebelumnya. Posisi baca dipetakan ke halaman sumber utama, termasuk setelah reorder
  halaman; halaman impor memakai halaman pertama. Zoom, offset baca dan pan dipulihkan
  setelah ukuran viewport tersedia. Sumber berubah dibuka dari halaman pertama.
- Edit/history/seleksi/pencarian yang telah ditutup tidak menjadi backup. Khususnya,
  Discard tetap membuang edit. Save As tidak mengganti sumber utama yang dibuka ulang.
- Gagal reopen mempertahankan entri untuk dicoba lagi. Kapasitas penuh menonaktifkan
  aksi; PDF yang sudah terbuka difokuskan tanpa membuat duplikat dan tanpa mereset state.
- Daftar tab membuka overlay pencarian nama/folder tanpa mengubah ukuran viewport.
  Filter berupa substring tanpa membedakan kapitalisasi, mengikuti urutan visual.
  Baris menampilkan status aktif, bintang, dirty dan loading. Tab loading belum dapat dipilih.
- Panah atas/bawah memilih hasil, Enter berpindah, Esc atau klik di luar overlay
  menutupnya. Ctrl+F tetap pencarian isi PDF. Hasil kosong mempunyai label eksplisit.

## Restore sesi baca — tahap 3

- Startup tanpa PDF eksplisit menawarkan **Pulihkan Sesi / X untuk melewati pemulihan** bila sesi
  sebelumnya berisi dokumen. Menolak tawaran mengosongkan sesi tersimpan. Startup
  dengan argumen PDF membuka permintaan tersebut tanpa otomatis memulihkan sesi lain.
- `%LOCALAPPDATA%/PDF Studio/session.json` menyimpan maksimal 8 sumber utama, urutan,
  tab aktif, fingerprint dan posisi baca/zoom/pan. Ini metadata lokal dengan lokasi
  file, bukan backup PDF, history, seleksi, pencarian atau edit yang belum diekspor.
- Snapshot diperiksa setiap satu detik dan ditulis hanya saat berubah, di background
  dengan penggantian atomik. Maksimal satu penulisan sesi berjalan dan satu snapshot
  terbaru menunggu. Batas file 1 MiB; versi/struktur/posisi yang tidak valid ditolak.
- Close jendela yang disetujui menyimpan sesi sebelum semua tab dilepas, dengan tab
  aktif sebelum prompt close. Save/Discard/Cancel tetap berlaku untuk edit PDF.
  Menutup tab terakhir secara individual menyimpan sesi kosong.
- Restore menginspeksi kembali setiap sumber. File hilang/gagal tidak membatalkan
  PDF lain; fingerprint berubah memulai baca dari halaman pertama. Tab aktif yang
  hilang memakai tab berhasil pertama. Posisi di halaman impor dipetakan ke awal
  sumber utama. Sumber/history asli tetap berlaku setelah Save As.
- Untuk dokumen dirty atau multi-source, hanya sumber utama asli yang dibuka lagi.
  Tawaran restore menjelaskan bahwa edit halaman dan impor belum diekspor tidak pulih.
  Ini bukan autosave edit atau crash-orphan recovery ekspor.
- Metadata rusak tidak ditimpa otomatis; pemulihan/penyimpanan sesi dinonaktifkan
  dengan pemberitahuan. **Berkas → Hapus riwayat sesi** menghapusnya secara eksplisit
  dan menghentikan pencatatan sampai proses berikutnya, tanpa menghapus PDF/favorit.
- Jika simpan sesi saat close gagal, jendela dipertahankan dan pesan ditampilkan.
  Close berikutnya dapat keluar tanpa checkpoint sesi yang gagal. PDF tetap melalui
  guard tersendiri; kegagalan preferensi tidak mengubah receipt atau dirty revision.
- Suite UI memakai `--state-dir` dalam folder fixture ignored; preferensi pengguna
  tidak digunakan untuk menyimpan metadata sintetis. Direktori override tahap 1 tetap berlaku.

## Dua PDF berdampingan — tahap 4A

- Tampilan → **Bandingkan dua PDF…** memilih pasangan dari tab terbuka. Kedua PDF
  tetap memiliki tab, workspace, seleksi, history, query dan posisi baca sendiri.
  Header/border menandai panel fokus. Klik panel atau fokus input mengalihkan sasaran.
- Memakai **Dock GPUI Kit 0.9.1**, dua panel horizontal dan splitter resize, dengan
  minimum panel 320 logical px. Layout docking dikunci; resize tetap tersedia.
  Renderer panel tidak memakai cache subtree agar native accessibility tetap lengkap
  setelah fokus/dialog berubah dalam integrasi ini. Dependency/toolchain tidak dipatch.
- Satu toolbar dan satu organizer mengikuti PDF fokus. Organizer menyebut dokumen
  sasaran; seleksi A tidak diganti oleh seleksi B. Pada jendela sempit organizer
  menjadi drawer dan canvas kedua PDF menolak input di bawah overlay tersebut.
- Ctrl+A/Delete pada input tetap aksi teks; shortcut halaman mengikuti fokus thumbnail.
  Drag halaman lintas PDF ditolak, tanpa mengubah kedua workspace. Ini bukan fitur
  memindahkan halaman antar-PDF dan bukan fitur ekspor **Alat → Split PDF**.
- **Keluar tampilan berdampingan** atau **Tampilan tunggal** mempertahankan PDF dan
  state dokumen. Memilih tab di luar pasangan juga kembali ke tampilan tunggal.
  Menutup tab pasangan tetap melewati guard; keberhasilan menyisakan PDF pasangan
  sebagai tampilan tunggal. Cancel close mengembalikan layout pasangan sebelumnya.
- Dua panel terlihat boleh render/scan Find sekaligus; hanya satu menjadi sasaran
  aksi bersama. Tab tersembunyi tetap berhenti menjadwalkan pekerjaan dan melepas cache.
  Budget jendela tetap page 128 MiB, thumbnail 16 MiB, text 16 MiB dan antrean render
  32; dua panel membaginya rata. Override budget raster yang ada tetap berlaku.
- Resize mempertahankan anchor baca/pan relatif per panel. Ekspor tetap menangkap
  dokumen/revisi asal; berpindah fokus tidak memindahkan job, consent atau receipt.
  Close/zoom bawaan Dock dinonaktifkan agar tidak melewati guard aplikasi.
- Restore sesi baca tetap menyimpan daftar dokumen/posisi, bukan layout split.

## Dua tampilan satu PDF — tahap 4B

- **Tampilan → Tampilan kedua PDF ini** membuka panel kiri/kanan dari dokumen yang
  sama. Jumlah tab tetap satu. Maksimal satu tampilan tambahan disimpan per dokumen;
  aksi berulang memakai kembali state tampilan tersebut.
- Kedua panel berbagi workspace, urutan/rotasi halaman, seleksi halaman, Undo/Redo,
  revisi dan status perubahan. Impor dari panel mana pun menambah model bersama
  dengan identitas sumber yang unik. Inspeksi impor mengunci edit kedua tampilan.
- Posisi baca, zoom/pan, query/opsi/hasil Find, seleksi teks dan anchor seleksi rentang
  masing-masing panel tetap terpisah. Edit memperbarui kedua panel dan membatalkan
  hasil lama. Halaman baca dipertahankan berdasarkan identitas; jika dihapus, panel
  berpindah ke indeks terdekat yang masih ada. Rescan akibat edit tidak melompat ke
  hasil pertama; Enter/F3 tetap dapat menavigasi hasil pencarian.
- **Tutup tampilan** (× pada header panel) menyisakan panel lain tanpa menutup PDF.
  **Tampilan tunggal** menyisakan panel yang dipilih. State tampilan tersembunyi
  disimpan tanpa cache raster/teks/GPU; membuka tampilan kedua memakainya lagi.
  Tab ×/Ctrl+W tetap menutup dokumen, dengan satu guard bersama untuk edit/ekspor.
- Ekspor/PNG/Split PDF dan receipt tetap dimiliki dokumen. Save As dari panel kanan
  tetap selesai dan menandai model bersama clean setelah panel kiri ditutup.
  Menutup tampilan tidak membatalkan pekerjaan dokumen. Duplicate Open memfokuskan
  tab yang sama dan mempertahankan tampilan terakhir yang dipilih.
- Session restore/reopen menyimpan satu entri dokumen dengan posisi baca tampilan
  yang terakhir dipilih. Layout/panel kedua dan edit yang dibuang tidak dipulihkan.
  Sumber yang berubah menginvalidasi kedua tampilan. Budget jendela tetap sama,
  maksimal dua panel terlihat; batas delapan tab tetap menghitung dokumen.

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
Saat tersembunyi, session berhenti menjadwalkan render, membuang cache raster/teks dan
melepas tekstur melalui deferred window GPU release. Edit/history/seleksi/viewport
tetap disimpan. Panel yang terlihat mempertahankan bagian budget cache; default window tetap page
128 MiB, thumbnail 16 MiB dan text 16 MiB, dengan override raster yang sudah ada.
Budget cache bukan batas seluruh process memory. Reload teks setelah switch bersifat
async. Copy dapat memuat ulang halaman seleksi yang keluar dari cache secara bertahap
dengan buffer hasil maksimal 1 MiB; pindah tab membatalkan job copy yang belum selesai.

Batas sumber 1024 halaman/128 MiB berlaku per workspace. PDFium tetap in-process,
tanpa hard native timeout/sandbox. Tidak ada pemulihan edit belum diekspor, detach/multi-window,
converter baru, OCR, atau content editing.

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
