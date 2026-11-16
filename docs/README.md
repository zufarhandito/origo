# Dokumentasi PDF Studio

Ini indeks dokumentasi untuk pengembang dan AI. Bahasa komunikasi dengan pemilik:
Indonesia. Jalankan command dari root repository `D:\projects\bpdf`.

## Mulai di sini untuk chat baru

1. [Handoff terkini](handoff/PROJECT_HANDOFF.md): fitur nyata, hasil tes terakhir,
   batas, status Git/push, dan instruksi terakhir pengguna.
2. [Perilaku desktop + multi-tab](multitab.md) dan
   [rancangan yang diimplementasikan](plans/UI_REDESIGN_MULTITAB_PLAN.md).
3. [Arsitektur saat ini](architecture.md), [kompatibilitas](compatibility.md), dan
   [bukti pengujian](testing.md): cocokkan dengan kode sebelum mengubahnya.
4. [Blueprint awal](blueprints/PDF_EDITOR_CODEX_BLUEPRINT.md) dan [ADR](adr/):
   kontrak engineering/keamanan dan keputusan teknis, dibaca sesuai kebutuhan.

[Prompt siap salin ke chat berikutnya](handoff/NEXT_CHAT_PROMPT.md).

Handoff terkini mengalahkan status milestone historis di arsip/blueprint. Rencana
tidak berarti sudah diimplementasikan atau otomatis diizinkan untuk dieksekusi.
Instruksi pengguna pada sesi aktif tetap menentukan pekerjaan yang dilakukan.

## Peta dokumentasi

| Kelompok | Dokumen |
|---|---|
| Konteks aktif | [Handoff](handoff/PROJECT_HANDOFF.md), [prompt chat baru](handoff/NEXT_CHAT_PROMPT.md) |
| Rencana UI | [Redesain desktop dan multi-tab](plans/UI_REDESIGN_MULTITAB_PLAN.md) |
| Arsitektur/keputusan | [Arsitektur](architecture.md), [keputusan](decisions.md), [ADR](adr/) |
| Viewer | [Buku dan benchmark](books.md), [seleksi teks](text-selection.md) |
| Pengaturan halaman | [Organizer](organizer.md), [perubahan belum disimpan](unsaved-changes.md) |
| Output PDF | [Save As/Extract](m3-safe-export-plan.md), [Split](split.md), [Optimize](optimize.md) |
| Dukungan/pengujian | [Kompatibilitas](compatibility.md), [hasil pengujian](testing.md) |
| Arah konversi | [Roadmap konversi](conversion-roadmap.md), belum ada converter aktif |
| Kontrak awal | [Blueprint Level 1](blueprints/PDF_EDITOR_CODEX_BLUEPRINT.md), memuat status historis |
| Sejarah sesi | [Arsip handoff](archive/PROJECT_HANDOFF_HISTORY.md), bukan titik mulai kerja |

Dokumen multi-tab menjelaskan perilaku shell baru. Catatan pengujian/blueprint lama
tetap historis; khususnya suite M3 Open-replacement belum dimigrasikan ke multi-tab.

## Konvensi path dan privasi

- Tautan Markdown relatif terhadap file yang memuat tautan tersebut.
- Path kode, command, `.tools`, `.research` dan `target` dalam teks/code block
  relatif terhadap root repository, kecuali disebut lain.
- [README root](../README.md) tetap menjadi pintu masuk setup/run bagi GitHub dan AI.
  Dokumentasi pendamping root lama telah dipindah ke folder di atas.
- [LICENSES](../LICENSES/README.md) tetap di dekat notice lisensi; tidak dipindah.
- Catatan `.research/PROJECT_HANDOFF_PRIVATE.md` dan PDF pengguna tetap lokal,
  gitignored, tidak dipindah ke docs, tidak di-commit atau diunggah.
