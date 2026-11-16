# Prompt siap pakai untuk chat baru

Salin teks berikut. Ganti baris permintaan terakhir sesuai pekerjaan berikutnya.
Status dicatat pada 16 November 2026; periksa Git lagi saat melanjutkan.
**Update:** pengguna telah meminta implementasi rancangan. UI desktop/multi-tab kini
ada; baca `docs/multitab.md`, bagian update handoff dan testing terbaru.
Follow-up hardening/profiling dan tab utuh ada di `docs/multitab-hardening.md`;
baseline hardening b749691. Checkpoint prioritas 3–5 memakai baseline 582cd67;
baca `docs/checkpoint-priorities-3-5.md` dan `docs/device-acceptance.md` untuk gate fisik.

```text
Kita lanjut proyek PDF Studio di D:\projects\bpdf. Berkomunikasi bahasa Indonesia.
Baca berurutan:
1. docs/README.md
2. docs/handoff/PROJECT_HANDOFF.md
3. docs/plans/UI_REDESIGN_MULTITAB_PLAN.md
4. docs/multitab.md dan bagian terbaru docs/testing.md
5. docs/checkpoint-priorities-3-5.md, docs/m3-ui-coverage.md, docs/device-acceptance.md
Lalu baca dokumen fitur/arsitektur dan kode yang relevan. Blueprint awal ada di
docs/blueprints/PDF_EDITOR_CODEX_BLUEPRINT.md; snapshot lama dipertahankan di
docs/archive/PROJECT_HANDOFF_HISTORY.md, bukan instruksi eksekusi terkini.

M1 viewer, M2 organizer, M3 safe Save As/Extract, unsaved guard, M4 Split dan
Optimize lossless sudah diimplementasikan. Pengguna kemudian meminta implementasi
rancangan yang disetujui. UI desktop dan multi-tab sekarang sudah dibuat: grouped
menus, WindowShell, DocumentSession terpisah, Open multi-file/deduplikasi, close
batch transactional dan renderer/koordinasi ekspor bersama. Konversi tetap belum
aktif dan placeholder-nya tidak masuk navigasi utama. Jangan menganggap fitur
konversi/OCR/content editing atau publikasi ikut diotorisasi.

Sumber read-only, ekspor policy v4, staging/validation/no-overwrite tetap berlaku.
Hanya full Save As dengan receipt instance/revisi yang sesuai menandai clean;
Extract/Split/Optimize tidak. Optimize membandingkan baseline ekspor dari plan
sama, dapat lebih besar, tanpa rasterisasi atau JPEG re-encoding.
Multi-tab menjaga state per dokumen, asal callback async, close guard dan budget
cache bersama dengan membersihkan cache tab nonaktif. Maksimal 8 tab termasuk
loading; panel alat muncul saat dipakai. Minimum window 860 x 500, drawer <1100 px;
threshold ini default produk awal, belum hasil physical mixed-DPI acceptance.

Baseline checkpoint prioritas 3–5: 582cd67 (feat: tab), main sama dengan cache
origin/main saat inspeksi. Lihat git log/status untuk checkpoint lokal terbaru dan
pertahankan perubahan yang ada. Tidak ada fetch/push atau hasil remote CI baru;
cache upstream bukan verifikasi remote terkini. Status Git snapshot lama bersifat historis.

Verifikasi terbaru: 47 unit tests, fmt/Clippy, build debug/release, native smoke,
release hardening/tab/chrome/text/M1/Split/Optimize UI lulus. Dua profil 3 putaran
8 buku x 1000 halaman (fit dan tambahan 400%) lulus; baca batas dan angka aktual di
docs/multitab-hardening.md, bagian terbaru docs/testing.md dan update handoff. Suite tabs
menguji lifecycle baru; m3-ui-smoke kini dimigrasikan dan 25 kasus per-kasus lulus,
termasuk output yang sudah ada tidak tertimpa. Snapshot lama disimpan byte-identical
di scripts/archive/m3-single-workspace-ui.ps1.txt, dengan coverage map di docs.
Audit otomatis DPI 100% dan UIA/targeted keyboard lulus. 150/200% fisik, monitor
kedua, pinch, modifier shortcuts fisik dan Narrator tetap pending; jangan dinyatakan
selesai hanya karena unit math atau API tersedia. Prioritas 4 belum penuh.
run.cmd memilih release. Jangan menutup sesi pengguna untuk rebuild, install
ulang toolchain, menghapus/memindahkan .tools atau merusak VS/SDK junction.
Toolchain sudah disiapkan di D:. Gunakan UTF-8 eksplisit untuk dokumen.

Catatan .research/PROJECT_HANDOFF_PRIVATE.md dan PDF pengguna tetap lokal,
gitignored. Jangan salin ke docs, commit atau upload. Periksa proses/path aktual;
jangan menganggap PID historis masih aktif. Command dijalankan dari root repo.

Permintaan saya berikutnya: [tulis pekerjaan yang ingin dilanjutkan di sini].
```
