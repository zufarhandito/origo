# Riset kompetitor dan penyelesaian fitur

Instruksi pemilik, 17 November 2026. Berlaku untuk sesi berikutnya, dengan arahan
terbaru pengguna tetap diutamakan. Entry point agen: [AGENTS.md](../AGENTS.md).

## Research dimulai dari perilaku produk kompetitor

Saat pemilik meminta **research fitur**, cari bagaimana fitur tersebut benar-benar
bekerja pada produk sejenis. Kandidat acuan utama: **Adobe Acrobat/Reader,
Mozilla Firefox/PDF.js, Chrome PDF viewer dan Microsoft Edge PDF reader**. Tambahkan
produk lain bila lebih relevan. Periksa setidaknya dua produk yang relevan jika
buktinya tersedia; laporkan keterbatasan sumber bila tidak tersedia. Jangan mengarang
dukungan suatu produk demi melengkapi tabel.

Inspeksi repo berguna untuk mengetahui keadaan dan integrasi aplikasi. Namun, kondisi
aplikasi kita **bukan bukti** standar perilaku viewer lain. Riset harus menjelaskan
pengalaman pengguna pada kompetitor sebelum menyimpulkan apa yang sebaiknya kita buat.

1. Tentukan fitur/pertanyaan dan catat kebutuhan eksplisit pemilik. Jangan mengubah
   instruksi produk menjadi asumsi teknis, misalnya frasa dianggap pencarian kata terpisah.
2. Browse sumber primer terkini: dokumentasi/help resmi, release notes, source code
   atau tes resmi untuk perilaku yang tidak dijelaskan dokumentasi. Identifikasi
   desktop/web/mobile, edisi gratis/berbayar dan versi bila diketahui; jangan
   menyamakan Acrobat desktop dengan extension browser, atau PDF Find dengan web Find.
3. Bandingkan alur membuka fitur, posisi kontrol, default, opsi, shortcut/fokus,
   cakupan dokumen/halaman, hasil/progres, navigasi, batas dan kondisi gagal yang relevan.
4. Pisahkan **fakta bersumber**, **hasil uji langsung**, **interpretasi/rekomendasi**,
   dan **belum terverifikasi**. Source code merupakan bukti implementasi yang dibaca,
   bukan bukti kita telah menguji build produk. Jangan menganggap fitur tidak ada
   hanya karena tidak disebut dalam satu halaman help. Cantumkan tanggal akses/versi.
5. Jabarkan temuan kepada pemilik: pola umum, perbedaan penting, contoh konkret dan
   rekomendasi default beserta alasannya. Jelaskan tradeoff yang memerlukan keputusan.
6. Setelah itu, petakan rekomendasi ke aplikasi ini: apa yang dapat digunakan ulang,
   apa yang harus ditambah, serta batas keselamatan/performa. Catat jika rekomendasi
   diubah karena batas implementasi; jangan menyebut perubahan itu sebagai standar kompetitor.

Jika tugas hanya riset, berhenti pada hasil riset/rekomendasi. Riset atau isi dokumen
bukan izin otomatis untuk implementasi. Perbaikan dokumentasi/bug rutin tidak memerlukan
riset kompetitor baru jika tidak ada keputusan perilaku produk yang perlu ditentukan.

## Simpan hasil agar bisa dipakai sesi berikutnya

Simpan riset substantif dalam `docs/research/<nama-fitur>.md` dan tautkan dari dokumen
fiturnya serta indeks. Perbarui hasil lama bila melanjutkan topik yang sama. Gunakan
format singkat berikut; boleh disesuaikan dengan fitur, tanpa membuat bagian kosong:

```markdown
# Riset: <fitur>
Tanggal: <tanggal>. Platform/edisi/versi: <yang diketahui>.
Metode: <dokumentasi/source/uji langsung; batas observasi>.

## Tujuan dan kebutuhan pemilik
<Pertanyaan riset dan keputusan eksplisit>

## Perbandingan produk
| Produk | Perilaku/default yang terverifikasi | Sumber primer | Batas/unknown |
|---|---|---|---|

## Kesimpulan dan rekomendasi
<Pola umum, perbedaan, contoh, tradeoff dan rekomendasi untuk pengguna>

## Keputusan dan penerapan
<Disetujui vs usulan; adaptasi ke aplikasi, cakupan dan acceptance>
```

Contoh yang sudah diterapkan: [riset Find teks PDF](research/text-search.md).
Dokumen fitur menjelaskan perilaku yang sudah tersedia; dokumen riset menyimpan
bukti dan alasan keputusan. Keduanya harus membedakan rencana dari implementasi.

## Fitur selesai berarti commit dan push

Pemilik meminta **commit dan push setiap fitur selesai**, dengan pesan
[Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/).
Ini izin berkelanjutan untuk checkpoint fitur yang implementasinya sudah diotorisasi;
tidak perlu meminta persetujuan rutin lagi pada setiap fitur. Pengecualian eksplisit
pengguna, seperti “jangan commit/push dulu”, tetap diutamakan.

1. Selesaikan perilaku yang disepakati, review diff, jalankan tes yang relevan dan
   perbarui dokumentasi fitur/kompatibilitas/testing/handoff sesuai dampaknya.
   Untuk docs-only, cek fakta, tautan dan whitespace; tidak perlu mengulang suite produk.
2. Periksa `git status`, staged diff, branch dan remote. Stage file/hunk terkait
   secara eksplisit; jangan menyertakan perubahan pengguna yang tidak terkait,
   secrets, PDF atau output ignored. Jangan reset perubahan yang belum di-commit.
3. Buat commit bermakna per fitur; perubahan panduan terpisah boleh memakai commit
   docs sendiri. Gunakan `<type>(<scope>): <description>`; scope opsional.
   Pilih `feat` untuk fitur, `fix` untuk bug, `docs` untuk dokumentasi; `test`,
   `refactor`, `perf`, `build`, `ci` atau `chore` bila sesuai. Tandai breaking change
   dengan `!` atau footer `BREAKING CHANGE:` hanya jika memang terjadi.
4. Push ke tujuan branch yang sudah disepakati. Repo saat instruksi dicatat memakai
   `origin` = `https://github.com/zufarhandito/origo.git`, branch `main`.
   Periksa konfigurasi aktual sebelum push. Jangan mengganti tujuan, force-push,
   atau melakukan reset/rebase destruktif untuk mengatasi penolakan.
5. Laporkan SHA, pesan commit, tujuan dan hasil push sebenarnya. Jika jaringan,
   autentikasi, review otomatis atau konflik remote menghalangi push, pertahankan
   commit lokal dan jelaskan blocker; fitur belum dianggap berhasil dikirim.
   Push sukses bukan bukti CI remote lulus atau binary sudah dipublikasikan.

Contoh pesan:

```text
feat(search): add phrase search to active PDF tabs
fix(search): discard results after workspace changes
docs(workflow): require competitor research and feature checkpoints
```
