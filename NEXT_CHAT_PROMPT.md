# Prompt siap pakai untuk chat baru

Salin teks berikut ke chat baru:

```text
Kita lanjut proyek PDF Studio di D:\projects\bpdf. Baca PROJECT_HANDOFF.md bagian
Snapshot handoff terkini terlebih dahulu; bagian Arsip di bawahnya bersifat historis.
Lalu baca PDF_EDITOR_CODEX_BLUEPRINT.md dan dokumen/kode yang relevan.

Status: M1 viewer, M2 organizer, M3 safe Save As/Extract sudah diimplementasikan.
Kebijakan ekspor sekarang v4: whole-source Extract bisa byte-identical; partial
single-source tagged Extract meminta persetujuan Simpan tanpa tag dan mempertahankan
passive Ink yang didukung. Bookmark/metadata loss juga memerlukan persetujuan.
Error memakai popup GPUI Kit dengan selectable text dan tombol Salin pesan.
Konversi masih kartu Akan datang, bukan engine yang sudah berfungsi.

Commit implementasi terakhir: 8adf7e9; HEAD dapat lebih baru karena commit handoff.
Verifikasi terakhir: 37 unit tests, Clippy, native,
release build dan M3 UI lulus; lihat docs/testing.md, jangan menganggap tes lama baru
saja dijalankan. run.cmd memakai release. Jangan menutup sesi pengguna untuk rebuild,
install ulang toolchain, atau menghapus .tools. Semua toolchain sudah disiapkan di D:.

Untuk reproduksi aktual, ada catatan lokal gitignored di
.research/PROJECT_HANDOFF_PRIVATE.md. Jangan commit/upload PDF atau catatan pribadi.
Periksa Git/proses/keberadaan sumber lagi; jangan menganggap PID historis masih aktif.
Push masih menunggu konfirmasi eksplisit destination sebagaimana dicatat handoff.

Permintaan saya berikutnya: [tulis pekerjaan yang ingin dilanjutkan di sini].
```
