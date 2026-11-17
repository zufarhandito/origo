# Riset dan TODO pematangan multi-tab

Tanggal riset/keputusan: **17 November 2026**. Baseline dokumentasi: `9478ddc`.
Audiens: pemilik, pengembang, dan agen yang melanjutkan PDF Studio di Windows.

Dokumen ini menjadi sumber tunggal riset dan TODO multi-tab berikutnya. Status
tahap 1–4B: **sudah diimplementasikan**. Lihat [perilaku aktual](../multitab.md) dan
[bukti pengujian](../testing.md). Riset awal hanya dokumentasi; implementasi bertahap
kemudian diotorisasi pemilik. Group tab dan tab vertikal tetap backlog.

## 1. Metode, status, dan kondisi saat ini

Riset memakai dokumentasi resmi Chrome desktop, Firefox desktop, Microsoft Edge,
Adobe Acrobat/Reader, serta source dan tes integrasi GPUI Kit **0.9.1** yang tersedia
di toolchain project. Produk kompetitor tidak diuji langsung dalam sesi ini; source
dan tes upstream yang dibaca juga tidak diklaim sudah dijalankan. Versi browser
terpasang tidak diverifikasi. Fitur pengelolaan tab browser bukan otomatis fitur
engine PDF atau spesifikasi PDF Studio.

Label yang dipakai:

- **Disetujui:** keputusan eksplisit atau usulan yang sudah diterima pemilik.
- **Rekomendasi:** adaptasi yang disarankan, belum menjadi keputusan produk terkunci.
- **Keputusan lanjutan:** perlu ditetapkan sebelum fitur terkait diimplementasikan.
- **Backlog:** kandidat tanpa jadwal dan tanpa izin implementasi otomatis.

Kondisi aplikasi yang sudah ada: maksimal **8 tab termasuk loading**, identitas
TabId stabil, state dokumen terpisah, duplicate Open memfokuskan sumber utama yang
sama, guard close tab/jendela, dan satu slot ekspor per jendela. Viewer/organizer,
history, posisi baca dan Find sudah tersedia. Lihat [perilaku aktual](../multitab.md),
[hardening/profil](../multitab-hardening.md), [Find](../search.md), dan
[arsitektur](../architecture.md).

Pada baseline riset hanya tab aktif menjadwalkan render. Tab tersembunyi menyimpan workspace,
edit/history, seleksi dan viewport dalam RAM, tetapi melepas cache raster/teks dan
residency GPU. Tidak ada session restore/crash recovery. Kondisi ini berasal dari
inspeksi kode lokal, bukan dugaan tentang perilaku kompetitor.

## Keputusan implementasi sesi aktif

Pemilik meminta seluruh tahap dikerjakan berurutan. Tahap 1 memakai favorit permanen
lokal berdasarkan canonical path; rename/move dianggap file berbeda. Klik kanan
mempertahankan tab aktif; reorder loading diperbolehkan dan close batch menguncinya.
Tahap 2: riwayat maksimal 20 tab tertutup, hanya selama proses berjalan, tanpa edit
yang sudah dibuang. Tahap 3 menawarkan restore sesi baca, bukan autosave edit.
Tahap 4A memilih pasangan PDF dari tab terbuka. Tahap 4B berbagi seleksi halaman dan
workspace; pencarian/seleksi teks/viewport/zoom terpisah. Batas dan acceptance lain
berlaku. Rekomendasi historis di bawah bukan klaim semua tahap sudah selesai.

## 2. TODO dan urutan yang disetujui

| Tahap | Daftar kerja | Status |
|---|---|---|
| 1 — Pengelolaan tab | Drag urutan, bintang kuning, posisi tombol +, menu klik kanan | Diimplementasikan; lihat multitab.md/testing.md |
| 2 — Navigasi/pemulihan tab | Buka ulang tab tertutup, pencarian dalam Daftar tab | Diimplementasikan; lihat multitab.md/testing.md |
| 3 — Melanjutkan pekerjaan | Pemulihan sesi terakhir dan posisi baca | Diimplementasikan; lihat multitab.md/testing.md |
| 4A — Dua dokumen | Dua PDF berdampingan dengan Dock GPUI Kit dan pembatas resize | Diimplementasikan; lihat multitab.md/testing.md |
| 4B — Satu dokumen, dua tampilan | Posisi baca terpisah pada workspace dokumen yang sama | Diimplementasikan; lihat multitab.md/testing.md |
| Backlog | Group tab dan tab vertikal | Dicatat untuk nanti; tanpa jadwal |

- [x] 1.1 Drag untuk mengubah urutan tab.
- [x] 1.2 Bintang kuning untuk dokumen prioritas.
- [x] 1.3 Tombol + langsung setelah tab terakhir.
- [x] 1.4 Menu klik kanan tab.
- [x] 2.1 Buka ulang tab tertutup.
- [x] 2.2 Cari tab dalam Daftar tab.
- [x] 3.1 Pulihkan sesi dan posisi baca.
- [x] 4A.1 Dua PDF berdampingan memakai Dock GPUI Kit.
- [x] 4A.2 Organizer, seleksi, fokus, ekspor dan resource bounds saat split.
- [x] 4B.1 Dua tampilan PDF yang sama.

Pin tab, hover preview, urutan recently used/MRU, dan multi-window dievaluasi dari
pemakaian berikutnya; tidak otomatis masuk tahap 1–4. Tidak ada penambahan batas
jumlah tab dalam keputusan ini. Split view di dokumen ini adalah pembagian tampilan,
berbeda dari fitur [Split PDF](../split.md) yang mengekspor beberapa file.

## 3. Riset dan perilaku per fitur

### 3.1 Drag untuk mengubah urutan tab

**Bukti kompetitor:** Chrome dan Firefox mendokumentasikan perpindahan urutan tab
dengan drag sepanjang tab strip. Keduanya juga mendukung perpindahan ke jendela lain;
kemampuan itu tidak dimasukkan ke cakupan reorder awal kita.
[Chrome](https://support.google.com/chrome/answer/2391819?co=GENIE.Platform%3DDesktop&hl=en),
[Firefox](https://support.mozilla.org/en-US/kb/tabs-organize-websites-single-window).

**Disetujui:** pengguna dapat menggeser posisi tab di jendela yang sama.

**Perilaku yang direkomendasikan:**

- Drag badan tab ke kiri/kanan dengan indikator posisi tujuan.
- Bintang dan tombol x tetap mempunyai area aksi sendiri; klik biasa tidak menjadi drag.
- Identitas, edit/history, posisi baca, pencarian dan bintang mengikuti tab.
- Ctrl+Tab, Daftar tab dan pemeriksaan close mengikuti urutan visual yang baru.
- Cancel atau drop tidak valid mempertahankan urutan semula. Overflow perlu tetap
  memungkinkan perpindahan ke tab di luar area yang terlihat.

**Integrasi lokal:** reorder berdasarkan TabId, bukan indeks callback. Jangan membuat
DocumentSession baru atau mengubah content revision. Tentukan perilaku tab loading
dan penguncian drag selama close batch sebelum implementasi.

**Acceptance:** tab awal/tengah/akhir, aktif/nonaktif, nama panjang, overflow, cancel,
serta ekspor/render yang selesai setelah reorder. State dan penerima callback benar.

### 3.2 Bintang kuning untuk dokumen prioritas

**Bukti kompetitor:** Acrobat mengenal starred files untuk melacak file penting;
preferensinya membedakan penyimpanan lokal dan cloud. Firefox memakai bintang untuk
bookmark halaman. Ini mendukung konsep favorit, tetapi tidak menentukan warna atau
cara penyimpanan yang wajib kita salin.
[Adobe](https://www.adobe.com/devnet-docs/acrobatetk/tools/PrefRef/Windows/FavoriteFilesSync.html),
[Firefox](https://support.mozilla.org/en-US/kb/bookmarks-firefox).

**Disetujui:** tab dapat dibintangi sebagai prioritas, dengan ikon aktif kuning.

**Rekomendasi:** tombol outline berubah menjadi bintang terisi di depan nama PDF;
tersedia juga di menu klik kanan dan terlihat dalam Daftar tab. Bintang tidak
mengubah posisi tab, isi PDF, dirty state atau history. Simpan favorit lokal agar
tetap diingat ketika file dibuka lagi; tidak ada upload otomatis.

**Keputusan awal (digantikan keputusan implementasi di bawah):** favorit permanen versus penanda selama tab terbuka belum
dijawab secara eksplisit. Permanen adalah rekomendasi, bukan keputusan terkunci.
Jika permanen dipilih, tetapkan identitas file, perilaku rename/move/file hilang,
serta penanganan gagal tulis preferensi.

**Acceptance:** toggle, reorder, close/reopen, nama sama di folder berbeda, serta
restart dan failure penyimpanan bila persistensi dipilih. PDF dan dirty state utuh.

### 3.3 Tombol + langsung setelah tab terakhir

**Bukti kompetitor:** dokumentasi desktop Chrome dan Firefox menempatkan tombol
tambah di kanan tab terakhir.
[Chrome](https://support.google.com/chrome/answer/2391819?co=GENIE.Platform%3DDesktop&hl=en),
[Firefox](https://support.mozilla.org/en-US/kb/tabs-organize-websites-single-window).

**Disetujui:** kelompok dimulai dari kiri dan + menempel di kanan tab terakhir:

```text
[★ A.pdf x] [B.pdf x] [C.pdf x] [+]    ruang kosong    [Daftar tab]
```

**Rekomendasi:** + mengikuti panjang deretan saat ruang cukup, tetap terjangkau saat
overflow, dan tidak menghalangi Daftar tab. Fungsi tetap membuka picker PDF; halaman
New Tab kosong bukan cakupan perubahan ini.

**Integrasi lokal:** wadah tab saat ini memakai `flex_1()` sehingga mengambil ruang
kosong dan mendorong + menjauh. Perbaiki pembagian ruang tanpa kehilangan scrolling
atau kemampuan menemukan tab aktif. Hormati kapasitas dan loading yang sudah ada.

**Acceptance:** nol/satu/delapan tab, nama panjang, reorder, resize, dua tema, kapasitas
penuh dan picker cancel. Tidak ada overlap atau tombol yang tidak terjangkau.

### 3.4 Menu klik kanan tab

**Bukti kompetitor:** Firefox menyediakan pemindahan ke awal/akhir, penutupan tab
lain/kiri/kanan, dan Reopen Closed Tab. Chrome memakai menu tab untuk pengelolaan tab.
[Firefox](https://support.mozilla.org/en-US/kb/tab-context-menu),
[Chrome](https://support.google.com/chrome/answer/2391819?co=GENIE.Platform%3DDesktop&hl=en).

**Disetujui:** tambahkan menu konteks sebagai bagian tahap 1.

**Rekomendasi menu awal:** Bintangi/Hapus bintang; Pindahkan ke awal/akhir; Tutup tab;
Tutup tab lainnya; Tutup tab di kanan. Tambahkan Buka ulang tab tertutup setelah
tahap 2 tersedia; jangan menampilkan aksi masa depan sebagai fungsi yang sudah siap.

Sasaran adalah tab yang diklik kanan, termasuk yang belum aktif. Aksi tidak berlaku
dinonaktifkan. Semua penutupan melewati guard aplikasi, termasuk ekspor berjalan.
Cancel pada close batch mempertahankan tab; Save As yang sudah sukses tetap berlaku.

**Keputusan lanjutan:** rincian menu dan perilaku fokus saat klik kanan tab nonaktif
ditetapkan saat merancang interaksi, tanpa mengubah sasaran aksi berdasarkan indeks.

**Acceptance:** tab nonaktif/terakhir/dirty, keyboard menu, reorder saat menu relevan,
job berjalan dan cancel di tengah batch. Tidak ada discard implisit.

### 3.5 Buka ulang tab tertutup

**Bukti kompetitor:** Firefox menyediakan Reopen Closed Tab dan Ctrl+Shift+T; Chrome
juga mendokumentasikan pembukaan ulang tab tertutup lewat shortcut.
[Firefox](https://support.mozilla.org/en-US/kb/tabs-organize-websites-single-window),
[Chrome shortcuts](https://support.google.com/chrome/answer/157179).

**Disetujui:** kemampuan ini masuk tahap 2.

**Rekomendasi:** menu dan Ctrl+Shift+T mengembalikan tab yang terakhir berhasil
ditutup; beberapa penutupan dibuka ulang dalam urutan terbalik. Pulihkan posisi baca
dan zoom yang tersedia. Close yang dibatalkan tidak menambah riwayat tab tertutup.
Hormati kapasitas, duplicate Open, serta sumber hilang atau berubah.

**Batas:** membuka kembali file dari disk tidak otomatis mengembalikan edit organizer
yang sebelumnya dipilih untuk dibuang. Jangan memberi kesan bahwa Discard sudah
memiliki backup/recovery. Retensi riwayat dan data apa yang dipulihkan ditetapkan
sebelum implementasi; hindari menahan semua bitmap/workspace tanpa batas.

**Acceptance:** close clean, Save As lalu close, Discard, close cancel, beberapa
penutupan, duplicate path, kapasitas penuh, file hilang/berubah dan gagal reopen.

### 3.6 Pencarian dalam Daftar tab

**Bukti kompetitor:** Chrome memiliki Tab Search; Firefox menyediakan pencarian
tab melalui menu dan address bar. Kedua sumber membedakan pencarian tab dari isi halaman.
[Chrome](https://support.google.com/chrome/answer/2391819?co=GENIE.Platform%3DDesktop&hl=en),
[Firefox](https://support.mozilla.org/en-US/kb/search-open-tabs-firefox).

**Disetujui:** tingkatkan Daftar tab dengan pencarian sebagai tahap 2.

**Rekomendasi:** filter nama PDF dan folder, tampilkan tab aktif, bintang, status edit,
serta pembeda nama sama. Atas/bawah memilih hasil, Enter berpindah, Esc menutup.
Tampilkan hasil kosong dengan jelas. Ctrl+F tetap mencari isi PDF pada tampilan
dokumen; jangan membebaninya dengan fungsi pencarian tab.

**Integrasi lokal:** gunakan metadata shell, tanpa ekstraksi isi PDF. Daftar harus
tetap konsisten terhadap reorder/close/loading. Pilihan shortcut dan perilaku filter
dicatat sebelum implementasi, tanpa mengubah default urutan Ctrl+Tab sekarang.

**Acceptance:** delapan tab, nama sama berbeda folder, Unicode/kapitalisasi, hasil
kosong, keyboard, loading/gagal, dan perubahan daftar ketika picker masih terbuka.

### 3.7 Pemulihan sesi dan posisi baca

**Bukti kompetitor:** Acrobat mendokumentasikan pembukaan PDF dari sesi terakhir
serta pengaturan pemulihan tampilan. Firefox menyediakan restore otomatis dan manual.
Kemampuan recovery browser tidak otomatis menjadi kemampuan pemulihan edit PDF kita.
[Acrobat opening](https://helpx.adobe.com/uk/acrobat/using/opening-pdfs.html),
[Acrobat preferences](https://helpx.adobe.com/acrobat/using/viewing-pdfs-viewing-preferences.html),
[Firefox](https://support.mozilla.org/en-US/kb/restore-previous-session).

**Disetujui:** pemulihan sesi dan posisi baca masuk tahap 3.

**Rekomendasi:** ingat daftar PDF, urutan, tab aktif, halaman baca dan zoom di
penyimpanan lokal; sediakan pilihan melanjutkan sesi terakhir. Inspeksi file lagi
saat restore. Satu dokumen gagal tidak membatalkan semua dokumen lain. Tangani
metadata rusak, file hilang/berubah, kapasitas dan startup tanpa dokumen.

**Batas:** pisahkan restore sesi baca dari autosave edit, crash recovery, dan
pemulihan workspace multi-source yang belum diekspor. Nomor halaman sumber lama
tidak selalu setara dengan indeks pada workspace yang sudah diubah. Jangan diam-diam
menganggap hasil Save As adalah sumber baru; semantics Save As sekarang mempertahankan
source/history asli.

**Keputusan lanjutan:** default restore otomatis/manual, informasi yang disimpan,
retensi/clear history, serta cakupan dokumen dirty atau multi-source. Tidak ada
janji pemulihan edit yang belum diekspor sebelum kontraknya ditetapkan dan diuji.

**Acceptance:** restart normal, partial restore, file hilang/berubah, metadata rusak,
kapasitas, serta Save/Discard/Cancel ketika keluar. Restore tidak menimpa PDF.

### 3.8 Dua PDF berdampingan memakai Dock GPUI Kit

**Bukti kompetitor:** Chrome menampilkan dua view dengan satu sasaran toolbar aktif
dan penanda visual fokus; Edge mendukung dua halaman dalam split screen. Ini pola
interaksi browser yang kita adaptasi untuk dua dokumen PDF.
[Chrome](https://support.google.com/chrome/answer/16971124?hl=en),
[Edge](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-policies/splitscreenenabled).

**Disetujui:** gunakan **Dock GPUI Kit**, dua panel kiri-kanan, splitter yang bisa
di-resize, navigasi/zoom/pencarian terpisah dan penanda panel fokus. Aksi dokumen
mengikuti panel yang fokus.

```text
Toolbar: Undo | Redo | halaman | zoom | Atur halaman
┌──────────────────┬──────────────────┬─────────────────────┐
│ A.pdf — AKTIF    │ B.pdf            │ Atur halaman        │
│                  │                  │ Dokumen: A.pdf       │
│ Thumbnail + PDF  │ PDF pembanding   │ 3 halaman dipilih   │
│                  │                  │ Putar/Hapus/Ekstrak   │
└──────────────────┴──────────────────┴─────────────────────┘
                   ↔ splitter Dock
```

**Organizer dan seleksi yang disepakati:**

- Seleksi halaman milik masing-masing PDF. Memilih A:2/4/6 lalu B:10–12 tidak
  mengganti pilihan A. Seleksi panel tidak fokus dapat dibuat lebih redup.
- Satu panel Atur halaman mengikuti PDF fokus, menyebut nama dokumen dan jumlah
  halaman terpilih. Klik kontrolnya tetap menggunakan sasaran tersebut.
- Putar/hapus/pindah/ekstrak/impor/Undo/Redo/Save As menuju dokumen fokus. Setiap PDF
  mempertahankan history dan dirty state-nya sendiri.
- Ctrl+A/Delete pada input mengedit teks; pada thumbnail organizer menjalankan
  aksi halaman. Fokus anak/input harus mengalahkan shortcut dokumen yang tidak sesuai.
- Ekspor menangkap plan/seleksi saat dimulai dan tetap milik dokumen asal meski
  fokus berpindah. Receipt tidak mengikuti panel aktif terbaru.

**Temuan API lokal:** GPUI Kit 0.9.1 menyediakan `DockArea`, `DockLayout::h_split()`,
`DockSkin` dan rendering handle resize. Source builder/Panel/DockEvent serta tes
integrasi tab, perpindahan antargrup dan fokus saat close dibaca. Integrasi dengan
PDF Studio belum diuji. Lokasi yang bisa diperiksa ulang setelah setup toolchain:

- `.tools/cargo/registry/src/index.crates.io-1949cf8c6b5b557f/gpui-kit-0.9.1/tests/integration/dock.rs`
- `.tools/cargo/registry/src/index.crates.io-1949cf8c6b5b557f/gpui-base-0.9.1/src/dock/`
- `.tools/cargo/registry/src/index.crates.io-1949cf8c6b5b557f/gpui-component-0.9.1/src/dock/`

Path ini adalah source dependency lokal/ignored, bukan file yang harus ditambahkan
ke repository. Gunakan versi yang dipin, bukan contoh API lama atau floating main.

**Implikasi arsitektur dan kasus sulit:**

- Pisahkan **terlihat** dari **fokus**. Dua panel terlihat tetap boleh merender;
  hanya satu menerima aksi bersama. Tab tersembunyi tetap berhenti menjadwalkan kerja.
- Anggaran cache berlaku seluruh jendela; jangan memberi masing-masing panel
  seluruh budget lama tanpa pengukuran. Pertahankan shared native worker dan
  generation/source rejection, serta GPU release melalui Window yang benar.
- Resize memperbarui viewport/fit tiap panel. Gunakan ukuran panel, bukan hanya
  lebar jendela, untuk layout sempit, scrollbar, drawer dan hit-testing.
- Penutupan bawaan Dock harus melalui guard aplikasi. `Panel::closable` adalah
  kontrol sinkron; `on_removed` adalah notifikasi pelepasan, bukan pengganti alur
  Save/Discard/Cancel asynchronous. Jangan melepas entity dirty terlebih dahulu.
- Bedakan keluar dari split dari menutup dokumen. Menata ulang Dock tidak boleh
  menghapus workspace, mereset seleksi/history, atau kehilangan pemilik ekspor.
- Menu masuk/keluar split, batas minimum panel, pemilihan PDF kedua dan perilaku
  tab di luar split perlu dikunci sebelum implementasi. Belum menjanjikan
  arbitrary docking, lebih dari dua panel, sinkronisasi scroll atau automatic diff.

**Acceptance:** resize ekstrem, fokus kiri/kanan, input pencarian, seleksi berbeda,
organizer saat split, edit/Undo/Redo masing-masing PDF, ekspor lalu pindah fokus,
close dirty/cancel, keluar split, clipping drawer, serta CPU/GPU/cache bounds.
Tambahkan regresi sumber berubah pada satu panel tanpa merusak dokumen lain.

### 3.9 Dua tampilan PDF yang sama

**Integrasi lokal tahap 4B (diimplementasikan setelah checkpoint 4A):**

- Satu model `Workspace` dalam shared handle, termasuk history/revisi/seleksi halaman.
  Shared handle juga mengoordinasikan identitas sumber impor dan status inspeksi impor.
  Clone handle tidak menyalin workspace/history. Invalidation sumber mengosongkan model
  bersama, bukan hanya salah satu view.
- Bedakan `TabId` dokumen dan `ViewId` tampilan. Satu tab memiliki actor dokumen utama
  serta maksimal satu view tambahan yang state bacanya boleh disimpan saat tersembunyi.
  Actor utama tetap hidup sampai dokumen ditutup, walau view utamanya sedang tidak terlihat.
- Export/PNG/Split PDF/unsaved transition tetap milik actor utama. Aksi dari view kedua
  meneruskan permintaan sebelum menangkap plan; progress/control/notice dapat diproyeksikan
  ke view lain, tetapi job/receipt tidak digandakan. Close view tidak memutus job dokumen.
- Observer revisi memperbarui jumlah halaman, cache, token teks dan Find view lain tanpa
  membersihkan seleksi halaman bersama. Pertahankan anchor `PageId` masing-masing view;
  jika halaman itu dihapus, pilih indeks terdekat yang masih ada.
- Viewport, query/opsi/hasil Find, seleksi teks dan shift-selection anchor adalah state view.
  View pilihan terakhir menjadi tampilan tunggal dan sumber posisi baca sesi/reopen.
  Actor tersembunyi tetap harus mengembalikan fokus dialog ke view yang terlihat.
- Semua views diperiksa untuk loading/modal dan dihitung dalam diagnostics/cache/queue.
  Maksimal dua view terlihat; budget window tidak bertambah. Dokumen tetap dihitung sekali
  untuk batas 8 tab, riwayat closed-tab, guard close dan restore sesi baca.
- Tambahkan aksi eksplisit membuka view kedua dan menutup view. Tab ×/Ctrl+W menutup
  dokumen dengan guard; menutup salah satu view mempertahankan dokumen dan edit bersama.

Ini keputusan implementasi internal, bukan bukti perilaku kompetitor. Hasil aktual
ada di [testing](../testing.md). Verifikasi mencakup import dari kedua view, Undo/Redo,
search invalidation, halaman terhapus, ekspor setelah view asal ditutup, close batch
cancel/save/discard, source changed, serta profil resource dua view.

**Bukti kompetitor:** Acrobat mendokumentasikan split satu dokumen dengan navigasi
dan zoom independen, serta beberapa window untuk dokumen yang sama. Firefox
menyediakan Duplicate Tab; duplikasi browser tidak menentukan cara edit PDF dibagi.
[Acrobat](https://helpx.adobe.com/acrobat/using/adjusting-pdf-views.html),
[Firefox](https://support.mozilla.org/en-US/kb/tab-context-menu).

**Disetujui:** tahap 4B memungkinkan membaca bagian utama dan lampiran dokumen yang
sama dengan posisi baca/zoom terpisah. Edit harus terlihat di kedua tampilan.

**Rekomendasi arsitektur:** satu model dokumen/workspace dengan history, revisi dan
dirty state bersama; dua state tampilan untuk viewport/fokus. Jangan menggandakan
workspace menjadi salinan edit yang dapat bertentangan. Registry duplicate Open
tetap mempertahankan makna yang jelas; membuka tampilan kedua harus aksi eksplisit.

**Keputusan implementasi:** seleksi halaman/history/revisi satu workspace, pencarian
dan seleksi teks per view, organizer bersama mengikuti fokus. Aksi **Tampilan kedua
PDF ini** eksplisit; header × menutup view, tab ×/Ctrl+W menutup dokumen dengan guard.
View tambahan disimpan maksimal satu per tab dan dipakai kembali. Posisi baca
metadata sesi/reopen berasal dari view terakhir yang dipilih; layout tidak dipersist.

**Acceptance:** edit tercermin pada dua tampilan, navigasi independen, invalidasi
pencarian, Undo/Redo konsisten, halaman yang sedang dibaca dihapus dari tampilan lain,
menutup satu view, dan guard ketika dokumen benar-benar dilepas dari view terakhir.

## 4. Backlog tanpa jadwal

### 4.1 Group tab

- [ ] Evaluasi setelah pengalaman penggunaan tahap 1–4 tersedia.

**Bukti kompetitor:** Chrome dan Firefox menyediakan grup bernama/berwarna. Rincian
save/sync berbeda antarproduk; jangan menganggap seluruh perilakunya identik.
[Chrome](https://support.google.com/chrome/answer/2391819?co=GENIE.Platform%3DDesktop&hl=en),
[Firefox](https://support.mozilla.org/en-US/kb/tab-groups).

**Manfaat:** kelompok seperti Referensi skripsi, Dokumen administrasi atau Bab 2.
Sebelum dikerjakan, riset interaksi drag, bintang, split, collapse, close batch,
kapasitas dan restore grup/sesi. Tidak ada keputusan cloud sync, grup otomatis,
kenaikan batas tab atau tanggal implementasi.

**Acceptance calon:** pindah tab antargrup tanpa kehilangan state; cancel menutup
grup dirty; restore yang gagal sebagian. Kriteria final ditetapkan saat diprioritaskan.

### 4.2 Tab vertikal

- [ ] Evaluasi setelah pengalaman penggunaan tahap 1–4 tersedia.

**Bukti kompetitor:** Firefox dan Edge menyediakan tab melalui sidebar.
[Firefox](https://support.mozilla.org/en-US/kb/use-sidebar-access-tools-and-vertical-tabs),
[Edge](https://explore.microsoft.com/en-us/edge/features/vertical-tabs).

**Manfaat:** nama PDF panjang lebih mudah dibaca. Riset penempatan bersama thumbnail,
panel alat, Dock split, mode sempit, keyboard dan transisi orientasi tanpa kehilangan
state. Sidebar tab tidak boleh membuat thumbnail atau PDF aktif sulit dibedakan.
Belum ada pilihan default atau jadwal implementasi.

**Acceptance calon:** ganti horizontal/vertikal, nama panjang, keyboard, resize dan
split; semua identitas/state tetap benar. Kriteria final menunggu prioritisasi.

## 5. Kontrak bersama, validasi dan checkpoint

- Pertahankan source read-only, private snapshots, preflight, consent, validasi,
  no-overwrite dan receipt ekspor. Hanya full Save As yang sesuai menandai clean.
- Reorder, bintang dan layout tidak membuat revisi isi PDF berubah. UI harus selalu
  membedakan prioritas, fokus, dirty state dan progres pekerjaan.
- Batas 8 tab termasuk loading, source/resource limits dan satu ekspor per jendela
  tetap baseline sampai ada keputusan dan pengukuran baru.
- Riset menjelaskan bukti kompetitor terlebih dahulu; adaptasi lokal dan keputusan
  belum terkunci tetap ditandai. Angka/ambang lokal bukan klaim batas kompetitor.
- Implementasikan bertahap sesuai urutan, dengan tes perilaku/failure dan dokumentasi
  aktual pada setiap fitur. Source/test upstream bukan bukti aplikasi kita lulus.
- Sesudah fitur selesai, buat Conventional Commit dan push sesuai instruksi pemilik;
  laporkan SHA/hasil sebenarnya. Catat blocker bila push ditolak, tanpa workaround.
- Dokumen awal ini tidak mengubah kode; implementasi tahap 1 kini dicatat di multitab.md. Rencana UI lama
  tetap historis. Indeks/handoff cukup menautkan dokumen ini, tanpa menduplikasi riset.

Validasi untuk pekerjaan dokumentasi: cocokkan keputusan percakapan dan kondisi
kode, cek sumber primer/tautan, pastikan daftar TODO tidak dicentang sebagai selesai,
dan jalankan diff whitespace checks. Tidak perlu mengulang suite aplikasi untuk
perubahan dokumentasi saja. Tes fitur yang disebut di atas adalah acceptance masa
depan, bukan hasil pengujian baru pada checkpoint dokumentasi ini.
