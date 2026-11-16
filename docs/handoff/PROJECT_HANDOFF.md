# Handoff terkini — desktop multi-tab, 16 November 2026

## Follow-up: hardening, profiling dan tab utuh

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
