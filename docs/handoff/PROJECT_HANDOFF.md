# Handoff terkini — 17 November 2026

Permintaan terakhir pemilik: **update handover dan progress docs sebelum pindah task**.
Sesi ini docs-only. Seluruh tahap multi-tab 1 → 2 → 3 → 4A → 4B dan perbaikan
background tab aktif sudah diimplementasikan. Task produk berikutnya belum dipilih;
jangan otomatis mengerjakan backlog, AI atau rilis.

Mulai dari [indeks](../README.md), [progres](../PROGRESS.md), [perilaku multi-tab](../multitab.md),
[roadmap/keputusan](../research/multitab-roadmap.md) dan [bukti pengujian](../testing.md).
Snapshot sebelumnya dipertahankan di [arsip sesi](../archive/PROJECT_HANDOFF_2026-11-17_MULTITAB.md);
pernyataan terkini/belum dikerjakan di sana berlaku pada checkpoint historisnya.

## Git dan push yang diperiksa

- Saat mulai handover: working tree bersih, branch `main`, HEAD **`70287a7`**,
  `fix(tabs): restore strip background to distinguish active tab`.
- Remote `origin`: **https://github.com/zufarhandito/origo.git**.
- Cache `origin/main`: **`a1647e0`**; reflog lokal terakhir bertuliskan `update by push`.
  Cache kini mencakup implementasi 4B, berbeda dari laporan lama bahwa semua tahap
  masih lokal. Ini observasi cache/reflog, bukan verifikasi ulang server, pelaku push atau CI.
- `git rev-list --left-right --count HEAD...origin/main` saat mulai: **`1 0`**.
  Fix `70287a7` di atas cache; belum termasuk commit handover ini. Periksa Git lagi.
- Agen tidak menjalankan fetch/push pada handover ini. Push sebelumnya pernah ditolak
  automatic approval review karena izin tujuan/payload spesifik; pertanyaan konfirmasi
  dalam percakapan belum dijawab eksplisit. Perubahan cache bukan jawaban persetujuan.
  Jangan retry/bypass atau mengklaim push sukses.
- Instruksi pemilik tetap Conventional Commit lalu push untuk fitur terotorisasi
  yang selesai, sesuai [workflow](../feature-development.md). Tidak mencakup binary,
  PDF pengguna atau data privat. Catat SHA dan hasil push sebenarnya.

## Kemampuan nyata dan keputusan terkunci

Viewer/organizer lokal, multi-tab, Find, safe Save As/Extract, Split PDF, Optimize
lossless dan PNG tersedia dalam [batas kompatibilitas](../compatibility.md).
Tahap/commit lengkap ada di [progres](../PROGRESS.md). Kontrak multi-tab:

- Maksimal 8 dokumen termasuk loading; duplicate Open memakai canonical Windows path.
  Drag reorder, menu konteks, + setelah strip dan bintang permanen lokal tersedia.
- Favorit berdasarkan path; rename/move dianggap berbeda. Reopen maksimal 20 entri
  metadata selama proses berjalan; edit yang dibuang tidak dipulihkan.
- Startup menawarkan restore sesi baca, tanpa recovery edit. Metadata bounded lokal
  di `%LOCALAPPDATA%/PDF Studio`; `--state-dir`/`PDF_STUDIO_STATE_DIR` untuk isolasi.
  Clear history, metadata rusak, partial restore dan gagal simpan ditangani.
- Dua PDF berdampingan memakai Dock GPUI Kit 0.9.1, splitter native dan minimum panel
  320 logical px. Renderer uncached menjaga aksesibilitas setelah modal/fokus berubah.
- Dua view satu PDF berbagi Workspace/history/seleksi halaman/revisi/dirty; viewport,
  zoom/pan, Find, seleksi teks dan anchor rentang per view terpisah. Identitas dokumen
  `TabId` dibedakan dari `ViewId`. Maksimal satu view tambahan disimpan per tab.
- Toolbar/organizer mengikuti fokus. Actor dokumen utama tetap memiliki job/receipt/
  close guard walau view-nya tersembunyi. Header × menutup view; tab ×/Ctrl+W menutup
  dokumen dengan satu guard. Membuka view kedua lagi memakai state yang disimpan.
- Maksimal dua panel terlihat membagi budget default jendela: page 128 MiB, thumbnail
  16 MiB, teks 16 MiB dan queue 32. Hidden views melepas cache/GPU dan berhenti
  menjadwalkan kerja. Budget cache bukan batas seluruh memori proses.
- Edit mempertahankan PageId baca atau indeks terdekat jika dihapus. Source changed
  menginvalidasi kedua view. Session/reopen memakai satu posisi dari view pilihan.
- Background strip kembali memakai `tab_bar`, berbeda dari folder aktif `tab_active`;
  penanda tab aktif terlihat pada tema terang/gelap.

## Verifikasi dan build terakhir

Bukti berikut berasal dari implementasi sebelum handover, **bukan tes ulang** sesi ini.
Handover hanya memeriksa ulang manifest, hasil numerik dan kesamaan hash executable:

- Check 4B lulus **57 unit tests**, fmt, Clippy denied warnings dan release workspace
  build. Setelah hardening splitter: 10 desktop tests, workspace Clippy/fmt dan
  release build lulus lagi.
- Same-document UI **9 kasus**, comparison **10**, Find **14**, restore sesi **8**
  berstatus passed; regresi tabs dan organizer/M2 juga lulus. Path di [testing](../testing.md).
- Profil same-document lulus **3 putaran/31 sampel**, 8 PDF sintetis × 1.000 halaman,
  kedua view 400%: puncak private 330.465.280 byte, warmed growth 679.936 byte dan
  handle growth 10; idle cache/native residency kembali nol. Corpus profil dua PDF
  berbeda, jadi angka keduanya bukan perbandingan performa langsung.
- Fix background lulus fmt/Clippy, debug build, isolated release build dan empat
  capture A/B aktif × terang/gelap dengan inspeksi visual/sampling warna. Tidak
  menambah atau mengulang unit tests untuk perubahan satu baris styling.
- `target/release/pdf-studio.exe` ada; hash masih cocok dengan build teruji di
  `target/tab-background-check/release/pdf-studio.exe`. `run.cmd` memilih release,
  fallback debug, tanpa build otomatis.
- Saat fix, review otomatis menolak memindahkan executable yang sedang digunakan.
  Solusinya build terpisah. Setelah pengecekan baru memastikan executable utama
  tidak dipakai, hasil teruji disalin dan hash dicocokkan. Tidak ada proses pengguna
  ditutup. Keadaan proses saat itu bukan izin menutup proses pada sesi baru.
- Tidak ada build/tes produk, scan keamanan baru, fetch, verifikasi CI remote atau
  publikasi binary dalam handover ini. Private notes/PDF pengguna tidak dibaca.

## Pekerjaan berikutnya dan batas yang tetap terbuka

Multi-tab yang diotorisasi selesai. Group tab/tab vertikal tetap backlog tanpa
jadwal; pin/preview/MRU/multi-window menunggu evaluasi. JPEG/TXT, OCR/Office dan AI
belum diimplementasikan. Tunggu pemilik memilih task berikutnya.

Gate rilis: DPI 150/200% fisik, mixed-monitor, trackpad, keyboard/Narrator;
clean Windows 10/11; lisensi/notice/redistribusi; native security dan disposition
advisory dependency. Lihat [device acceptance](../device-acceptance.md),
[audit distribusi](../distribution-audit.md) dan [release checklist](../release-checklist.md).
Jangan menyatakan public-release ready atau corpus sintetis sebagai jaminan bebas leak.

Model bisnis AI online berbayar masih ditunda. Konteks disimpan di
[arsip](../archive/PROJECT_HANDOFF_2026-11-17_MULTITAB.md#konteks-ai-berbayar-yang-ditunda);
provider/model/backend/akun/pembayaran/upload belum dibuat. Handover bukan izin AI.

## Aturan kerja untuk sesi berikutnya

Bahasa Indonesia. Ikuti [AGENTS.md](../../AGENTS.md) dan [workflow](../feature-development.md).
Riset fitur dimulai dari kompetitor/sumber primer; pisahkan fakta, pengamatan langsung,
rekomendasi dan unknown. Docs-only cukup cek fakta, tautan dan whitespace.

PDF sumber read-only; pertahankan snapshot/preflight/consent/validasi/no-overwrite.
Hanya full Save As dengan instance/revisi tepat menandai clean. `.research`, `.tools`,
`sidecars`, `target`, `dist`, PDF pengguna dan catatan privat tetap lokal/ignored.
Jangan membaca catatan privat, mereset perubahan pengguna, mengubah/memindahkan
toolchain terdaftar atau menutup proses/dokumen pengguna. Jangan memindahkan executable
aktif; gunakan target build terpisah, lalu cek proses lagi sebelum update binary.

Command dari `D:\projects\bpdf`, PowerShell; toolchain dipin. Helper lokal
[use-local-rust.ps1](../../scripts/use-local-rust.ps1) tersedia. Headed UI suites
bergantian memakai PID/HWND dan state-dir fixture sendiri. Script Windows PowerShell
berlabel Unicode memakai UTF-8 BOM. Jangan install ulang SDK/VS/Rust atau mengulang
seluruh suite hanya untuk handover dokumentasi.

[Prompt siap salin](NEXT_CHAT_PROMPT.md).
