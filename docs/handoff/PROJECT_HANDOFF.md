# Handoff terkini ? 17 November 2026

Permintaan terbaru pemilik: implementasikan anotator dulu; tanda tangan sesudahnya.
**Anotator inti selesai**: highlight teks, pen/stabilo, lima warna, ketebalan goresan,
pilih/hapus/recolor, Undo/Redo, unsaved-close guard, safe Save As dan reopen/edit.
Lihat [anotator](../annotations.md), [testing](../testing.md), [kompatibilitas](../compatibility.md)
dan [riset kompetitor](../research/pdf-annotations-and-signatures.md).

## Batas dan keputusan aktif

- Output memakai Ink/Highlight standar dan appearance stream vektor, bukan flatten.
  Model edit bounded v1 tersimpan dalam NM. File hasil Save As bisa dibuka kembali
  dan anotasinya dihapus; file sumber tetap read-only.
- Edit ulang hanya untuk anotasi milik PDF Studio. Anotasi Edge/Adobe/Firefox dan
  perubahan metadata oleh editor eksternal belum didukung sebagai model edit.
- Komentar, textbox, shape, underline/strikethrough, move/resize, partial eraser, OCR
  dan tanda tangan belum diimplementasikan. Tanda tangan tetap ditunda; daftar riset
  bukan klaim semua fitur sudah selesai.
- Workspace/history/revisi/ID allocator dibagi dua view; tool/draft/seleksi anotasi
  per view. Satu drag atau seleksi teks multi-page adalah satu Undo.
- Policy ekspor **v5** menambah subset pasif milik aplikasi, dengan closed-schema
  preflight, penolakan action inline/reference dan validasi round-trip sebelum publish.
  Snapshot/preflight/consent/validasi/no-overwrite serta batas form/signature/tag lama
  tetap berlaku kecuali pengecualian eksplisit yang didokumentasikan.
- Save As penuh pada revisi cocok menandai clean. Extract/Split/Optimize/PNG tidak.
  Tidak ada autosave/crash recovery. Penghapus menghapus objek utuh yang diklik.

## Hasil verifikasi sesi ini

- Fmt, Clippy denied warnings, **61 unit tests** dan release workspace build lulus.
- Annotation native smoke lulus: appearance nyata, metadata/reopen/delete, Extract
  tidak melewatkan edit, empat rotasi editor vs output, render isolation dan source bytes.
- Native regresi M1/text/search/Save As/Extract/Split/Optimize/PNG lulus, termasuk
  failure/cancel/race dan blocking compatibility yang sudah ada.
- UI release: anotator **7**, same-document **9**, Find **14** kasus lulus.
  Capture anotasi dua panel diperiksa visual. Path bukti ignored ada di testing.
- Interoperabilitas UI Acrobat/Edge/Chrome/Firefox dan profiling anotasi padat belum diuji.
  Suite comparison/session restore historis tidak diulang.

## Build dan Git

Build teruji dibuat di `target/tab-background-check/release` karena aplikasi release
lama sempat aktif. Jangan menutup proses pengguna untuk mengganti executable; cek
proses dan hash sebelum pemasangan. Setelah aplikasi lama tidak aktif, executable
teruji disalin ke `target/release/pdf-studio.exe` (dipakai `run.cmd`); hash keduanya
sama: `CD67BA1CE09D953E1D8D43D74D91B9F81FB6AE43C63825F0768B4BDD886E5396`.

Awal sesi: `main`, HEAD `47d3632`, cache `origin/main` `a1647e0` (2 ahead).
Remote diverifikasi: `https://github.com/zufarhandito/origo.git`, branch `main`.
Commit fitur **`8a40b73`**: `feat(annotations): add editable PDF highlights and ink`.
`git push origin main` **berhasil**, respons remote `a1647e0..8a40b73 main -> main`.
Ini mencakup dua commit lokal pendahulu yang sudah ada. Tidak ada force-push atau
unggah binary/PDF/data privat. Keberhasilan push bukan bukti CI remote lulus.
Catatan ini ditambahkan melalui commit dokumentasi setelah push fitur.

Multi-tab tahap 1?4B dan fix background tetap tersedia; detail kontrak di
[multi-tab](../multitab.md), hasil historis di [progres](../PROGRESS.md). Instruksi
pemilik tetap commit Conventional Commits lalu push setelah fitur terotorisasi selesai.
