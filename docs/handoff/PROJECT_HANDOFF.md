# Handoff terkini — desktop multi-tab, PNG dan pencarian teks, 17 November 2026

## Fokus berikutnya — pematangan multi-tab

Keputusan dan riset lengkap disatukan dalam
[riset dan TODO multi-tab](../research/multitab-roadmap.md). Urutan yang disetujui:
1) drag/bintang kuning/posisi +/menu konteks; 2) reopen tab dan pencarian Daftar tab;
3) restore sesi/posisi baca; 4) dua PDF berdampingan lalu dua view PDF yang sama.
Split view wajib memakai Dock GPUI Kit dengan pembatas resize. Organizer mengikuti
panel fokus, seleksi tiap PDF terpisah, dan job tetap milik dokumen asal.

Group tab dan tab vertikal menjadi backlog tanpa jadwal. Rincian yang belum dikunci
(termasuk persistensi bintang dan cakupan recovery) ditandai di dokumen. Follow-up
ini hanya menyimpan dokumentasi rencana, bukan mengimplementasikan fitur; jangan
menyamakan TODO/acceptance masa depan dengan kemampuan atau hasil tes saat ini.
Indeks dan prompt ditautkan; blueprint/rencana UI historis dipertahankan.

## Instruksi kerja terbaru pemilik — riset, commit dan push

Saat diminta research fitur, teliti produk kompetitor (Acrobat/Reader, Firefox/PDF.js,
Chrome PDF, Edge PDF dan produk relevan lain) memakai sumber primer. Jangan menebak
perilaku fitur hanya dari kondisi aplikasi ini. Jabarkan bukti, perbedaan, default,
unknown dan rekomendasi sebelum memetakan implementasi lokal. Simpan riset substantif
di `docs/research/`; [riset Find](../research/text-search.md) menjadi contoh.

Setiap fitur yang selesai dan lolos verifikasi harus **di-commit lalu di-push**, dengan
**Conventional Commits**. Instruksi ini berlaku berkelanjutan; target repo saat ini
`origin` = `https://github.com/zufarhandito/origo.git`, branch `main`. Ini menggantikan
status belum ada izin push pada snapshot lama, tanpa mengizinkan publikasi binary,
pengiriman data privat atau eksekusi pekerjaan yang masih plan-only. Lihat
[panduan lengkap](../feature-development.md) dan [AGENTS.md](../../AGENTS.md).

## Follow-up terbaru — pencarian teks PDF aktif

Pemilik menyetujui implementasi Find setelah riset viewer lain. Ketentuan eksplisit:
default **tidak case-sensitive** dan `hasil penelitian` dicari sebagai satu frasa,
bukan dua kata yang tersebar. Fitur sudah diimplementasikan; checkpoint PNG di bawah
adalah keadaan sebelum follow-up ini, termasuk pernyataan sesi hanya dokumentasi.

Ctrl+F/tombol Cari teks/menu Edit membuka bar di bawah toolbar baca, di dalam tab.
Setiap tab punya query/opsi/hasil sendiri. Ada penghitung, sorotan aktif/lainnya,
Enter/F3 dan Shift+Enter/Shift+F3, wrap, Esc/×, serta opsi kapitalisasi/kata utuh.
Whitespace/pergantian baris dalam satu halaman dinormalisasi; tidak lintas halaman.
Pencarian mengikuti urutan workspace/impor dan diulang setelah edit/Undo/Redo.

Pemindaian satu halaman bergantian memakai worker PDFium bersama, debounce 200 ms,
offset hasil ringkas, query maksimum 1024 karakter dan maksimum 10.000 hasil.
Tab nonaktif menjeda scan belum selesai; kembali mengulangnya. Token menolak hasil
lama; native call tetap cooperative. Tidak ada OCR/upload/dependency baru atau
perubahan kontrak ekspor/dirty state. Detail: [search](../search.md).

Verifikasi: fmt/Clippy, **53 unit tests**, release workspace build, search native
smoke dan full native smoke lulus. Final search UI **14 kasus** termasuk 1000 halaman
pause/resume/cache, dan chrome dua tema/empat lebar lulus. Bukti run ada di
[testing](../testing.md). Kode utama: `apps/desktop/src/search.rs`,
`crates/application/src/search.rs`, `apps/smoke/src/search.rs`; harness
`scripts/search-ui-smoke.ps1`. Aplikasi pengguna yang sudah berjalan dipertahankan.

Fitur pencarian sudah di-commit sebagai `a74d380`,
`feat(search): add phrase search to active PDF tabs`. Panduan riset/checkpoint dan
handoff diperbarui dalam commit dokumentasi terpisah; lihat `git log` untuk SHA-nya
dan periksa status/upstream aktual sebelum melanjutkan. Empat perubahan dokumentasi
yang sudah ada saat sesi dimulai dipertahankan dalam checkpoint dokumentasi.
Angka `2124580` di snapshot berikut adalah checkpoint PNG sebelumnya. Tidak ada
verifikasi remote CI atau publikasi portable baru; gate rilis tetap terbuka.

## Checkpoint sebelum pencarian — 17 November 2026

Permintaan terakhir pemilik: **buat project handover seperti sebelumnya**. Sesi ini
hanya memperbarui dokumentasi; implementasi terakhir adalah **PDF → PNG saja**.
Bagian terkini ini menggantikan status Git, instruksi berikutnya, dan jumlah tes pada
snapshot historis di bawah. Arsip tetap dipertahankan sebagai riwayat.

- Saat mulai handover, working tree bersih. Branch `main`, HEAD **`2124580`**
  (`export png`, 17 November 2026), sama dengan cache lokal `origin/main`;
  `git rev-list --left-right --count HEAD...origin/main` menghasilkan `0 0`.
- Commit sebelumnya `2c3033c` (portable candidate/audit), lalu `06c2a39` (migrasi
  M3 UI/device gates). Baseline `582cd67` di bagian lama bukan HEAD terkini.
- Tidak ada fetch, push, atau pemeriksaan remote CI dalam sesi handover ini.
  Cache upstream bukan bukti keadaan server saat ini. Perubahan handover ini
  belum di-commit/push oleh sesi ini; periksa Git lagi saat melanjutkan.

## Implementasi terakhir: PDF → PNG

Menu **Alat → Ekspor PNG…** pada tab aktif menyediakan semua/halaman terpilih,
150 DPI (default) atau 300 DPI, dan folder baru `hasil-png`. Nama gambar
`page-0001.png` dan seterusnya mengikuti urutan batch, termasuk halaman impor dan
rotasi workspace. Seleksi kosong menonaktifkan opsi halaman terpilih.

Plan menangkap identitas/revisi tab asal dan fingerprint sumber. Sumber tetap
read-only; snapshot privat, preflight, validasi, source recheck dan publikasi folder
tanpa overwrite tetap berlaku. Metadata/bookmark biasa boleh diraster dengan
penjelasan kehilangan semantik interaktif; forms, signature, tags, annotations,
enkripsi dan input kompleks/aktif tetap diblokir oleh preflight konservatif.
Pengecualian whole-source/tagged Extract tidak dipakai untuk PNG.

Raster berukuran tepat, terpisah dari clamp zoom/DPR viewer: latar putih opaque,
`round(displayed_points × DPI / 72)`, maksimum 8192 piksel per sisi dan
16 × 1024 × 1024 piksel per halaman. Render → encode → decode validation dilakukan
satu halaman bergantian; decoded RGBA harus identik. Batas output batch 1 GiB.
DPI menentukan dimensi piksel; encoder belum menulis metadata physical-print `pHYs`.

Job memakai slot ekspor/progress/cancel bersama dan tetap dimiliki tab asal meski
pengguna berpindah tab. Close menunggu job, lalu tetap meminta keputusan untuk
edit PDF yang dirty. **Receipt PNG tidak menandai PDF clean**; hanya full Save As
dengan instance/revisi yang sesuai dapat melakukannya. Cancel bersifat cooperative;
native render yang berjalan tidak dapat diputus. Budget kerja PNG 30 menit bukan
hard native timeout. Kegagalan/cancel membersihkan staging milik job; crash-orphan
recovery belum tersedia.

Peta kode untuk follow-up:

- `apps/desktop/src/png_export.rs`: konfigurasi, picker dan job UI; integrasi menu/
  progress berada di `shell.rs`, `views.rs`, `exporting.rs` dan `diagnostics.rs`.
- `crates/application/src/png.rs`: plan/receipt, dimensi, resource limits dan alur
  ekspor; `crates/pdf-engine/src/lib.rs`: exact raster pada worker native bersama.
- `crates/platform/src/png.rs`: encoder/decoder bounded; `split.rs`: helper snapshot
  batch dan publikasi folder yang digunakan bersama Split.
- `apps/smoke/src/png.rs` dan `scripts/png-ui-smoke.ps1`: native/UI regression.

Hanya fitur PNG pada dependency `image` yang sudah dipin 0.25.8 diaktifkan;
versi package lock tidak dinaikkan dalam pekerjaan PNG. Baca [PNG](../png.md),
[arsitektur](../architecture.md) dan [kompatibilitas](../compatibility.md).
**JPEG dan TXT di-skip oleh pemilik**; OCR, AI dan converter lain belum ditambahkan.

## Bukti recheck terakhir

Ini hasil pekerjaan PNG yang sudah dicatat pada [testing](../testing.md), bukan
klaim menjalankan ulang seluruh suite saat handover dokumentasi:

- `scripts/check.ps1`: fmt, Clippy workspace/all-target dengan denied warnings,
  **49 unit tests**, dan release workspace build lulus.
- Full native smoke lulus, termasuk M1/text, M3/Extract, Split, Optimize dan PNG.
  Final release `--png-smoke` lulus: geometri campuran, 150/300 DPI, 16 kombinasi
  rotasi sumber/virtual, kesamaan pixel hasil decode, seleksi/urutan, blockers,
  limit, source change, failure/cancel, collision/race dan cleanup.
- PNG UI: **11 kasus bernama lulus pada build lokal dan 11 pada executable hasil
  ekstraksi portable**, termasuk tab asal, cancel dari tab lain, close yang menunggu
  job serta PNG sukses yang tetap tidak mengizinkan dirty-close tanpa keputusan.
- Regresi tabs, **25 kasus M3**, Split dan Optimize UI lulus. Screenshot dialog
  dan PNG berotasi diperiksa secara visual. Path bukti per-run ada di testing.
- Kandidat internal `dist/pdf-studio-windows-x64-candidate-2c3033c-0442227c.zip`
  lulus manifest 1103 file, checksum, x64 runtime/import checks dan lima negative
  integrity checks. PNG UI portable diuji dengan restricted PATH dari path Unicode.
  Marker kandidat berasal dari source dirty sebelum commit PNG; nama ZIP tersebut
  bukan build bersih dari HEAD `2124580`. ZIP tetap ignored dan belum dipublikasikan.

## Konteks AI berbayar yang ditunda

Pemilik menginginkan AI **online**, dengan model disediakan aplikasi dan pelanggan
membayar kepada pemilik. ACP/Zed hanya inspirasi integrasi; provider/model belum
dipilih. Saat ini belum ada backend AI, akun, pembayaran, upload, atau implementasi
fitur AI. Pemilik meminta memikirkan model bisnis dahulu, lalu beralih ke PNG.

Kesepakatan bisnis dari diskusi yang perlu dibawa ke sesi berikutnya:

- Audiens mahasiswa/peneliti; Indonesia lebih dahulu dengan harga Rupiah.
- Reader/organizer gratis, trial AI terbatas sekali per akun; paket mencakup
  pendamping baca serta riset multi-PDF: tanya jawab dengan sumber/halaman,
  penjelasan teks, ringkasan, perbandingan dan sintesis dokumen.
- Beta **Rp49.000 per 30 hari**, prepaid dan perpanjangan manual, tanpa autodebit.
  Model usage termasuk paket, tanpa akun provider terpisah milik pelanggan.
- Target biaya AI maksimum **Rp10.000 per pelanggan per 30 hari**. Alokasi operasi/
  pembayaran Rp5.000 masih asumsi; sisa Rp34.000 bukan proyeksi laba karena biaya
  tetap, pajak, support dan akuisisi belum dihitung.
- Tampilkan kredit AI dan estimasi biaya kredit sebelum tugas berat. Saat kuota habis,
  tunggu periode berikutnya; top-up belum termasuk beta. Pilot 10–20 pengguna undangan.
- PDF tetap lokal; hanya konteks teks yang diperlukan dikirim saat fitur AI digunakan
  dengan persetujuan pengguna. Beta tidak menyediakan library PDF cloud.

Angka **100 kredit/paket**, **10 kredit trial selama 7 hari**, biaya internal
Rp100/kredit, maksimum 5 PDF, target 5 pembayar/3 renewal dan stress biaya +25%
adalah **usulan/asumsi untuk divalidasi**, bukan kuota produksi atau ekonomi yang
sudah terbukti. Belum ada file `docs/plans/AI_BUSINESS_MODEL.md`; keputusan diskusi
dicatat di handover ini agar tidak hilang. Jika melanjutkan AI, langkah berikutnya
adalah menulis/meninjau model bisnis tersebut dan memvalidasi paket/biayanya sebelum
implementasi teknis. Jangan menganggap handover ini sebagai instruksi membangun AI.

## Sisa pekerjaan dan batas sesi berikutnya

Fitur PNG sudah diimplementasikan; tidak ada persetujuan baru untuk JPEG/TXT atau
fitur lanjutan. Pilih pekerjaan berikutnya dari arahan pemilik, bukan nomor prioritas
historis. Jalur yang masih terbuka:

- Gate fisik P4: 150/200%, mixed-monitor, physical modifier shortcuts,
  trackpad pinch/scroll/pan dan Narrator. Bukti otomatis pada satu display 100%/DPI96
  tidak menutup gate tersebut; lihat [device acceptance](../device-acceptance.md).
- Acceptance pada Windows 10 22H2/11 x64 yang bersih, tanpa toolchain/engine eksternal.
- Lisensi proyek/review redistribusi, entitlement MSVC, tiga gap notice
  (`mac 0.1.1`, `seahash 4.1.0`, `simd_helpers 0.1.0`) dan review source MPL
  `option-ext 0.2.0`; lihat [audit distribusi](../distribution-audit.md).
- Review security native dan disposition empat advisory unmaintained transitive
  (`instant`, `paste`, `rustybuzz`, `ttf-parser`). `anyhow` sudah 1.0.103 untuk
  RUSTSEC-2026-0190. Jangan menyatakan vulnerability-free atau public-release ready.
- Model bisnis AI tetap pekerjaan dokumen/validasi yang ditunda, seperti di atas.

Tidak perlu mengulang semua test produk untuk perubahan handover saja. Untuk kode
baru nanti, pilih check yang relevan; headed UI suite dijalankan bergantian pada
HWND/PID fixture sendiri. Jangan menutup aplikasi/dokumen pengguna.

## Menjalankan dan melanjutkan

Rust dipin **1.98.1**, GPUI Kit **0.9.1**, pdfium-render **0.9.5** / PDFium **8125**,
QPDF **12.4.2**. `run.cmd` memilih release lalu debug sebagai fallback, tanpa build.
Command dari root repo, PowerShell:

```powershell
. ./scripts/use-build-environment.ps1
./scripts/check.ps1
./scripts/smoke-test.ps1
cargo run --release --locked --offline -p pdf-smoke -- --png-smoke
./scripts/png-ui-smoke.ps1 -Profile release
```

Helper build menyiapkan Rust serta MSVC/SDK untuk sesi shell itu. Jangan install
ulang rutin, menghapus/memindahkan `.tools`, atau merusak registered VS/SDK junction.
Gunakan UTF-8 eksplisit. Pertahankan perubahan pengguna dan recheck Git sebelum edit.
`.research`, `.tools`, `sidecars`, `target` dan `dist` tetap ignored; catatan
`.research/PROJECT_HANDOFF_PRIVATE.md` dan PDF pengguna tidak boleh dibaca/disalin
ke dokumentasi, commit atau upload. Gunakan fixture sintetis untuk tes.

[Prompt siap salin](NEXT_CHAT_PROMPT.md) memuat konteks ringkas sesi selanjutnya.
Snapshot di bawah dan [arsip lengkap](../archive/PROJECT_HANDOFF_HISTORY.md) bersifat
historis, termasuk status push ditolak, commit lama, dan instruksi UI pra-implementasi.

## Snapshot distribusi dan verifikasi akhir

Setelah checkpoint `06c2a39`, pengguna meminta prioritas berikutnya sampai terakhir,
lalu commit/push hasilnya. Judul P6/7 tidak tersimpan di checklist; cakupan yang dipakai
adalah audit runtime/notice, kandidat ZIP portable dan verifikasi/readiness. Detail:
[distribution audit](../distribution-audit.md), [release checklist](../release-checklist.md).

Builder/verifier/integrity negative tests dan UI smoke paket tersedia di scripts.
Runtime MSVC app-local, sidecars dari arsip hash-verified, inventory Cargo/notices,
manifest file dan checksum ZIP ikut kandidat. ZIP lokal tetap ignored di `dist/`;
Git push hanya source/docs/notice, tanpa publikasi binary atau release tag.

`anyhow` dipin ke 1.0.103 untuk RUSTSEC-2026-0190. Scan OSV ulang 463 versi registry
menyisakan empat pemberitahuan unmaintained pada dependency GPUI; bukan diabaikan.
18 notice tambahan punya provenance commit/SHA; tiga gap notice, lisensi proyek/
review distribusi, native security, clean Windows serta tes fisik P4 masih gate rilis.
Jangan menyebut production release atau P4 selesai. Bukti aktual di [testing](../testing.md).
Gunakan `git log/status` dan hasil push yang benar-benar teramati untuk status Git terkini.

## Snapshot prioritas 3–5

Pengguna meminta lanjut sampai prioritas 5 dan **recheck satu per satu**. Baseline
Git sesi ini `582cd67` (`feat: tab`), working tree bersih dan sama dengan cache
origin/main saat mulai. Checkpoint lokal terbaru dilihat melalui `git log`; sesi
ini tidak melakukan fetch/push atau mengklaim hasil remote CI.

- **P3:** M3 UI dimigrasikan dengan 25 kasus bernama yang lulus per-kasus, termasuk
  consent/tag/Ink/bookmark, native output checks, existing-file no-overwrite,
  failed/cancelled Save, source/hash dan cleanup. Snapshot asli tetap byte-identical
  di `scripts/archive/m3-single-workspace-ui.ps1.txt`, cocok dengan blob baseline.
  Lihat [coverage map](../m3-ui-coverage.md).
- **P4:** audit otomatis pada display nyata yang terhubung lulus: DPI 96 (100%),
  skala GPUI/viewport, UIA roles/names/ValuePattern/focus serta targeted HWND
  Enter/PageUp/PageDown/Tab/Esc. Baru satu display 1920 × 1200; 150/200% fisik,
  mixed-monitor, pinch trackpad, physical modifier shortcuts dan Narrator masih
  pending hardware/human. **Jangan menyebut P4 selesai penuh.**
- **P5:** docs aktif/entrypoint/handoff diselaraskan, coverage historis tetap ada,
  syntax/link/diff dan quality/native/UI gates direcheck sebelum checkpoint lokal.

Detail status dan langkah manual: [checkpoint 3–5](../checkpoint-priorities-3-5.md),
[device acceptance](../device-acceptance.md), dan [testing terbaru](../testing.md).
Untuk fixture manual: `scripts/device-acceptance.ps1 -Profile release -LeaveOpen`.
Tidak ada perubahan policy ekspor atau milestone portable/converter pada pekerjaan ini.

## Snapshot prioritas 1–2: hardening, profiling dan tab utuh

Pengguna meminta prioritas 1 (hardening multi-tab) dan 2 (profiling memory), serta
nama PDF/× menjadi satu tab dan Daftar tab menjadi satu control berlabel/caret.
Sekarang memakai GPUI Kit TabBar/Tab folder + close suffix, close tab nonaktif
tidak memindahkan tab aktif, dan copy dapat reload seleksi offscreen secara
bounded. Diagnostics memproyeksikan seluruh session/empty state; native retained
document dan Windows GPU process counters diukur, bukan disimpulkan dari cache.

47 unit tests, Clippy/fmt/release build dan native smoke lulus. Release hardening
termasuk source change/reopen, receipt origin dan race cancel ekspor tab lain saat
dirty-close menunggu slot lulus. Profil 3 putaran delapan buku 1000 halaman lulus:
peak working set 209,53 MiB, private maximum terukur 162,81 MiB, GPU shared maximum
70,50 MiB; warmed endpoint private growth 0,438 MiB dan GPU tetap stabil.
Lihat [hardening/profile](../multitab-hardening.md) dan [testing terbaru](../testing.md).
Profil tambahan termasuk fase 400% juga lulus: peak working set 509,99 MiB,
private maximum terukur 410,25 MiB, GPU shared maximum 185,26 MiB; warmed private
growth 0,875 MiB dan GPU growth nol. Cache page terukur maksimal 120,31 MiB dari
budget 128 MiB. Ini bukti workload sintetis tiga putaran, bukan leak-free universal.

Baseline Git yang ditemukan pada sesi ini **b749691** (`restyle tab`), main sama
dengan cache origin/main; ini menggantikan status HEAD/push lama di snapshot bawah.
Pekerjaan follow-up ini belum di-commit/push. Tidak ada fetch atau pemeriksaan
remote pada sesi ini. Proses aplikasi pengguna dipertahankan; jendela lama memakai
build lama meskipun `run.cmd` sekarang memilih executable release baru.

## Snapshot implementasi awal multi-tab (digantikan follow-up di atas)

Pengguna telah menyetujui rancangan dan meminta **Implement the approved plan**.
UI desktop dan multi-tab kini diimplementasikan: WindowShell, DocumentSession
terpisah, maksimal 8 tab termasuk loading, panel alat saat dipakai, grouped menus,
Open multi-file/deduplikasi, isolasi state dan close batch transactional.
Lihat [perilaku aktual](../multitab.md). Default minimum 860 × 500, drawer <1100 px.

Renderer/session PDFium dipakai bersama. Tab nonaktif membersihkan cache raster/teks
dan melepas GPU melalui deferred Window update; history/seleksi/viewport dipertahankan.
Satu ekspor/modal dikoordinasikan per jendela. Receipt dan async callback tetap milik
entity dokumen asal; kebijakan preflight/consent/no-overwrite tidak dilonggarkan.

Verifikasi baru dan lokasi evidence dicatat di bagian terbaru [testing](../testing.md).
Suite `tabs-ui-smoke.ps1` menjadi acceptance lifecycle multi-tab. Suite M3 lama tetap
disimpan; skenario Open-replacement-nya historis dan belum dimigrasikan. Automatic
approval review menolak penggantian suite lama karena menghapus cakupan tes; solusi
yang dipakai adalah mempertahankannya dan menambah suite tersendiri.

Perubahan implementasi dan penataan docs sebelumnya masih working tree, belum
di-commit/push. Jangan mereset perubahan pengguna. HEAD tetap 87ba1c8 sampai ada
commit baru. Tidak ada fetch/push atau verifikasi remote CI pada pekerjaan ini.
Proses release pengguna dipertahankan; executable lama dapat tersimpan sebagai
`target/release/pdf-studio.previous-*.exe` agar release baru dapat dibangun tanpa
menutup proses lama. Jangan menganggap proses yang sudah berjalan memakai UI baru.

## Snapshot sebelum implementasi (historis)

Bagian berikut merekam sesi dokumentasi sebelumnya. Pernyataan bahwa UI/multi-tab
belum dibuat atau belum diizinkan sudah digantikan update di atas.

# Snapshot sebelum implementasi multi-tab (historis)

**Mulai dari update di atas dan [perilaku multi-tab](../multitab.md). Bagian di bawah
mempertahankan konteks sebelum implementasi, bukan status terkini.**
Command/path kode di bawah mengacu pada root `D:\projects\bpdf` (Windows x64).
Berkomunikasi dalam bahasa Indonesia. [Indeks dokumentasi](../README.md) dan
[prompt siap salin](NEXT_CHAT_PROMPT.md) menjelaskan urutan baca untuk chat baru.

## 1. Instruksi pengguna yang paling baru

Pengguna menilai UI terlalu tersebar dan meminta pola aplikasi desktop: fungsi
dikategorikan, menu yang jelas, serta tab agar beberapa PDF dapat dibuka bersamaan.
Referensi visualnya adalah Acrobat desktop; ambil hierarkinya, bukan seluruh fitur.

Pengguna secara eksplisit berkata **“plan dulu broo. jangan langsung eksekusi”**.
Diskusi menghasilkan rencana yang disimpan di
[UI_REDESIGN_MULTITAB_PLAN.md](../plans/UI_REDESIGN_MULTITAB_PLAN.md). UI baru dan
multi-tab **belum diimplementasikan**. Tidak ada persetujuan implementasi baru
setelah instruksi planning tersebut.

Permintaan terakhir adalah mencatat progress, membuat file rencana terpisah, dan
merapikan dokumentasi ke `docs` untuk handover. Sesi penataan ini hanya mengubah
dokumentasi/lokasinya, bukan kode aplikasi. Jangan otomatis mulai redesain, konverter,
packaging atau fitur lain. Jika chat baru belum menyebut task, review rencana dahulu.

## 2. Keadaan aplikasi yang sudah nyata

- Rust workspace, GPUI Kit 0.9.1, PDFium 8125/pdfium-render 0.9.5 dan QPDF 12.4.2;
  Rust dipin 1.98.1. Engine berjalan lokal/offline.
- **M1 viewer:** continuous scrolling virtualized, thumbnail, navigasi, zoom/fit,
  horizontal pan, tema, seleksi teks/salin. Single-click membersihkan, double-click
  memilih kalimat. OCR, search dan content editing belum tersedia.
- **M2 organizer:** seleksi halaman, reorder, rotate/delete, import multi-source,
  Undo/Redo bounded dan identitas halaman stabil; edit non-destruktif.
- **M3 output:** Save As penuh dan Extract terpilih ke PDF baru, tanpa overwrite.
  Hanya receipt full Save As yang cocok dengan instance/revisi dapat menandai clean.
- **Unsaved guard:** Open/close meminta Simpan sebagai/Buang/Batal bila dirty.
  Open gagal/batal mempertahankan workspace. Close menunggu ekspor dan cleanup.
- **M4 parsial:** Split per N/rentang manual ke folder baru, serta Optimize lossless
  ke PDF baru sudah tersedia di menu Lainnya. Keduanya tidak menandai edit tersimpan.
  Portable release, audit distribusi, dan hardening M4 belum selesai.
- **Optimize:** membandingkan ekspor standar dari page plan yang sama dengan hasil
  kompresi object/Flate; bukan total ukuran sumber. Tidak meraster atau melakukan
  JPEG re-encoding. Hasil valid bisa lebih besar dan dilaporkan jujur.
- **Konversi:** halaman kartu “Akan datang”, bukan converter yang berfungsi.
- Saat ini satu jendela masih memiliki satu workspace. Open mengganti workspace
  melalui guard; membuka file baru sebagai tab adalah perubahan yang baru direncanakan.

Rincian perilaku: [organizer](../organizer.md), [unsaved guard](../unsaved-changes.md),
[Split](../split.md), [Optimize](../optimize.md), [kompatibilitas](../compatibility.md).

## 3. Batas dan kebijakan yang wajib dipertahankan

- Sumber tetap read-only. Snapshot privat, preflight QPDF, staging, validasi QPDF/
  PDFium, source recheck, flush dan no-clobber commit mendahului publikasi.
- Kebijakan ekspor **v4**: whole-source Selected Extract dalam urutan asli tanpa
  virtual rotation dapat byte-identical, termasuk pengecualian empty-form/tagged
  yang sudah tervalidasi. Partial single-source tagged Extract membutuhkan consent
  “Simpan tanpa tag”; passive Ink yang didukung dipertahankan.
- Bookmark/metadata loss membutuhkan consent. Active forms/XFA, signatures,
  encryption, actions dan kompleksitas lain di luar dukungan tetap diblokir.
  Optimize mengikuti kebijakan full Save As, bukan bypass whole-source Extract.
- Default per workspace: 1024 aggregate source pages, total sumber 128 MiB,
  history 128. Bitmap 8192 per sisi/16 juta piksel. Cache CPU page/thumbnail/text
  default 128/16/16 MiB; budget bukan total process-memory limit.
- QPDF JSON/job/patch 16 MiB, stderr 64 KiB, cumulative child-process budget 120 detik.
  PDFium in-process, cancellation cooperative; belum ada hard native timeout/sandbox.
- Fingerprint size/mtime, bukan hash kriptografis. Belum ada autosave, session restore
  atau crash-orphan recovery. Jangan menjanjikan dukungan dokumen universal.

Lihat [ADR-0003](../adr/0003-export-preservation-policy.md) dan [arsitektur](../architecture.md).

## 4. Verifikasi yang benar-benar sudah dijalankan

Pada implementasi Optimize sebelumnya, bukan pada turn dokumentasi ini:

- `scripts/check.ps1`: fmt, denied-warning workspace/all-target Clippy,
  **45 unit tests**, release workspace build lulus. Debug workspace build juga lulus.
- Full native smoke M1/text/M3/Split/Optimize lulus. Optimize memeriksa exact 512px
  RGBA terhadap baseline plan yang sama, teks/order/rotasi, byte sizes, original JPEG
  bytes, consent/blockers, cancellation/failure/race/source-change dan cleanup.
- Release Optimize UI lulus. Fixture blank valid tumbuh 431 → 521 B dan JPEG kecil
  1571 → 1615 B; hasil UI menyatakan kenaikan, bukan klaim pengurangan.
- Regresi release M3/unsaved guard, release Split dan debug M1 viewer/empty startup/
  100 halaman/zoom/resize lulus. Jendela tes ditutup normal; sesi pengguna dipertahankan.

Bukti lokal ignored dan rincian ada di [testing.md](../testing.md):

| Suite | Folder `.research/` |
|---|---|
| Optimize UI | `optimize-ui-5b6e871cd8b14f77bdbb69626adf3a7a` |
| M3/unsaved UI | `m3-ui-612af32f2ad1469cbf4aef08de67390b` |
| Split UI | `split-ui-1fc72d1242b74d978835dd0613cab04a` |
| Debug M1 UI | `m1-ui-93de63cbfd69453991750c92596dc122` |

Pada sesi handover ini hanya dilakukan pemeriksaan Git, keberadaan executable,
dokumentasi, path dan tautan. **Tidak ada test/build aplikasi yang dijalankan ulang.**
Pemeriksaan dokumentasi: seluruh 25 file Markdown repository dan 87 tautan lokal
valid; `git diff --check` lulus. Isi arsip/blueprint lama dipertahankan, selain
banner penjelas dan penyesuaian tujuan tautan setelah pemindahan.
Remote CI, physical mixed-DPI/trackpad dan clean-machine portable acceptance tidak
diverifikasi pada sesi ini. Bukti historis bukan jaminan proses/fixture masih ada.

## 5. Git dan status push

- HEAD lokal yang diperiksa: **`87ba1c8`**, `feat: add lossless PDF optimization with honest size reporting`.
- Commit sebelumnya: `5b3fea2` Split, `f65cdc7` unsaved transition.
- Branch `main`, cache upstream `origin/main` di `5b3fea2`; lokal ahead 1 saat diperiksa.
- Push Optimize terakhir **ditolak automatic approval review**, karena pengiriman
  kode ke remote belum diotorisasi dengan menyebut tujuan persis secara eksplisit.
  Tujuan yang diminta untuk dikonfirmasi: **https://github.com/zufarhandito/origo.git,
  branch main**. Pengguna belum menjawab konfirmasi tersebut; diskusi berikutnya
  beralih ke UI/planning/dokumentasi. Jangan retry/bypass atau mengklaim sudah push.
- Perubahan penataan dokumen/handoff/rencana pada sesi ini **belum di-commit**.
  Periksa `git status`/`git diff` pada chat berikutnya; jangan mereset perubahan ini.
- Tidak ada commit/push baru, fetch atau remote CI pada sesi penataan dokumentasi.

## 6. Toolchain dan pengulangan tes

Toolchain dan dependencies sudah tersedia di D:. Jangan install ulang rutin,
memindahkan/menghapus `.tools`, atau merusak registered VS/SDK junction ke C:.

```powershell
. ./scripts/use-build-environment.ps1
./scripts/check.ps1
./scripts/smoke-test.ps1
cargo run --locked -p pdf-smoke -- --write-export-fixtures
./scripts/optimize-ui-smoke.ps1 -Profile release
./scripts/m3-ui-smoke.ps1 -Profile release
./scripts/split-ui-smoke.ps1 -Profile release
./scripts/m1-ui-smoke.ps1 -Profile debug
```

Jalankan headed suite berurutan, hanya pada HWND/PID fixture sendiri, tanpa global
SendKeys. Simpan/pulihkan clipboard bila dipakai. Jangan tutup/kill sesi pengguna
untuk rebuild. Jika EXE terkunci, gunakan prosedur rename dalam direktori target
yang sama setelah memverifikasi absolute paths; proses lama tetap memakai build lama.
`run.cmd` memilih release, fallback debug. Periksa ulang proses/path, jangan memakai
PID historis sebagai kebenaran saat ini.

Pertahankan stack reserve 8 MiB, deferred GPU `drop_image` dengan Window yang benar,
bounded caches, generation/source rejection, focused page draft dan clipped canvas
hit-testing. Gunakan ButtonCustomVariant, bukan raw Button.hover. Gunakan UTF-8
eksplisit saat membaca/menulis dokumen di Windows PowerShell.

## 7. Peta kode untuk pekerjaan berikutnya

- `apps/desktop/src/main.rs`: satu `Viewer` saat ini memiliki state jendela sekaligus
  dokumen. Pemisahan shell/session belum dilakukan.
- `chrome.rs`, `views.rs`, `navigation.rs`: pengelompokan kontrol, layout dan input.
- `transitions.rs`, `exporting.rs`, `splitting.rs`, `feedback.rs`: lifecycle,
  ekspor, consent dan modal yang perlu diarahkan ke tab asal saat multi-tab dibangun.
- `crates/domain`: workspace, stable IDs, commands/history/dirty revision.
- `crates/application`: ports, rendering/cache policy, export/split orchestration.
- `crates/pdf-engine`: satu worker/session bounded per renderer; clones berbagi worker.
  PDFium global mutex tetap berlaku. `inspect_output` melepas native output handles.
- `crates/platform`: snapshot, commit no-clobber dan Split publication memakai
  pinned atomicwrites 0.4.4. Jangan menggantinya dengan `fs::rename`, yang dapat
  mengganti folder tujuan kosong pada toolchain/filesystem ini.
- `crates/qpdf-adapter`: preflight v4 dan JSON jobs; tidak memakai shell strings.

GPUI Kit 0.9.1 lokal memiliki `Tab`/`TabBar` dan `AppMenuBar`; sudah diperiksa sumbernya
secara read-only, belum diintegrasikan atau diuji untuk rancangan baru.

## 8. Penataan dokumentasi dan urutan lanjut

Root kini hanya mempertahankan README sebagai pintu masuk. Blueprint berada di
`docs/blueprints/`, konteks aktif di `docs/handoff/`, rencana di `docs/plans/`, dan
handoff lama di `docs/archive/`. Dokumen fitur/ADR yang sudah ada tetap di tempatnya.
Notice lisensi tetap di `LICENSES`. Path kode/command pada semua dokumen relatif
root repository; tautan Markdown relatif file masing-masing.

Selanjutnya: baca rencana, tampilkan tradeoff/default yang perlu direview pemilik,
dan tunggu arahan implementasi. Jangan memperlakukan batas 8 tab atau threshold
1100 px sebagai hasil benchmark atau keputusan produk yang sudah terkunci.

[Arsip handoff lengkap](../archive/PROJECT_HANDOFF_HISTORY.md) dipertahankan untuk
sejarah kasus/fix, bukan instruksi eksekusi saat ini. Catatan privat/PDF pengguna
tetap di `.research`, gitignored; jangan salin ke docs, commit atau upload.
