# Prompt siap pakai untuk chat baru

Aturan aktif: baca `AGENTS.md` dan `docs/feature-development.md`. Research fitur wajib
berbasis kompetitor/sumber primer; setiap fitur selesai wajib Conventional Commit
dan push. Instruksi ini menggantikan status izin push historis pada checkpoint lama.

**Follow-up terbaru:** pencarian teks PDF aktif sudah diimplementasikan setelah
checkpoint PNG berikut. Baca bagian teratas handoff, `docs/search.md` dan bagian
search terbaru `docs/testing.md`: default tidak case-sensitive, pencocokan frasa
berurutan, bar dalam tab, 53 unit tests dan 14 kasus search UI lulus termasuk
1000 halaman/pause/resume. Fitur tercatat di commit `a74d380`; periksa Git untuk
checkpoint dokumentasi dan status upstream terbaru.
Keterangan PNG sebagai implementasi terakhir di prompt checkpoint berikut bersifat
historis dan digantikan follow-up ini.

Salin teks berikut dan ganti permintaan terakhir. Checkpoint **17 November 2026**:
PNG sudah tercatat di commit `2124580`; saat mulai handover, working tree bersih dan `main`
sama dengan cache `origin/main`. Sesi handover hanya mengubah dokumentasi, tanpa
fetch/commit/push atau pemeriksaan remote CI. Periksa Git lagi saat melanjutkan.

```text
Kita lanjut proyek PDF Studio di D:\projects\bpdf. Berkomunikasi bahasa Indonesia.
Baca AGENTS.md dan docs/feature-development.md terlebih dahulu. Research fitur harus
berbasis produk kompetitor (Acrobat/Reader, Firefox/PDF.js, Chrome PDF, Edge PDF, dll),
bukan menebak dari keterbatasan aplikasi kita. Sertakan sumber primer, bedakan fakta/
observasi/rekomendasi/unknown dan simpan riset substantif di docs/research/.
Setiap fitur implementasi yang selesai dan lolos tes wajib Conventional Commit lalu
push ke origin https://github.com/zufarhandito/origo.git, branch main. Ini izin
berkelanjutan terbaru; status penolakan/izin lama pada checkpoint historis sudah
digantikan. Tidak mengizinkan implementasi plan-only atau publikasi binary/data privat.
Baca berurutan:
1. README.md dan docs/README.md
2. Bagian terkini docs/handoff/PROJECT_HANDOFF.md (17 November 2026)
3. docs/png.md dan bagian PDF-to-PNG terbaru docs/testing.md
4. docs/multitab.md, docs/multitab-hardening.md dan docs/architecture.md
5. docs/device-acceptance.md, docs/distribution-audit.md, docs/release-checklist.md
Lalu baca kode/dokumen fitur terkait. Blueprint dan snapshot lama bersifat historis;
rancangan UI di docs/plans/UI_REDESIGN_MULTITAB_PLAN.md sudah diimplementasikan.

Produk nyata: viewer, organizer, multi-tab desktop, safe Save As/Extract, unsaved
guard, Split, Optimize lossless dan PDF → PNG. Nama PDF/close menyatu dalam tab,
Daftar tab satu control berlabel/caret; state/callback terpisah per dokumen.
Renderer, slot ekspor dan budget cache bersama. Maksimal 8 tab termasuk loading.

Update setelah checkpoint PNG: fitur Find sudah di commit a74d380
(feat(search): add phrase search to active PDF tabs). Baca docs/search.md,
docs/research/text-search.md dan bagian terbaru docs/testing.md. Ctrl+F membuka bar
dalam tab aktif; default tidak case-sensitive dan frasa berurutan, dengan sorotan,
penghitung, navigasi, case/whole-word options dan state per tab. 53 unit tests,
full native smoke, 14 kasus search UI termasuk 1000 halaman/pause/resume dan chrome
regression lulus. Checkpoint Git/izin push lama di bawah bersifat historis.

PNG: Alat → Ekspor PNG…, semua/seleksi, 150 DPI default atau 300 DPI, folder baru
no-overwrite; page-0001.png dst mengikuti snapshot urutan/impor/rotasi workspace.
Latar putih opaque, dimensi exact terpisah dari viewer zoom/DPR; batas 8192px/sisi,
16 × 1024 × 1024 pixel/halaman dan 1 GiB encoded/batch. Encode/decode divalidasi per
halaman. DPI belum ditulis sebagai metadata physical-print pHYs. Input kompleks
masih diblokir preflight konservatif; lihat docs/png.md untuk detail/batas.
Job tetap milik tab asal, dapat dibatalkan dari tab lain, close menunggu job.
Cancel cooperative, native render tidak preemptible; crash-orphan recovery belum ada.

Sumber read-only, policy v4, private snapshots, validation/source recheck dan
staging/no-overwrite harus dipertahankan. Hanya full PDF Save As dengan receipt
instance/revisi sesuai menandai clean; Extract/Split/Optimize/PNG tidak.
PNG sukses tidak mengizinkan dirty-close otomatis. Optimize lossless bisa membesar,
tanpa rasterisasi atau re-encoding JPEG.

Git saat handover dimulai: main/HEAD 2124580 (export png), cache origin/main sama,
working tree bersih. Perubahan handover sesudahnya belum di-commit oleh sesi itu.
Sebelumnya 2c3033c (portable/audit), 06c2a39 (M3/device gates), 582cd67 (tab).
Recheck git status/log/upstream dan pertahankan perubahan pengguna. Tidak ada fetch
atau verifikasi remote CI/server baru pada sesi handover. Status push ditolak dan
HEAD lama di snapshot historis tidak menggantikan checkpoint ini.

Bukti pekerjaan PNG di docs/testing.md: check.ps1 lulus fmt/Clippy, 49 unit tests
dan release build; full native smoke dan final release --png-smoke lulus.
PNG UI 11 kasus bernama pada build lokal dan 11 pada extracted portable lulus.
Tabs, 25 kasus M3, Split dan Optimize UI lulus. Jangan klaim suite produk dijalankan
ulang saat handover dokumentasi. Profil delapan buku/400% ada di multitab-hardening.
Snapshot M3 asli tetap byte-identical di scripts/archive/m3-single-workspace-ui.ps1.txt.
Kandidat dist/pdf-studio-windows-x64-candidate-2c3033c-0442227c.zip lulus manifest
1103 file/checksum/import/runtime, lima negative checks, serta PNG UI restricted
PATH dari path Unicode. Ini dirty source sebelum commit PNG, bukan build bersih
HEAD 2124580. ZIP ignored/internal dan belum dipublikasikan.

Gate terbuka: physical 150/200%, mixed-monitor, physical modifier shortcuts,
trackpad/Narrator, clean Windows 10/11, lisensi/redistribution/MSVC entitlement,
tiga notice gaps dan native/security review. Bukti otomatis satu display 100%/DPI96
tidak menutup tes fisik; P4 belum penuh. anyhow sudah 1.0.103; empat unmaintained
advisories tetap tercatat. Jangan menyatakan public-release ready.

JPEG/TXT sengaja di-skip; AI/OCR/converter lain belum diimplementasikan.
Konteks AI ditunda: AI online berbayar ke pemilik, model usage termasuk paket,
mahasiswa/peneliti Indonesia. Reader gratis + trial sekali per akun; beta Rp49.000/
30 hari prepaid/manual renewal, tanpa autodebit/top-up. Target biaya AI maksimum
Rp10.000/pelanggan/30 hari; kredit transparan dan estimasi sebelum tugas berat;
pilot 10–20 undangan. Pendamping baca + riset multi-PDF dengan sumber/halaman.
PDF lokal, kirim konteks perlu dengan persetujuan saat AI dipakai. Provider/model
belum dipilih; ACP/Zed hanya inspirasi. Belum ada backend/auth/payments AI.
File docs/plans/AI_BUSINESS_MODEL.md belum dibuat. Jika diminta melanjutkan AI,
tulis/validasi bisnis dahulu, jangan langsung implementasi. Detail keputusan vs
usulan kredit/trial/limit PDF/unit economics ada di handover; usulan belum terbukti.

Run: run.cmd (release, debug fallback, tanpa build). Build/check dari root PowerShell:
. ./scripts/use-build-environment.ps1
./scripts/check.ps1
./scripts/smoke-test.ps1
cargo run --release --locked --offline -p pdf-smoke -- --png-smoke
./scripts/png-ui-smoke.ps1 -Profile release
Rust 1.98.1; GPUI Kit 0.9.1; pdfium-render 0.9.5/PDFium 8125; QPDF 12.4.2.
Toolchain sudah di D:. Jangan reinstall rutin, menghapus/memindahkan .tools atau
merusak VS/SDK junction. Gunakan UTF-8 eksplisit. UI suites bergantian, hanya
HWND/PID fixture sendiri; jangan menutup dokumen/proses pengguna. Gunakan PDF sintetis.
.research/PROJECT_HANDOFF_PRIVATE.md dan PDF pengguna tetap privat/lokal/ignored;
jangan dibaca/disalin ke docs, commit atau upload. Handover tidak memerintahkan
publikasi binary atau implementasi fitur baru. Pilih tes sesuai perubahan;
docs-only cukup link/diff/fact checks. Lanjut sesuai permintaan pemilik berikutnya.

Permintaan saya berikutnya: [tulis pekerjaan yang ingin dilanjutkan di sini].
```
