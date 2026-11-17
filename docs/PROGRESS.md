# Progres PDF Studio

Checkpoint **17 November 2026**: anotator inti commit **`8a40b73`**, sudah di-push
ke `origin/main` (remote `a1647e0..8a40b73`). Diimplementasikan setelah persetujuan
pemilik. Highlight teks, pen/stabilo, pilih/hapus/warna, shared history, unsaved guard
dan Save As/reopen tersedia. Tanda tangan ditunda. Lihat [anotator](annotations.md),
[handoff](handoff/PROJECT_HANDOFF.md) dan [testing](testing.md) untuk batas dan bukti.
Angka/commit multi-tab di bawah adalah checkpoint historis sebelum anotator.

## Multi-tab yang sudah selesai

| Tahap | Hasil | Commit |
|---|---|---|
| 1 | Drag reorder, bintang kuning permanen lokal, + setelah strip, menu konteks/guard subset close | `615e57e` |
| 2 | Reopen metadata-only (20 entri per proses), pencarian Daftar tab | `7c9db9a` |
| 3 | Tawaran restore sesi baca, posisi/zoom, partial restore, clear history/failure handling | `bf51fb6` |
| 4A | Dua PDF berdampingan, Dock GPUI Kit/splitter, fokus/organizer/ekspor dan budget jendela | `73e2409` |
| 4B | Dua view satu Workspace/history, posisi baca/Find/seleksi teks terpisah, satu actor/guard dokumen | `a1647e0` |
| Perbaikan visual | Background bar tab dipulihkan agar folder tab aktif terlihat di kedua tema | `70287a7` |

Seluruh urutan yang diotorisasi telah diimplementasikan. Keputusan dan riset awal
tetap ada di [roadmap](research/multitab-roadmap.md); kontrak penggunaan aktual di
[multi-tab](multitab.md). Tidak ada kenaikan batas delapan dokumen atau pemulihan edit
yang sudah dibuang. Split view berbeda dari ekspor [Split PDF](split.md).

## Kemampuan lain yang tetap tersedia

Viewer lokal (navigasi/zoom/thumbnail/seleksi teks), organizer/history, safe Save As/
Extract, [Find](search.md), [Split PDF](split.md), [Optimize lossless](optimize.md) dan
[PNG](png.md). Sumber tetap read-only; ekspor mempertahankan snapshot, preflight,
consent, validasi dan no-overwrite. Hanya full Save As yang cocok menandai clean.

## Verifikasi yang sudah dicatat

- Check tahap 4B: **57 unit tests**, fmt, denied-warning Clippy dan release workspace
  build lulus; pemeriksaan terfokus berikutnya dicatat terpisah di testing.
- UI: **9** same-document, **10** comparison, **14** Find dan **8** restore sesi
  passed, ditambah regresi tabs dan organizer/M2.
- Profil dua view satu PDF: **3 putaran, 31 sampel**, delapan PDF sintetis masing-masing
  1.000 halaman, kedua view 400%. Puncak private 330.465.280 byte, warmed growth
  679.936 byte, handle growth 10; idle cache/native residency kembali nol.
- Profil dua PDF berbeda juga lulus; corpus/workload berbeda, bukan pembanding langsung.
- Background tab: empat capture A/B aktif × terang/gelap, inspeksi visual dan sampling
  warna lulus; fmt/Clippy, debug dan isolated release build lulus. Executable release
  utama diperbarui setelah dipastikan tidak dipakai, tanpa menutup proses pengguna.
- Sesi handover hanya memeriksa ulang manifest/hash/tautan/whitespace, tidak menjalankan
  tes produk atau memberi bukti remote CI baru. Build lokal bukan publikasi binary.

## Backlog dan gate yang belum selesai

| Pekerjaan | Status / batas otorisasi |
|---|---|
| Group tab, tab vertikal | Backlog tanpa jadwal; belum diimplementasikan |
| Pin, hover preview, MRU, multi-window | Menunggu evaluasi pemakaian; tidak otomatis masuk tahap 1–4B |
| Pemulihan edit/crash-orphan | Belum tersedia; session restore hanya metadata baca |
| DPI fisik/mixed-monitor, trackpad, keyboard/Narrator | Gate acceptance manusia/perangkat; hasil otomatis tidak menutupnya |
| Clean Windows, lisensi/notice/redistribusi, native security/advisory review | Gate rilis, bukan fitur yang selesai karena build lokal lulus |
| JPEG/TXT, OCR/Office, converter lain | Ditunda/belum diimplementasikan; lihat conversion roadmap |
| AI online berbayar | Diskusi bisnis ditunda; belum ada backend/model/akun/pembayaran/upload |
| Publikasi binary/portable | Belum diotorisasi pada handover ini; kandidat internal historis bukan rilis terbaru |

Lihat [device acceptance](device-acceptance.md), [release checklist](release-checklist.md),
[audit distribusi](distribution-audit.md) dan [conversion roadmap](conversion-roadmap.md).
Task berikutnya belum dipilih oleh pemilik. Daftar ini bukan izin eksekusi otomatis.

## Git pada awal handover

Branch `main`, HEAD `70287a7`; cache `origin/main = a1647e0`, selisih **1 ahead / 0
behind**, belum termasuk commit dokumen ini. Reflog tracking mencatat `update by push`;
tidak ada fetch/push/verifikasi server oleh agen pada handover ini. Jangan mengulang
klaim lama bahwa semua commit multi-tab masih di atas cache upstream.

Push agen sebelumnya ditolak automatic approval review karena persetujuan tujuan/
payload; konfirmasi percakapan belum dijawab eksplisit. Jangan melewati penolakan
berdasarkan perubahan cache saja. Remote yang diperiksa:
`https://github.com/zufarhandito/origo.git`, `main`. Lihat handoff/Git untuk status setelah
commit handover. PDF privat, `.research`, toolchain dan binary tetap lokal/ignored.
