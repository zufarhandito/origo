# Prompt siap pakai untuk chat baru

Checkpoint **17 November 2026**, ditulis di atas `6bbc6c1`. Baca
[handoff terkini](PROJECT_HANDOFF.md) untuk detail dan [roadmap multi-tab](../research/multitab-roadmap.md)
untuk satu dokumen riset/TODO. Copy teks berikut, lalu ganti permintaan terakhir.
Prompt ini menggantikan prompt checkpoint PNG lama; riwayat tetap di handoff/Git.

```text
Kita lanjut PDF Studio di D:\projects\bpdf, Windows x64/PowerShell. Bahasa Indonesia.
Baca berurutan:
1. AGENTS.md, docs/README.md dan docs/feature-development.md.
2. Checkpoint paling atas docs/handoff/PROJECT_HANDOFF.md, 17 November 2026.
3. docs/research/multitab-roadmap.md: satu dokumen TODO/riset multi-tab berikutnya.
4. docs/search.md, docs/research/text-search.md dan bagian terbaru docs/testing.md.
5. docs/multitab.md, docs/multitab-hardening.md, docs/architecture.md.
Lalu dokumen/kode terkait tahap yang diminta. Blueprint/snapshot lama bukan status terkini.

Status Git saat handover dimulai: main/HEAD 6bbc6c1, working tree bersih,
cache origin/main di 2124580; ahead 3/behind 0. Commit belum terkirim:
- a74d380 feat(search): add phrase search to active PDF tabs.
- 9478ddc docs(workflow): require competitor research and feature checkpoints.
- 6bbc6c1 docs(multitab): document researched roadmap and backlog.
Commit handover menambah checkpoint di atasnya; periksa git status/log/upstream.
Tidak ada fetch atau verifikasi remote CI pada handover. Cache bukan bukti server.

Push ke origin https://github.com/zufarhandito/origo.git branch main sebelumnya
DITOLAK automatic approval review: pengiriman source/dokumentasi ke tujuan spesifik
itu belum dianggap mendapat persetujuan eksplisit destination/payload. Konfirmasi
sudah ditanyakan untuk ketiga commit; pemilik beralih meminta handover, belum menjawab
secara eksplisit. Jangan retry/bypass atau mengklaim sudah push. Instruksi umum
pemilik untuk Conventional Commit lalu push tiap fitur tetap berlaku, tetapi tidak
menghapus penolakan ini. Pertahankan commit lokal dan jelaskan blocker bila relevan.
Tidak ada binary/ZIP yang dipublikasikan; handover bukan instruksi publikasi.

Produk nyata: viewer, seleksi/copy teks, organizer, desktop multi-tab maksimal 8
termasuk loading, safe Save As/Extract, unsaved guard, Split PDF, Optimize lossless,
PNG dan Find. Source read-only; state/history/viewport terpisah per PDF. Tab
tersembunyi berhenti menjadwalkan render dan melepas cache raster/teks/GPU, tetapi
workspace/seleksi/posisi baca bertahan di RAM. Belum ada restore sesi/crash recovery.

Find sudah di commit a74d380: Ctrl+F/tombol Cari teks/menu Edit, bar di dalam tab
bawah toolbar, default TIDAK case-sensitive. 'hasil penelitian' dicari sebagai
frasa berurutan, bukan kata terpisah. Normalisasi whitespace/linebreak hanya dalam
satu halaman. Counter/highlight, next/previous/Enter/F3/arah balik, wrap, Esc,
opsi case/whole-word, state per tab. Debounce 200 ms, scan satu halaman melalui
worker bersama, query <=1024 karakter, <=10000 hasil. Tidak ada OCR/upload/regex.
Tab nonaktif pause scan belum selesai, kembali mengulangnya; hasil lama ditolak token.

Bukti terakhir di docs/testing.md: fmt/denied-warning Clippy, 53 unit tests dan
release workspace build lulus; search/full native smoke termasuk M1/text/M3/
whole-source/tagged Extract/Split/Optimize/PNG lulus. Final search UI 14 kasus:
.research/search-ui-4e4935cb28a548e88e110e53a2d9ea66, termasuk 1000 halaman,
progress/pause/resume/cache, frasa/case, isolasi tab, organizer dan source hash.
Chrome UI dua tema/empat lebar lulus di chrome-ui-8b6d7ecc2a954fd581726d239ebb9a32.
Handover hanya memeriksa keberadaan manifest dan 14 passed unik, executable,
Git/dokumen/tautan; TIDAK mengulang test/build produk. Bukti historis bukan remote CI.

FOKUS BERIKUTNYA: pemilik ingin mematangkan multi-tab sebelum prioritas lain.
Roadmap 389 baris pada 6bbc6c1 memuat 9 riset fitur dan 2 backlog; semua TODO belum
implementasi. Urutan disetujui:
1. Drag urutan tab; bintang kuning; + langsung di kanan tab terakhir, kelompok rata
   kiri dan Daftar tab di ujung; menu klik kanan tab.
2. Buka ulang tab tertutup dan pencarian dalam Daftar tab.
3. Pemulihan sesi/posisi baca.
4A. Dua PDF berdampingan; 4B. dua tampilan PDF yang sama.
Pemilik mewajibkan Dock GPUI Kit untuk split, dengan pembatas bisa digeser/resize.
API lokal 0.9.1 sudah dibaca: DockArea, DockLayout::h_split(), DockSkin dan resize
handle. Tes/source upstream yang dibaca tidak diklaim diuji sebagai integrasi app.

Organizer saat split sudah disetujui: satu panel alat mengikuti PDF fokus dan
menyebut nama/jumlah seleksi; seleksi A/B terpisah dan bertahan saat pindah fokus.
Rotasi/hapus/pindah/ekstrak/impor/Undo/Redo/Save As ke sasaran yang benar; Ctrl+A/Delete
pada input tetap mengedit teks. Job ekspor tetap milik dokumen asal setelah switch.
Dua panel terlihat boleh render; fokus hanya satu penerima aksi. Jangan memakai
'active=false' lama untuk mematikan semua kerja panel split yang masih terlihat.
Resize memakai viewport panel; Dock close tidak boleh melewati unsaved guard.
Dua view PDF yang sama: edit dokumen dibagi, posisi baca/zoom terpisah; perlu
pemisahan state dokumen dan state tampilan, bukan salinan workspace yang berkonflik.

Group tab dan tab vertikal BACKLOG tanpa jadwal. Pin/hover preview/MRU/multi-window
menunggu pengalaman penggunaan, bukan bagian otomatis tahap 1-4. Belum dikunci:
persistensi bintang (permanen baru rekomendasi), retensi/cakupan reopen, default/
cakupan restore dirty atau multi-source, detail split/close view, dan kepemilikan
seleksi/pencarian untuk dua view satu PDF. Kunci detail relevan sebelum implementasi.
Permintaan 'Implement the approved plan' terakhir mengimplementasikan PENULISAN
DOKUMEN roadmap, bukan memulai semua fitur. Permintaan terakhir sekarang handover.
Ikuti task berikutnya dari pemilik; jangan otomatis menjalankan seluruh roadmap.

Riset fitur harus berdasarkan kompetitor/sumber primer (Acrobat/Reader, Firefox/
PDF.js, Chrome PDF, Edge PDF), bukan menebak dari kondisi app kita. Bedakan fakta,
uji langsung, rekomendasi dan unknown; simpan bukti/keputusan. Commit fitur selesai
memakai Conventional Commits. Dokumentasi multi-tab riset/TODO tetap satu dokumen.

Kontrak ekspor: policy v4, private snapshots, preflight/consent, validasi/source
recheck, staging/no-overwrite. Hanya full Save As receipt instance/revisi cocok
menandai clean; Extract/Split/Optimize/PNG tidak. Optimize lossless bisa membesar.
Native PDFium masih in-process/cooperative, tanpa hard timeout/sandbox. Fingerprint
size/mtime bukan hash. PNG 150/300 DPI, opaque putih, exact dimensions, 8192px/sisi,
16 Mi-pixel/halaman dan 1 GiB/batch; belum ada pHYs/crash-orphan recovery.

Run: run.cmd (release, fallback debug, tanpa build). Toolchain sudah di D:.
. ./scripts/use-build-environment.ps1
./scripts/check.ps1
./scripts/smoke-test.ps1
cargo run --release --locked --offline -p pdf-smoke -- --search-smoke
cargo run --release --locked --offline -p pdf-smoke -- --write-search-fixtures
./scripts/search-ui-smoke.ps1 -Profile release
Pilih check sesuai perubahan; docs-only cukup fact/link/diff checks. Rust 1.98.1,
GPUI Kit 0.9.1, pdfium-render 0.9.5/PDFium 8125, QPDF 12.4.2. Jangan reinstall rutin,
hapus/pindah .tools atau merusak VS/SDK junction. Gunakan UTF-8 eksplisit.
Headed suites bergantian, hanya HWND/PID fixture sintetis sendiri. Jangan tutup/
kill dokumen/proses pengguna. .research/PROJECT_HANDOFF_PRIVATE.md dan PDF pengguna
privat/lokal/ignored, jangan dibaca/disalin/commit/upload. Pertahankan perubahan user.

Gate rilis tetap terbuka: physical DPI150/200/mixed monitor/trackpad/modifier/Narrator,
clean Windows10/11, lisensi/redistribusi/MSVC entitlement/notice gaps dan security.
Lihat docs/device-acceptance.md, docs/distribution-audit.md, docs/release-checklist.md.
Candidate ZIP lama bukan build bersih fitur Find dan bukan public release.
JPEG/TXT sengaja ditunda; OCR/Office converter/AI belum ada. Diskusi bisnis AI
sebelumnya tetap di handoff, bukan task sekarang; AI_BUSINESS_MODEL.md belum dibuat.

Permintaan saya berikutnya: [tulis pekerjaan yang ingin dilanjutkan di sini].
```
