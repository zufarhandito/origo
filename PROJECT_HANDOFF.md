# Current update - compact reader/organizer UX (11 November 2026)

This supersedes the presentation details in the M2 handoff below. The M2 domain,
non-destructive commands and export limitations remain in force.

- Default Baca mode: compact 44px document header and one 40px reading toolbar.
  Atur halaman opens the sidebar, selects the current page when needed, and adds
  a 40px contextual organizer toolbar. Returning to Baca clears page selection,
  anchor and drag, preserves text selection, and never changes content revision.
- apps/desktop/src/chrome.rs owns ViewerMode, header, grouped reading/organizer
  builders and page-field projection. views.rs owns viewer canvas/sidebar/items.
  Successful import enters Atur halaman; failed import leaves the plan/mode intact.
- Toolbar groups: sidebar/history; navigation; zoom dropdown/presets. Copy appears
  only with text selection. First/last move into navigation menu below 1000 logical
  pixels. Mode/header uses icon + essential labels; other common actions use icons,
  accessible labels and tooltips. Theme choices are in Lainnya.
- The page field displays current position and submits with Enter; the Ke button
  is removed. Its projection updates are deferred outside rendering and do not
  replace a focused draft. Input Blur also schedules projection refresh.
- Organizer contains import, selected count, rotation/delete, Pindahkan and Seleksi
  menus. Page shortcuts/drag are enabled only in Atur halaman's page panel. End-drop
  hint is visible only during a page drag; target has a top insertion indicator.
- Session dirty notice is plain footer text, not a disabled button. Footer includes
  source count and does not repeat zoom. No Save As/Extract placeholder is added.
- IMPORTANT regression fix: page text overlays register global window mouse
  listeners. Capture window.content_mask().bounds at paint time and check it along
  with page bounds on down/move. A partially scrolled page can extend behind toolbar
  or sidebar: checking page bounds alone intercepts clicks in those controls. Keep
  global mouse-up handling unrestricted so a drag ends outside the page.
- GPUI Kit Button already installs hover styling. A second raw .hover() on Button
  panics with "hover style already set". Delete uses supported ButtonCustomVariant
  hover/active colors plus focus-visible style. Do not reintroduce the raw hover.
- Headed tests now use Enter via HWND-posted keyboard messages: Windows GPUI handles
  key-down in the message pump pretranslation, so SendMessage(WM_KEYDOWN) bypasses it.
  Menu controls can lack UIA InvokePattern; use their own HWND-relative bounds for
  fallback clicks, never global input. Wait for accessible controls/layout readiness.
- New scripts/chrome-ui-smoke.ps1 covers mode/theme/width layout, long filename and
  focused page input while scrolling. Existing M1/M2/text smoke follows new controls.
  Final evidence/results are in docs/testing.md; physical mixed-monitor DPI remains manual.
- An already-running release was retained by renaming its loaded executable inside
  target/release to pdf-studio.previous-fe73d6dbc0f84bfdadedefb6e68892a3.exe. This let
  us build the new default executable without closing the user's session. That old
  window continues using the old UI; run.cmd launches the new build. Do not infer
  a currently live PID from this note or delete a mapped executable during a session.

---

# Current handoff - M2 organizer (11 November 2026)

This update supersedes the pre-M2 snapshot preserved below. Communicate with the
owner in Indonesian. Continue reading the blueprint and docs/organizer.md.

- M2 is implemented: multi-source virtual page plan, stable IDs, page selection,
  reorder (keyboard/buttons and thumbnail drag/drop), virtual rotation, delete,
  import/append, bounded Undo/Redo, content revisions, dirty and empty states.
- Save As/Extract and production export remain M3. Edits are session-only; opening
  another PDF or closing discards the virtual plan. Sources are never modified.
- Domain Command validates before mutation and stores affected entries/positions,
  selections/revisions and optional imported source metadata. History cap: 128.
  New edit clears redo; Undo to the opening revision clears dirty. No-op edits do
  not create history. Registry retains sources after delete for undo.
- Import appends one whole PDF per dialog. Duplicate source path/ID is refused.
  Aggregate limits: 1024 source pages (including deleted pages of registered sources)
  and 128 MiB source bytes. Undo import removes its source from the registry.
- Desktop organizer logic: apps/desktop/src/organizer.rs; page plan/commands/history:
  crates/domain/src/lib.rs. Controller resolves every render by SourceId.
  RenderRequest now includes rotation_delta; worker rotates owned RGBA with bounded
  dimensions. TextPage::rotate composes normalized character boxes with virtual rotation.
- Every structural edit clears positional text state, raster caches/GPU residency,
  and advances the token rejecting old text results. This is the chosen conservative
  correctness policy; preserving unchanged caches across edits is a future optimization.
- Ctrl/Shift thumbnail selection; Up/Down, Shift+Up/Down, Ctrl+Up/Down, Space, Ctrl+A,
  Esc in the page panel; Alt+Up/Down move; Ctrl+R/Ctrl+Shift+R rotate; Delete removes.
  Ctrl+Shift+O imports; Ctrl+Z/Ctrl+Y undo/redo. Text selection behavior is preserved.
- The source_sequence counter also invalidates text results and therefore skips IDs;
  no IDs are reused. Source identity is independent of workspace position/revision.
- M2 UI smoke: scripts/m2-ui-smoke.ps1. Native dialog automation sets the filename
  Edit control ID 1148 and clicks the native Open button ID 1 via HWND messages.
  UIA traversal can expose Search Box instead of filename; never use the last Edit
  or first matching automation ID as a fallback. Only target the test's own handles.
- Release organizer, viewer and text-selection headed tests passed; evidence:
  .research/m2-ui-31aaa601d905498f8c39cdda04d6f371,
  .research/m1-ui-41a98684860c4b2895595eb58e49e7ce,
  .research/text-ui-3422b39a1186498d8a8443ffd7de6bd7.
  Source hashes unchanged. See docs/testing.md for final command results.
- Physical modifier-key/trackpad, broad multi-selection drag/cancel and mixed-monitor
  acceptance still need the documented manual matrix. No 2000+ page claim is added.
- Keep toolchain/sidecars on D:, main stack reserve 8 MiB, and deferred drop_image.
  run.cmd still prefers release; release is not yet portable-product certification.

---

## Historical snapshot before M2 (retained for prior evidence)

# PDF Studio — handoff untuk chat berikutnya

Dicatat: **11 November 2026**, zona waktu Asia/Bangkok.
Dokumen ini merangkum sesi pengembangan sampai perbaikan perilaku text selection.
Blueprint utama tetap [PDF_EDITOR_CODEX_BLUEPRINT.md](PDF_EDITOR_CODEX_BLUEPRINT.md).
Jika instruksi terbaru pemilik berbeda dengan blueprint, ikuti instruksi pemilik.

## 1. Baca ini terlebih dahulu

- Proyek native Windows Rust/GPUI Kit di `D:\projects\bpdf`; shell PowerShell.
- **M1 viewer sudah diimplementasikan dan acceptance sintetisnya lulus. M2 belum dimulai.**
- Tambahan sebelum M2: penguatan buku besar, trackpad zoom, text selection/copy,
  dan perbaikan single/double-click. Tambahan ini diminta eksplisit oleh pemilik,
  meskipun text selection awalnya di luar scope Level 1 blueprint.
- Aplikasi masih read-only. Tidak ada fitur organizer, undo/redo, atau Save As produksi.
- `run.cmd` memilih **release**, lalu fallback debug bila release tidak ada.
- Build release berarti compiler dioptimalkan; belum berarti portable product release
  yang sudah lolos seluruh gate M4. Installer/ZIP tidak otomatis mempercepat aplikasi.
- Pemilik ingin aplikasi/dependency baru ditempatkan di **D:** agar C: tidak penuh.
  Toolchain sudah tersedia; jangan menginstal ulang sebagai langkah rutin.
- Sesi ini sebelumnya tidak memiliki `.git`/AGENTS.md. Periksa keadaan terkini sebelum
  membuat asumsi Git, commit, branch, atau aturan lokal baru.

## 2. Progress terhadap blueprint

| Milestone | Status | Bukti/pekerjaan tersisa |
|---|---|---|
| M0 Windows feasibility | Gate rendering/native integration sudah terbukti | GPUI benar-benar menampilkan PDF, dialog native, PDFium dan QPDF nyata. Full physical HiDPI, audit portable runtime/license dan clean-machine acceptance belum selesai. |
| M1 Core foundation/viewer | Diimplementasikan; acceptance sintetis lulus | Domain stable IDs, inspect/open ports, virtual page list, thumbnails, render queue/cache, zoom/fit, native adapter tests dan headed UI tests. |
| Tambahan penguatan buku | Diimplementasikan dan diuji sintetis | Sesi native persisten, recycling/idle release, progressive preview, uji sampai 1000 halaman. Bukan jaminan semua buku scan asli. |
| Tambahan trackpad | Handler diimplementasikan | Native pinch Windows + Ctrl+scroll, anchor zoom dan fractional deltas. Gesture fisik trackpad/mixed-monitor masih manual acceptance. |
| Tambahan text selection | Diimplementasikan dan diuji | Drag, highlight overlay, copy, clear, intrinsic rotation, zoom reuse, cross-page visible selection. OCR/search belum ada. |
| M2 Page organizer | **Belum dimulai** | Belum ada page selection multi-select, reorder, virtual rotate, delete, import PDF lain, command engine/history, dirty revision. |
| M3 Safe exporting | **Belum dimulai untuk dokumen pengguna** | QPDF baru digunakan oleh probe sintetis. ExportPlan, preflight, staging/validation dan Save As belum ada. |
| M4 Split/optimize/release | Belum diimplementasikan sebagai milestone | Build/staging dan bounded rendering ada, tetapi split, optimize, cancellation/process isolation, portable ZIP/clean-machine gate belum selesai. |
| M5 Editing/annotations | Di luar scope saat ini | Tidak ada content editing, anotasi tersimpan, tanda tangan, atau OCR. Highlight seleksi sekarang hanyalah overlay UI. |

## 3. Yang sudah bisa dipakai

### Viewer dan tampilan

- GPUI Kit **0.9.1**, light/dark session-only, native custom title bar, toolbar, icons,
  proper disabled states, loading/empty/error states, scrollbar dan footer.
- Open native dialog / Ctrl+O; bisa menerima path PDF dari CLI atau `run.cmd`.
- Continuous scrolling lintas halaman, daftar virtual, clickable thumbnail sidebar.
- First/previous/next/last, direct page-number field + Ke/Enter, Ctrl+G untuk fokus field.
- PgUp/PgDn, Left/Right, Ctrl+Home/End; toolbar juga punya focus navigation milik Kit.
- Manual zoom 50–400%, reset 100%, fit width/page, horizontal pan.
- Ctrl+plus/minus, Ctrl+0 fit page, Ctrl+1 fit width.
- Native trackpad pinch pada viewport dan Ctrl+scroll; plain two-finger scroll tetap scroll.
  Gesture menjaga reading offset/anchor sejauh batas scroll memungkinkan.

### Perilaku text selection terbaru — penting dipertahankan

1. **Klik tunggal tidak langsung memilih satu karakter**; seleksi lama dibersihkan.
2. Mouse-down menyimpan anchor tertunda. Drag baru mengaktifkan seleksi setelah
   bergerak minimal 4 logical pixels **dan** melewati karakter awal.
3. **Double-click memilih satu kalimat**, sesuai permintaan pemilik. Jika tidak ada
   tanda baca kalimat, fallback ke baris hasil ekstraksi (misalnya judul/label).
4. Drag setelah double-click memperluas menurut kalimat. Segmentasi masih heuristik:
   tanda baca/blank line, decimal ditangani; abbreviation/layout kompleks belum sempurna.
5. Forward/reverse drag, highlight, Salin/Ctrl+C, Esc atau klik area kosong untuk clear.
6. Seleksi bertahan saat zoom/scroll; koordinat tidak diekstrak ulang setiap zoom.
7. Bisa drag melintasi halaman yang **terlihat bersamaan**. Belum ada edge autoscroll,
   whole-document Select All, mode pemilihan kata/paragraf terpisah, atau search.
8. Event hover/repaint tanpa pressed button **tidak boleh membatalkan drag**.
   Akhiri press saat mouse-up (termasuk di luar bounds halaman) atau window deactivation.
9. Scan tanpa text layer menampilkan penjelasan perlu OCR ketika dicoba dipilih.
   Tidak ada OCR otomatis dan tidak ada teks hasil tebakan.

## 4. Arsitektur dan peta file

```text
apps/desktop (GPUI Kit) -> crates/application -> crates/domain
          |                       ^
          +-> crates/pdf-engine --+   (PDFium adapter + dedicated worker)
apps/smoke -> pdf-engine + qpdf-adapter   (generated/native integration)
```

| Lokasi | Tanggung jawab |
|---|---|
| `crates/domain/src/lib.rs` | SourceId/PageId/fingerprint/geometry, read-only single-source Workspace dan invariants. |
| `crates/application/src/lib.rs` | PdfInspector, PageRenderer, OpenDocument, owned RGBA, typed RenderError. |
| `crates/application/src/viewer.rs` | Zoom/DPR/anchor math, GestureZoom, RenderKey, byte LRU, bounded priority queue/generations. |
| `crates/application/src/text.rs` | TextExtractor port, Unicode chars/normalized rects, hit testing, range/copy/highlight merging, sentence boundaries. |
| `crates/pdf-engine/src/lib.rs` | Pinned PDFium, persistent native document session, serialized native work/destruction, fingerprints/resource checks, text extraction. |
| `apps/desktop/src/main.rs` | Composition root, Viewer state, key bindings, CLI, Kit setup, activation observer. |
| `apps/desktop/src/controller.rs` | Async opening/render scheduling, navigation, zoom/layout, CPU/GPU lifetime, optional numeric metrics. |
| `apps/desktop/src/views.rs` | Kit UI, virtual items, preview fallback, canvas overlays and gesture/mouse input. |
| `apps/desktop/src/text_selection.rs` | Lazy text cache/loading, pending press/drag/sentence state, selection and copy actions. |
| `apps/desktop/build.rs` | Windows MSVC executable main-stack reserve 8 MiB. **Jangan dihapus.** |
| `apps/smoke/src/` | Real native QPDF/PDFium tests, fixture generation, book benchmark, text/raster alignment tests. |
| `crates/qpdf-adapter/` | Trusted synthetic QPDF probe; belum exporter produksi. |

### Kontrak/perilaku utama

- Tidak ada native handle atau objek borrowed PDFium yang keluar dari adapter thread.
  UI callbacks tidak melakukan parsing/rendering/extraction/file I/O native.
- Global PDFium binding diinisialisasi sekali dari DLL eksplisit yang dipaketkan.
  Semua native operations/destruction memakai mutex; jangan rebind DLL lain/system DLL.
- Renderer clones berbagi bounded channel ke satu thread native dan satu sesi dokumen.
  Ganti sumber/error/fingerprint mismatch melepas sesi. Idle 15 detik juga melepasnya.
  Sesi diperbarui setelah 64 operasi render/text untuk membatasi retensi internal.
- Opening memeriksa metadata semua halaman; tidak meraster seluruh halaman.
- UI render queue kapasitas 32: visible preview, visible full raster, neighbor, thumbnail.
  Generation membuang pending/hasil usang; native call yang sudah berjalan tidak bisa dipreempt.
- Visible raster >1024px meminta preview 512px lebih dulu. Full cached raster langsung dipakai.
- Render key mencakup sumber/fingerprint/index/rotation/background, raster bucket, DPR dan kind.
- GPU residency dipisahkan dari CPU LRU: viewport + neighbor. Pelepasan texture harus
  melalui deferred window update dan `cx.drop_image(image, Some(window))`.
- Text request satu in-flight, hasil diterima hanya untuk SourceId workspace yang sama.
  Cache teks tidak bergantung raster zoom generation. Thumbnails tidak mengekstrak teks.
- Rect karakter dinormalisasi terhadap displayed page memakai PDFium PageToDevice,
  termasuk intrinsic rotation; config koordinat 16384-square tidak membuat bitmap.
- Fingerprint hanya size + modified time, sebelum/sesudah native read. Tidak ada live
  watcher pada halaman cached, strict hashing, hard native timeout, atau process sandbox.
- `ExportSupport::NotPreflighted`, `Workspace::is_dirty()` masih selalu false.

## 5. Batas saat ini dan ribuan halaman

| Batas | Nilai |
|---|---|
| Halaman input default | **1024**; 1025 ditolak |
| File input default | **128 MiB** |
| Bitmap maksimum | 8192px per sisi; 16 Mi pixels, maksimum 64 MiB RGBA |
| CPU page bitmap cache | Default 128 MiB; env `PDF_STUDIO_PAGE_CACHE_MIB` menerima 128–512 |
| CPU thumbnail cache | Default 16 MiB; env `PDF_STUDIO_THUMBNAIL_CACHE_MIB` menerima 16–64 |
| Text data cache | 16 MiB, terpisah dari bitmap |
| Karakter per halaman | 200.000, dicek sebelum alokasi character-index vector |
| Copy sekali aksi | Maksimal 1 MiB UTF-8; missing cached pages tidak diam-diam dilewati |

Cache budgets bukan total process-memory limit. PDFium decoding, transient buffers,
layout, GPU dan cache overhead tetap menggunakan memori tambahan.

Pemilik menanyakan ribuan halaman. Jawaban terakhir: fondasinya mendukung pertumbuhan,
tetapi **belum ada bukti untuk 2000–10000 halaman** dan runtime sekarang menolaknya.
Jangan sekadar menaikkan cap atau mengklaim sudah siap. Opening masih menginspeksi
semua metadata; `content_width()` memindai geometri dan zoom mereset layout list.
Benchmark 2000/5000/10000, caching agregat layout, dan native-call isolation adalah
calon penguatan setelah ada instruksi, bukan pekerjaan yang sudah selesai/otomatis disetujui.

## 6. Toolchain dan cara menjalankan

Versi dipin: Rust **1.98.1**, GPUI Kit **0.9.1** / GPUI-pre **0.3.12**,
pdfium-render **0.9.5** (`pdfium_8125`, thread_safe), PDFium Chromium **8125**,
QPDF **12.4.2** MSVC64. Lockfile Cargo dan hash sidecars asli sudah ada.

```powershell
Set-Location D:\projects\bpdf
. ./scripts/use-build-environment.ps1
./scripts/check.ps1
./scripts/smoke-test.ps1
```

- `check.ps1`: fmt check, denied-warning workspace/all-target Clippy, workspace tests,
  lalu workspace release build. Native smoke adalah perintah terpisah.
- Aktivasi di atas mengatur local Rust/MSVC, `TEMP`/`TMP`, target dan build jobs di D:.
- Rust: `.tools/cargo`, `.tools/rustup`; MSVC: `.tools/vs2022`; shared/cache juga D:.
- Toolchain yang dipasang dengan izin pemilik: Microsoft-signed VS 2022 Build Tools
  17.14.44 dan Windows SDK 10.0.26100; tidak perlu dipasang ulang untuk melanjutkan.
- SDK payload `.tools/windows-sdk`, melalui junction dari registered Windows Kits
  path di C:. Sebagian mandatory servicing/runtime Microsoft tetap di C:.
- **Jangan hapus/pindah `.tools` atau seluruh workspace sembarangan**: VS/SDK terdaftar
  secara sistem dan junction C: bergantung lokasi D: ini.
- Tidak perlu fetch/download dependencies ulang jika yang lokal masih tersedia.

Jalankan tanpa build: double-click `run.cmd`. Script memilih
`target/release/pdf-studio.exe`, fallback `target/debug/pdf-studio.exe`, mengatur cwd
proyek dan meneruskan argument opsional. Kedua profil sudah dibangun pada akhir sesi
implementasi, timestamp yang dibaca saat handoff: **10 November 2026**.

Untuk build/stage debug plus membuka PDF:

```powershell
. ./scripts/use-build-environment.ps1
./scripts/run-desktop.ps1 -Pdf ./tests/fixtures/text-selection.pdf
```

Sidecars sumber ada di `sidecars/`; runtime memuat dari `sidecars` di samping executable.
Salinan sudah distage di `target/debug/sidecars` dan `target/release/sidecars`.
Build release biasa tidak otomatis menyalin sidecars pada checkout baru.
Windows mengunci executable yang berjalan: jika build tertahan Access denied, periksa
proses/path, tutup jendela proyek yang sesuai secara normal, lalu rebuild. Jangan kill
aplikasi lain atau menganggap PID lama di log masih valid.

## 7. Bukti pengujian yang benar-benar sudah dijalankan

Tidak ada suite yang dijalankan ulang pada turn handoff dokumentasi ini. Berikut
adalah hasil sesi implementasi sebelumnya, ditambah pemeriksaan file saat handoff.

- **16 workspace unit tests** lulus; fmt/Clippy all targets `-D warnings` lulus.
  Desktop debug/release terbaru berhasil dibangun. Lihat log historis, bukan hanya klaim.
- Real native QPDF/PDFium smoke lulus: urutan/rotasi output, Unicode/spaces/& paths,
  identitas raster, sumber tidak berubah, inspect/render/resource/source-change checks.
- Native text smoke lulus: isi/newline, semua intrinsic rotations 0/90/180/270,
  kotak glyph overlap actual raster ink, scan kosong, sumber berubah ditolak.
- Headed release selection UI terbaru lulus: single-click/jitter kosong, double-click
  sentence/label, hover saat pressed drag, forward/reverse, 4 rotations, zoom reuse,
  cross-page copy, clear, cache bound, source hashes. Clipboard disimpan/dipulihkan.
- Headed debug empty startup dan viewer regression juga pernah lulus setelah overlay
  ditambahkan; bukti itu mendahului koreksi click behavior terakhir.
- CI workflow Windows ditulis (quality + native + generated book sessions), tetapi
  **belum pernah dijalankan remotely dalam sesi ini**. Headed GPU tests adalah lokal.

### Books benchmark historis

- Delapan kasus vector/image-only sintetis, 100/300/500/1000 halaman, total 576 render:
  lulus, 72 render hanya 2 native opens, idle/recycle/source checks lulus.
- Scan sintetis 1000 halaman ~48,52 MiB: inspect 205 ms, first raster 48,46 ms,
  median 51,29 ms, p95 54,20 ms pada salah satu release run. Ini native-only timing,
  bukan latency layar, bukan buku scan asli, dan bukan jaminan performa umum.
- Headed scan 1000 halaman: dua pass × 100 destinasi tersebar, **bukan** membaca semua
  1000 halaman per pass. Working set akhir 353,5 MiB, peak 521,5 MiB; cache bounded.
- Benchmark process peak 159,8 MiB termasuk generator, tanpa GUI/GPU.
- Angka ini mendahului fitur text selection; cache teks baru menambah budget terpisah.
  Shared GPU dapat overlap working set: jangan menjumlahkan keduanya sebagai total RAM.
- Mesin pengujian: i7-13620H, ~15,7 GiB RAM, Windows build 10.0.26300.

### Bukti lokal (ignored, mungkin tidak ada pada checkout baru)

| Bukti | Lokasi `.research/` |
|---|---|
| Click behavior UI paling baru | `text-ui-9beba3501152498bad944cc76a8d252f` |
| Seleksi awal lengkap termasuk cross-page | `text-ui-4225477efef6436e95f5ae2a5c267bbd` |
| Debug viewer regression setelah overlay | `m1-ui-13f9daebe00440249c16a5de48130d1f` |
| Release book 1000-page UI stress | `m1-ui-24335a04409b41e89ec507eb6746c8cb` |
| Book CSV/native memory | `book-benchmark/results.csv`, `book-benchmark/process.json` |
| Text fixture numeric probes | `text-selection/probe.json` |

Semua capture bukti berisi fixture sintetis. Catatan lama “viewer left open” adalah
keadaan saat tes, bukan jaminan proses masih hidup di chat berikutnya.

### Cara mengulang tes khusus jika diperlukan

```powershell
. ./scripts/use-build-environment.ps1
cargo run --locked -p pdf-smoke -- --write-fixtures
cargo run --locked -p pdf-smoke
./scripts/text-selection-smoke.ps1 -Profile release
./scripts/m1-ui-smoke.ps1 -Profile debug
./scripts/benchmark-books.ps1
./scripts/m1-ui-smoke.ps1 -Profile release -Fixture .research/book-benchmark/simulated-scan-1000.pdf -ExpectedPages 1000 -Stress -StressSamples 100
```

Jangan menjalankan dua headed test secara bersamaan: discovery PID/jendela dapat
bertabrakan. UI tests memakai salinan fixture masing-masing, UIAutomation dan targeted
HWND input, bukan global SendKeys. `text-selection-smoke` memerlukan native probe hasil
`pdf-smoke`; backup clipboard jangan dihilangkan. Screenshot hanya jendela tes sendiri.

## 8. Bug/keputusan yang jangan diulang

- CPU LRU saja tidak melepas texture di window atlas: sempat menyebabkan pertumbuhan
  memori GPU. Pakai residency tracking + deferred `drop_image` dengan current Window.
- Debug no-argument startup pernah stack overflow, terlewat tes release-only. Reserve
  `/STACK:8388608` dan empty-start check perlu dipertahankan.
- Absolute canvas tanpa `.top_0().left_0()` pernah berada di bawah gambar sehingga drag
  tidak mengenai teks. Overlay seleksi dan Ctrl+scroll probe sekarang diikat top-left.
- Hover/repaint tanpa pressed button pernah memutus drag; release bukan ditentukan
  dari hover. Ada global mouse-up capture pada viewport dan window activation observer.
- QPDF memerlukan path Windows normal: `dunce::canonicalize` menghindari `\\?\` prefix
  yang sebelumnya tidak diterima probe. Argumen harus token terpisah, bukan shell string.
- GPUI Kit membutuhkan **AllAssets**; reduced default asset bundle tidak memiliki
  seluruh icon yang dipakai. Jangan kembali ke bundle yang membuat ikon hilang.
- PDFium bindings process-global; rebinding DLL per request merusak kontrak wrapper.
- Windows foreground activation tidak andal untuk automation; jangan mengirim global
  keyboard input ke aplikasi yang mungkin bukan target. Gunakan UIA/HWND sendiri.
- Generator dan UI pernah memakai file yang sama lalu memicu SourceChanged. Salin
  fixture dan jangan menulis ulang sumber yang sedang diuji.
- Windows PowerShell dapat salah membaca UTF-8 tanpa BOM: untuk read/modify/write
  gunakan **`Get-Content -Encoding UTF8`**, jangan menyebabkan `Â·`/mojibake kembali.
- Angka 9/10/12/15 tests di bagian lama `docs/testing.md` adalah snapshot historis;
  hasil paling baru adalah **16**, bukan berarti seluruh log lama gagal atau belum jalan.

## 9. Catatan implementasi untuk M2 berikutnya

M2 adalah kandidat berikutnya ketika pemilik memintanya; turn handoff ini **bukan**
instruksi mulai implementasi M2 atau menambahkan scope baru secara otomatis.

Urutan yang sudah dijelaskan kepada pemilik:

1. Workspace multi-sumber + validated pure commands/inverse deltas.
2. Page selection (berbeda dari text selection): Ctrl-toggle, Shift-range, keyboard.
3. Rotate/delete dengan live preview, stable IDs dan selection semantics.
4. Reorder/drag thumbnails: satu drag = satu command, bukan per mouse movement.
5. Import PDF lain (virtual merge), tidak menulis ulang file sumber.
6. Undo/redo bounded, dirty revision, empty state dan UI/native tests.

Perubahan model yang perlu direncanakan:

- Workspace sekarang menyimpan **satu** `SourceDocument`; controller dan layout banyak
  memanggil `.source()`. Multi-source bukan sekadar menambah tombol Import.
- `PageEntry.source_page_index` menunjuk halaman asal; PageId harus tetap stabil saat
  reorder/rotate. Jangan samakan posisi UI dengan identitas.
- Cache teks sekarang berkunci posisi workspace (`usize`) dan scope sumber tunggal.
  Setelah reorder/import/delete, jangan gunakan cache lama untuk posisi yang berbeda.
  Gunakan source/fingerprint/page/transform key atau invalidasi yang benar.
- TextSelection memakai posisi halaman. Saat organizer edit, remap ke stable identity
  atau clear secara eksplisit; jangan copy/highlight halaman sumber yang salah.
- Cache raster sudah source-aware, tetapi controller masih memilih path dari sumber
  tunggal. Source resolution dan virtual rotation harus masuk render plan.
- Intrinsic rotation dan virtual rotation berbeda: uji komposisinya. Overlay teks juga
  harus mengikuti virtual rotation; jangan mengandalkan native intrinsic mapping saja.
- Satu native cached source dapat sering dibuka ulang ketika page plan bergantian
  sumber. Pertimbangkan pool kecil berbatas setelah profiling, bukan pool tak terbatas.
- Constructor domain saat ini menolak 0 halaman. M2 perlu empty workspace setelah Delete
  semua; M3 nanti tetap wajib menolak export kosong.
- `is_dirty()` sekarang false. Gunakan edit revision/clean baseline; zoom/scroll/text/page
  selection saja tidak dirty. Undo kembali baseline harus konsisten. New edit setelah
  Undo membersihkan redo stack. History harus bounded/compact, bukan clone buku penuh.
- Import perlu aturan agregat resource limits; cap per-file saja tidak membatasi total
  workspace banyak sumber. Jangan langsung mengaktifkan duplicate same-page semantics
  tanpa ID/history/export tests, sesuai kehati-hatian blueprint.
- Domain authoritative; jangan membuat dua writable page lists di domain dan UI.
- Tests: deterministic edit/undo sequences, stable IDs/order/rotation/selection,
  seeded operation sequences, dan UI smoke multi-source. Save As tetap M3.

## 10. Gate yang masih terbuka

- Physical precision-trackpad pinch/Ctrl+scroll, keyboard interaction matrix lengkap,
  mixed-monitor 100/150/200% DPI, broader accessibility.
- Buku scan asli kompleks/JPEG2000/color, unusual fonts/crop boxes, reading order,
  ligature/Unicode breadth; bukan hanya fixture sederhana.
- 2000–10000 page benchmarks, files >128 MiB, process isolation/hard cancellation.
- Forms/signatures/bookmarks/export preservation preflight, QPDF produksi, safe output
  staging, validation, runtime/license audit, portable clean-machine packaging.

## 11. Dokumen pendamping

- [README.md](README.md): setup/run/check commands.
- [docs/testing.md](docs/testing.md): log verifikasi bertahap, terbaru di atas.
- [docs/architecture.md](docs/architecture.md): desain saat ini.
- [docs/compatibility.md](docs/compatibility.md): fitur dan batas pengguna.
- [docs/books.md](docs/books.md): benchmark buku dan batas interpretasinya.
- [docs/text-selection.md](docs/text-selection.md): perilaku dan tests teks.
- [docs/decisions.md](docs/decisions.md), [docs/adr](docs/adr): keputusan dan tradeoff.
- [sidecars.lock.json](sidecars.lock.json), [Cargo.lock](Cargo.lock): pins/checksums.

## 12. Prompt siap pakai untuk chat baru

> Kita melanjutkan PDF Studio di D:\projects\bpdf. Baca PROJECT_HANDOFF.md terlebih
> dahulu, lalu PDF_EDITOR_CODEX_BLUEPRINT.md dan dokumen/kode yang relevan. M1 beserta
> penguatan buku, trackpad zoom, text selection/copy dan perbaikan single/double-click
> sudah diimplementasikan. M2 belum dimulai. run.cmd memakai release, fallback debug.
> Jangan instal ulang toolchain atau menghapus .tools; jika butuh aplikasi baru,
> tempatkan di D:. Gunakan hasil tes yang tercatat dengan jujur dan periksa keadaan
> file/proses sekarang, jangan menganggap PID atau “left open” lama masih berlaku.
> Setelah memahami konteks, lanjutkan permintaan saya berikut ini: [isi task berikutnya].
