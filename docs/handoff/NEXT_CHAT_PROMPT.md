# Prompt siap pakai untuk chat baru

Baca [handoff terkini](PROJECT_HANDOFF.md), [perilaku multi-tab](../multitab.md),
[roadmap](../research/multitab-roadmap.md) dan [testing](../testing.md) dahulu.

```text
Kita lanjut PDF Studio di D:\projects\bpdf, Windows x64/PowerShell. Bahasa Indonesia.
Mulai dari AGENTS.md, docs/README.md, docs/feature-development.md dan checkpoint
paling atas docs/handoff/PROJECT_HANDOFF.md. Cocokkan dengan git status/log.

Pemilik mengotorisasi seluruh tahap pematangan multi-tab, satu per satu:
1 pengelolaan tab → 2 reopen/pencarian Daftar tab → 3 restore sesi baca →
4A dua PDF berdampingan → 4B dua view dokumen yang sama.
Tahap 1 sudah di commit 615e57e. Tahap 2 di commit 7c9db9a; tahap 3 diimplementasikan pada checkpoint ini;
lihat log dan testing untuk commit/verifikasi terbaru. Tahap 4A dua PDF berdampingan
sudah diimplementasikan dan comparison release lulus 10 kasus. Lanjutkan 4B dua
tampilan satu workspace; cocokkan checkpoint paling atas handoff dengan Git aktual.

Keputusan: bintang permanen lokal berdasarkan path; reopen maksimal 20 entri
selama proses berjalan tanpa edit yang dibuang; tawarkan restore sesi baca saat
startup tanpa pemulihan edit; split memakai pemilihan pasangan tab. Dua view PDF
sama berbagi workspace/seleksi halaman, dengan pencarian/seleksi teks/viewport
terpisah. Dock GPUI Kit 0.9.1 dan splitter resize wajib dipakai untuk split.

Remote yang dicatat origin https://github.com/zufarhandito/origo.git branch main.
Push sebelumnya ditolak automatic approval review karena persetujuan tujuan/payload;
konfirmasi spesifik ditanyakan kembali pada sesi implementasi. Periksa jawaban
pemilik dan hasil Git aktual. Jangan bypass penolakan atau mengklaim telah push.
Fitur selesai tetap Conventional Commit dan push sesuai izin yang berlaku.

PDF pengguna read-only. Pertahankan snapshot/preflight/consent/validasi/no-overwrite.
Jangan membaca catatan privat atau mengunggah PDF/.research/binary. Jangan tutup
proses pengguna untuk rebuild; rename executable dalam target yang sudah diverifikasi
jika perlu. Source pinned/toolchain lokal dipertahankan. Suite UI hanya fixture sendiri.

Permintaan terakhir: lanjutkan pekerjaan yang sudah diotorisasi secara berurutan.
```
