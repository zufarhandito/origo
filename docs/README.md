# Dokumentasi PDF Studio

Ini indeks dokumentasi untuk pengembang dan AI. Bahasa komunikasi dengan pemilik:
Indonesia. Jalankan command dari root repository `D:\projects\bpdf`.

Aturan aktif pemilik: [panduan riset dan penyelesaian fitur](feature-development.md)
dan [AGENTS.md](../AGENTS.md). Research fitur memakai produk kompetitor dan sumber
primer, lalu dipetakan ke aplikasi ini. Setiap fitur selesai wajib commit Conventional
Commits dan push ke tujuan repo yang disepakati.

## Mulai di sini untuk chat baru

1. [Handoff terkini](handoff/PROJECT_HANDOFF.md): fitur nyata, hasil tes terakhir,
   batas, status Git/push, dan instruksi terakhir pengguna.
   [Progres proyek](PROGRESS.md): tahap selesai, commit, backlog dan gate yang terbuka.
2. [Perilaku desktop + multi-tab](multitab.md) dan
   [rancangan yang diimplementasikan](plans/UI_REDESIGN_MULTITAB_PLAN.md).
3. [Arsitektur saat ini](architecture.md), [kompatibilitas](compatibility.md), dan
   [bukti pengujian](testing.md): cocokkan dengan kode sebelum mengubahnya.
4. [Blueprint awal](blueprints/PDF_EDITOR_CODEX_BLUEPRINT.md) dan [ADR](adr/):
   kontrak engineering/keamanan dan keputusan teknis, dibaca sesuai kebutuhan.

[Prompt siap salin ke chat berikutnya](handoff/NEXT_CHAT_PROMPT.md).

Pematangan multi-tab **1–4B sudah selesai**, termasuk fix background tab aktif.
[Roadmap/riset](research/multitab-roadmap.md) menyimpan keputusan dan daftar acceptance;
[progres](PROGRESS.md) memetakan hasil ke commit. Group tab/tab vertikal tetap backlog;
anotator inti kini tersedia melalui [Anotasi](annotations.md). [Tanda tangan visual](signatures.md) tersedia.
Jangan menganggap daftar backlog sebagai izin eksekusi.

Implementasi bertahap **17 November 2026** mencakup commit `615e57e` (tahap 1),
`7c9db9a` (2), `bf51fb6` (3), `73e2409` (4A), `a1647e0` (4B), dan `70287a7`
(background tab). Awal handover: `main` bersih, HEAD `70287a7`, cache `origin/main`
`a1647e0`, selisih 1 ahead/0 behind sebelum commit dokumen ini. Cache/reflog kini
mencakup 4B, tetapi bukan verifikasi ulang server/CI. Push agen sebelumnya ditolak
automatic approval review untuk tujuan/payload spesifik; jangan menyamakan perubahan
cache dengan jawaban persetujuan. Tidak ada fetch/push oleh agen pada handover ini.
Rincian dan batas kelanjutan ada di [handoff terbaru](handoff/PROJECT_HANDOFF.md).

[Pencarian PDF aktif](search.md) sudah diimplementasikan, default tidak case-sensitive
dan frasa berurutan. Check multi-tab terakhir lulus 57 unit tests; search UI lulus
14 kasus. Fix background berikutnya lulus cek visual dua tema/fmt/Clippy/build.
Bukti dan batas ada di [testing](testing.md); suite produk tidak diulang pada handover ini.

Handoff terkini mengalahkan status milestone historis di arsip/blueprint. Rencana
tidak berarti sudah diimplementasikan atau otomatis diizinkan untuk dieksekusi.
Instruksi pengguna pada sesi aktif tetap menentukan pekerjaan yang dilakukan.

## Peta dokumentasi

Riset terbaru: [anotasi PDF dan tanda tangan draw/import](research/pdf-annotations-and-signatures.md)
(17 November 2026). Berisi perbandingan kompetitor, usulan UX dan gap ekspor;
anotator inti dan tanda tangan visual sudah diotorisasi dan diimplementasikan.

| Kelompok | Dokumen |
|---|---|
| Konteks aktif | [Handoff](handoff/PROJECT_HANDOFF.md), [prompt chat baru](handoff/NEXT_CHAT_PROMPT.md) |
| Progres proyek | [Tahap selesai, commit dan pekerjaan terbuka](PROGRESS.md) |
| Alur pengembangan | [Riset kompetitor, commit dan push](feature-development.md), [contoh riset Find](research/text-search.md) |
| Rencana UI | [Redesain desktop dan multi-tab](plans/UI_REDESIGN_MULTITAB_PLAN.md) |
| Multi-tab aktif | [Perilaku tab](multitab.md), [hardening dan hasil profil](multitab-hardening.md) |
| Keputusan multi-tab | [Riset kompetitor, acceptance selesai dan backlog](research/multitab-roadmap.md) |
| Arsitektur/keputusan | [Arsitektur](architecture.md), [keputusan](decisions.md), [ADR](adr/) |
| Tanda tangan visual | [Draw/import PNG/JPEG, penempatan dan edit ulang](signatures.md) |
| Anotator aktif | [Highlight, pen/stabilo dan edit ulang](annotations.md) |
| Viewer | [Buku dan benchmark](books.md), [seleksi teks](text-selection.md) |
| Pencarian PDF aktif | [Cari kata/frasa](search.md), default tidak case-sensitive |
| Pengaturan halaman | [Organizer](organizer.md), [perubahan belum disimpan](unsaved-changes.md) |
| Output PDF | [Save As/Extract](m3-safe-export-plan.md), [Split](split.md), [Optimize](optimize.md) |
| Dukungan/pengujian | [Kompatibilitas](compatibility.md), [hasil pengujian](testing.md) |
| Acceptance aktif | [Pemetaan migrasi M3](m3-ui-coverage.md), [perangkat/input/aksesibilitas](device-acceptance.md) |
| Checkpoint kerja | [Recheck prioritas 3–5](checkpoint-priorities-3-5.md) |
| Distribusi Windows | [Audit dan kandidat portable](distribution-audit.md), [gate rilis](release-checklist.md) |
| Arah konversi | [Roadmap konversi](conversion-roadmap.md), hanya PNG yang aktif |
| Ekspor gambar aktif | [PDF → PNG](png.md), semua/seleksi dan 150/300 DPI |
| Kontrak awal | [Blueprint Level 1](blueprints/PDF_EDITOR_CODEX_BLUEPRINT.md), memuat status historis |
| Sejarah sesi | [Arsip multi-tab 17 November](archive/PROJECT_HANDOFF_2026-11-17_MULTITAB.md), [arsip sebelumnya](archive/PROJECT_HANDOFF_HISTORY.md); bukan titik mulai kerja |

Dokumen multi-tab menjelaskan perilaku shell baru. Suite M3 sudah dimigrasikan;
[coverage map](m3-ui-coverage.md) menjelaskan padanan kasus dan snapshot historisnya.
Catatan pengujian/blueprint lama tetap historis. Gate fisik yang belum dijalankan
tetap terlihat pada [matriks perangkat](device-acceptance.md).

## Konvensi path dan privasi

- Tautan Markdown relatif terhadap file yang memuat tautan tersebut.
- Path kode, command, `.tools`, `.research` dan `target` dalam teks/code block
  relatif terhadap root repository, kecuali disebut lain.
- [README root](../README.md) tetap menjadi pintu masuk setup/run bagi GitHub dan AI.
  Dokumentasi pendamping root lama telah dipindah ke folder di atas.
- [LICENSES](../LICENSES/README.md) tetap di dekat notice lisensi; tidak dipindah.
- Catatan `.research/PROJECT_HANDOFF_PRIVATE.md` dan PDF pengguna tetap lokal,
  gitignored, tidak dipindah ke docs, tidak di-commit atau diunggah.
