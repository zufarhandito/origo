# Prompt siap pakai untuk chat baru

Baca [handoff terkini](PROJECT_HANDOFF.md), [progres](../PROGRESS.md), [perilaku multi-tab](../multitab.md),
[roadmap](../research/multitab-roadmap.md) dan [testing](../testing.md) dahulu.

```text
Kita lanjut PDF Studio di D:\projects\bpdf, Windows x64/PowerShell. Bahasa Indonesia.
Mulai dari AGENTS.md, docs/README.md, docs/feature-development.md dan checkpoint
paling atas docs/handoff/PROJECT_HANDOFF.md. Cocokkan dengan git status/log.

Seluruh tahap multi-tab yang diotorisasi sudah selesai:
1=615e57e, 2=7c9db9a, 3=bf51fb6, 4A=73e2409, 4B=a1647e0.
Fix background tab aktif=70287a7; run.cmd sudah memakai executable yang diperbarui.
Check 4B lulus 57 unit tests/fmt/Clippy/release build; UI same-document 9 kasus,
comparison 10, Find 14, session restore 8. Profil 3 putaran/31 sampel lulus.
Fix styling berikutnya diuji visual A/B aktif pada dua tema serta fmt/Clippy/build;
unit tests tidak diulang untuk satu baris styling. Handover hanya memeriksa ulang
manifest/hash/tautan/whitespace, tanpa build/tes produk baru.

Keputusan: bintang permanen lokal berdasarkan path; reopen maksimal 20 entri
selama proses berjalan tanpa edit yang dibuang; tawarkan restore sesi baca saat
startup tanpa pemulihan edit; split memakai pemilihan pasangan tab. Dua view PDF
sama berbagi workspace/seleksi halaman, dengan pencarian/seleksi teks/viewport
terpisah. Dock GPUI Kit 0.9.1 dan splitter resize wajib dipakai untuk split.

Awal handover: main bersih, HEAD 70287a7, cache origin/main a1647e0; 1 ahead/0 behind
sebelum commit docs handover. Reflog cache bertuliskan update by push, bukan bukti
verifikasi ulang server/CI atau siapa yang push. Agen tidak fetch/push pada handover.
Remote origin https://github.com/zufarhandito/origo.git branch main.
Push sebelumnya ditolak automatic approval review karena persetujuan tujuan/payload;
konfirmasi spesifik ditanyakan kembali pada sesi implementasi. Periksa jawaban
pemilik dan hasil Git aktual. Jangan bypass penolakan atau mengklaim telah push.
Fitur selesai tetap Conventional Commit dan push sesuai izin yang berlaku.

PDF pengguna read-only. Pertahankan snapshot/preflight/consent/validasi/no-overwrite.
Jangan membaca catatan privat atau mengunggah PDF/.research/binary. Jangan tutup
proses pengguna atau memindahkan executable aktif untuk rebuild. Gunakan target build
terpisah; cek proses lagi sebelum menyalin hasil ke executable utama. Source pinned/
toolchain lokal dipertahankan. Suite UI hanya fixture sendiri, dijalankan bergantian.

Permintaan terakhir: update handover dan progress docs sebelum pindah task.
Task baru belum dipilih. Group tab/tab vertikal/pin/preview/MRU/multi-window tetap
backlog; gate rilis fisik/clean Windows/lisensi/security tetap terbuka. JPEG/TXT dan
AI ditunda. Jangan otomatis mengerjakan backlog, fitur baru, packaging atau rilis.
```
