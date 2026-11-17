# Riset: anotasi PDF dan tanda tangan visual

Tanggal akses sumber: **17 November 2026**. Status: **riset dan rekomendasi,
belum persetujuan implementasi**. Fokus desktop Windows; iLovePDF/Smallpdf yang
dibandingkan adalah layanan web, bukan aplikasi desktop mereka.

Metode: membaca help resmi, halaman produk dan API upstream; mencocokkan batas
integrasi dengan kode lokal. Tidak menjalankan aplikasi kompetitor, mengunggah PDF,
menguji akun berbayar atau memverifikasi output lintas viewer. Nomor build browser
tidak diverifikasi. Dokumentasi live dapat mendahului rollout aplikasi tertentu.

## Kebutuhan pemilik

- Riset fitur annotator: highlight dan alat anotasi lainnya, mengacu pada Edge,
  Chrome, Firefox dan Adobe.
- Riset tanda tangan: menggambar langsung dalam aplikasi atau mengimpor gambar,
  kemudian drag-and-drop ke lokasi yang diinginkan; acuan iLovePDF dan Smallpdf.
- Kerjakan riset dahulu. Backlog sebelumnya tidak otomatis dijalankan.

## Fakta bersumber: anotasi

### Klarifikasi pemilik: simpan, buka ulang, dan hapus

Pemilik mengusulkan prompt saat menutup dokumen dengan draw/highlight belum
disimpan, persistensi di PDF hasil simpan, dan kemampuan menghapusnya setelah
dibuka kembali. Ini kebutuhan yang dicatat untuk rancangan, bukan izin implementasi.

Anotasi PDF dapat tersimpan sebagai objek terpisah dari konten dasar halaman,
tetapi tetap berada **di dalam file PDF**. Save tidak harus menyatukan anotasi
secara permanen ke konten halaman. Flatten justru merupakan operasi tersendiri
yang melebur anotasi ke konten [T2]. API PDFium juga menyediakan akses dan
penghapusan anotasi halaman [T1]. Karena itu, reopened highlight/ink dapat dihapus
oleh editor yang mendukung tipenya; bukan berarti aplikasi memulihkan Undo sesi lama.

Kontrak yang diusulkan: tambah/edit/hapus anotasi membuat dokumen dirty. Close tab
atau aplikasi menawarkan **Simpan sebagai / Buang perubahan / Batal**. Save berhasil
menandai hanya revisi yang diekspor clean. Reopen file hasil simpan membaca anotasi
persisten sebagai baseline baru; menghapusnya kembali dirty dan harus disimpan lagi.
Buang perubahan hanya membuang edit sejak baseline, bukan anotasi yang sudah ada
di file. Save As saat ini tidak mengganti sumber tab; pengguna harus membuka file
hasil simpan untuk mendapatkan baseline tersebut. PDF asli tetap read-only.

Yang dibuka ulang harus **file hasil simpan**, bukan PDF lain yang hanya tampak
sama. Anotasi flattened atau coretan yang sudah menjadi pixel scan tidak dapat
dihapus sebagai objek anotasi. Dukungan edit anotasi dari viewer lain harus diuji,
bukan dijanjikan untuk semua PDF.

Microsoft mendokumentasikan simpan perubahan dan edit highlight, tetapi teks
prompt close serta round-trip erase pada build Edge tertentu **belum diuji langsung**.
Sumber tambahan: [Edge PDF reader, save changes](https://www.microsoft.com/en-us/edge/features/pdf-reader).

| Produk/edisi | Alur dan fitur terdokumentasi | Batas bukti |
|---|---|---|
| Microsoft Edge, PDF reader bawaan desktop | Seleksi teks → klik kanan → highlight dengan pilihan warna. Ink memiliki warna/lebar garis. Add Comment pada seleksi membuat catatan yang dapat dibuka, diubah dan dipreview dari ikonnya. [E1] | Help enterprise bukan jaminan kesamaan UI semua build; impor gambar tanda tangan belum terverifikasi dari sumber ini. |
| Google Chrome, viewer bawaan **Computer** | Draw membuka panel pen/highlighter; ukuran dan warna dapat dipilih. Ada eraser, Undo/Redo, tanda tangan dengan pen, serta Add text dengan font/style/warna. Panel menyusut ke bawah pada jendela kecil. Help juga menjelaskan highlight melalui seleksi keyboard. [C1] | Jangan menyebut Chrome hanya viewer. Detail snapping mouse, penghapus per-stroke/per-segmen, impor tanda tangan dan komentar ber-thread belum terverifikasi. Halaman Android berbeda dan mengarahkan Edit ke Drive. |
| Firefox, viewer bawaan desktop | Highlight teks/area, ubah warna, hapus, show/hide. Drawing memiliki warna, ketebalan, opacity; tersedia penambahan teks. Komentar dapat dibuat, diedit, dihapus, dipindah dan dikelola di sidebar. Gambar bisa diimpor, dipindah, di-resize dan diberi alt text. [F1] | Help diperbarui 18 Mei 2026; tidak menetapkan build terpasang. Perilaku tiap tipe anotasi setelah reopen belum diuji. |
| Adobe Acrobat desktop | Seleksi teks menyediakan highlight, underline, strikethrough. Alat gambar meliputi garis, panah, persegi, lingkaran, polygon/cloud, connected lines dan freehand; ada penghapus gambar, resize, Delete, serta komentar pada markup. [A1][A2] | Bukan matriks entitlement Reader gratis vs Standard/Pro. Jangan menganggap Edit PDF/redaction yang berdekatan di menu sebagai bagian gratis dari annotation. |

Interpretasi: ada dua interaksi yang perlu jelas bagi pengguna: **sorot teks** yang
mengikuti teks terpilih, dan **stabilo bebas** untuk area/gambar. Highlight permanen
juga berbeda dari warna hasil Find yang hanya membantu navigasi.

## Fakta bersumber: tanda tangan

| Produk | Pembuatan dan penempatan | Batas/cakupan |
|---|---|---|
| iLovePDF web | Only me → buat tanda tangan dari teks/draw/image → Apply → drag field dari Signing Options ke halaman → Sign PDF → download. Halaman tool menerima PNG/JPG/SVG, juga menyediakan initials dan company stamp. [I1][I2] | Self-sign dan Request Signatures adalah alur berbeda. Dokumentasi menjelaskan date/initials dan pengumpulan tanda tangan dengan audit trail [I3]; detail resize/rotasi dan kuota paket belum diuji. |
| Smallpdf web | Your Signature menyediakan type/draw/upload; tanda tangan di-drag ke posisi, di-resize, lalu Finish dan download. Permintaan ke orang lain diarahkan ke Sign.com. [S1] | Jangan menyamakan self-sign dengan audit trail layanan request. Harga/kuota akun belum diperiksa. |
| Smallpdf Edit PDF web | Add image menerima PNG/JPG; gambar diposisikan dan di-resize dari sudut, lalu Export. PNG transparan disarankan untuk menghindari kotak latar. [S2] | Ini alur gambar di editor, berbeda dari field tanda tangan atau sertifikat. Penghapusan background otomatis tidak dibuktikan. |
| Firefox desktop | Add signature → Add New Signature → Type/Draw/Image → Add. Ada clear, deskripsi dan pilihan menyimpan untuk penggunaan ulang. Mozilla menjelaskan ini anotasi tanpa autentikasi/verifikasi. [F1] | Tidak menggeneralisasi pernyataan hukum vendor ke semua yurisdiksi. |
| Acrobat desktop | Apply lalu klik lokasi; tanda tangan dapat digeser dan di-resize lewat handle sudut. Dialog mendokumentasikan type/draw/image. [A3] | Kemampuan mengedit ulang setelah finalisasi harus dibedakan dari penempatan sebelum simpan; belum diuji langsung. |
| Chrome desktop | Tanda tangan digambar dengan pen langsung pada halaman, dengan ukuran/warna pilihan. [C1] | Bukan bukti adanya pustaka signature atau impor gambar tanda tangan. |

Kesimpulan teknis: kebutuhan pemilik adalah **tanda tangan visual yang dapat
ditempatkan**, dengan dua metode input wajib: draw dan gambar. Sertifikat digital,
identitas terverifikasi, timestamp dan audit trail adalah kemampuan terpisah. Jangan
memberi label "terverifikasi" atau menjanjikan dokumen anti-perubahan hanya karena
gambar tanda tangan sudah ditempel. Riset ini tidak menilai keabsahan hukum.

## Rekomendasi UX untuk PDF Studio (usulan)

### Annotator

1. Tambahkan akses **Anotasi** pada toolbar/menu Alat. Mode aktif harus terlihat;
   pilihan objek, seleksi teks, pan, pen dan penghapus tidak boleh berebut gesture.
2. Prioritas awal: highlight teks, stabilo bebas, pen, pilih/hapus objek, warna dan
   ketebalan, Undo/Redo. Sorot teks dapat dipanggil dari seleksi yang sudah ada.
   Usulan default highlight kuning transparan; bukan klaim default semua kompetitor.
3. Berikutnya: underline/strikethrough, catatan dengan sidebar, text box, lalu
   panah/persegi/lingkaran. Sticky note dan text box berbeda: isi catatan dibuka
   melalui ikon/panel; text box selalu terlihat pada halaman.
4. Usulan eraser awal menghapus stroke utuh, bukan pixel dan bukan konten PDF asli.
   Tombol Delete menghapus objek terpilih. Jika nanti menghapus sebagian stroke,
   perlakukan sebagai fitur tersendiri dengan tes geometri dan Undo.
5. Esc membatalkan gesture/placement dan kembali ke seleksi. Delete/Ctrl+Z pada
   input komentar tetap berlaku pada teks. Satu gesture drag/draw menjadi satu
   operasi Undo, bukan satu operasi per mouse-move.
6. Hide annotations hanya preferensi tampilan, bukan penghapusan. Save tetap
   menyertakan anotasi; PNG harus menjelaskan kebijakan inclusion secara konsisten.
   Highlight tidak boleh menutupi keterbacaan teks. Scan tanpa text layer tetap
   dapat memakai stabilo bebas; highlight teks tidak menjanjikan OCR.

### Tanda tangan draw/import dan drag-and-drop

1. **Tanda tangan → Buat tanda tangan** membuka dialog dengan tab **Gambar langsung**
   dan **Impor gambar**. Type/name-font dapat menyusul; bukan kebutuhan wajib pemilik.
2. Canvas draw mendukung mouse, Undo stroke, Hapus semua dan preview. Stylus/touch
   memerlukan acceptance perangkat; jangan mengklaim pressure sensitivity sebelum diuji.
3. Import awal PNG/JPEG. Pertahankan alpha PNG, terapkan orientasi gambar, sediakan
   crop whitespace dan preview. Opsi menghapus latar putih harus eksplisit karena
   dapat menghilangkan goresan tipis. SVG ditunda untuk menyederhanakan parser,
   meskipun iLovePDF mendukungnya; ini pilihan engineering lokal.
4. Setelah **Gunakan**, tampilkan preview posisi. Pengguna dapat drag dari item
   signature ke halaman atau klik item lalu klik lokasi sebagai alternatif.
   Drop di toolbar, gap halaman atau panel dokumen lain membatalkan penempatan.
5. Klik objek untuk drag lagi, resize proporsional dari sudut, Delete, duplikat dan
   gerak halus dengan keyboard. Setiap penempatan memakai identitas sendiri;
   menggeser satu tanda tangan tidak menggeser salinannya. Rotasi objek bisa menyusul.
6. Usulan default penyimpanan template **mati**. Pilihan "Simpan di perangkat ini"
   harus eksplisit, dapat dinamai/dihapus dan dipisahkan dari favorit/riwayat PDF.
   Menghapus template tidak menghapus tanda tangan yang sudah ada dalam dokumen.
7. Save As menghasilkan PDF baru lewat transaksi aman. Membuat template belum
   mengubah dirty dokumen; menempatkan/mengubah/menghapus tanda tangan mengubahnya.

## Pemetaan ke aplikasi: pengamatan lokal dan gap

Kode ditinjau pada dasar `47d3632`; ini inspeksi, bukan hasil tes baru:

- [Domain](../../crates/domain/src/lib.rs) punya PageId stabil, command/history,
  revision dan clean baseline, tetapi belum model anotasi dan aset tanda tangan.
- [SharedWorkspace](../../apps/desktop/src/shared_workspace.rs) berbagi dokumen
  antar-view. Anotasi harus dimiliki model bersama; tool aktif, selection handle
  dan draft gesture tetap per-view. Jangan menggandakan anotasi saat dua view terbuka.
- [Seleksi teks](../../apps/desktop/src/text_selection.rs) dan geometri ekstraksi
  dapat menjadi dasar highlight. Find tetap terpisah dan tidak mengubah dirty.
- [Preflight](../../crates/qpdf-adapter/src/preflight.rs) membatasi anotasi/signed
  PDF. [Pengecualian Ink](../../crates/qpdf-adapter/src/preflight/ink.rs) sempit,
  terkait Extract yang didukung; bukan izin umum menulis semua annotation.
- [Ekspor](../../crates/application/src/export.rs) perlu menangkap anotasi dan aset
  dalam snapshot revisi. Copy-only Extract harus dinonaktifkan bila ada perubahan
  anotasi pada halaman terpilih, agar output tidak diam-diam kehilangan edit.

Ini lebih dari overlay UI. Agar fitur selesai, hasil harus benar-benar tersimpan,
terbaca kembali, dan lolos preflight aplikasi sendiri.

### Pilihan representasi output

Rekomendasi: gunakan anotasi PDF standar yang didukung dan diuji untuk highlight,
ink dan komentar, dengan appearance yang konsisten. Tanda tangan visual dapat
menjadi ink/stamp dengan appearance gambar; bukan field signature kriptografis.
Detail writer perlu spike terpisah sebelum diputuskan.

API upstream PDFium menyediakan pembuatan anotasi, ink stroke dan objek ink/stamp
[T1]. Ini bukti adanya API upstream, **bukan** bukti wrapper `pdfium-render 0.9.5`
dan binary pin kita sudah memenuhi seluruh kebutuhan. QPDF mendokumentasikan
overlay dan flatten annotations [T2]; flatten mengubah anotasi menjadi konten,
sehingga bukan pengganti kemampuan edit komentar. Jangan meraster seluruh halaman
sebagai jalan pintas. Opsi flattened copy boleh dievaluasi terpisah dengan label jelas.

Policy baru harus mengizinkan subset anotasi pasif secara tervalidasi, mempertahankan
anotasi sumber yang didukung dan tetap menolak fitur aktif/tidak didukung secara
eksplisit. Jangan sekadar menghapus pemeriksaan Annots. PDF bersertifikat/bertanda
tangan digital tetap memakai batas lama sampai ada riset preservation khusus.

Simpan geometri dalam koordinat halaman PDF, bukan pixel viewport. Transformasi
harus menangani crop/media box, rotasi sumber+virtual, zoom/DPI dan kedua view.
Anotasi mengikuti PageId saat reorder; delete halaman dan Undo membawa anotasinya.
PNG, Split, Extract dan Optimize harus menyertakan perubahan relevan atau menolak
secara jelas selama dukungan belum tersedia. Source tetap read-only; no-overwrite,
snapshot, consent, validasi dan receipt matching tetap berlaku.

## Usulan urutan implementasi dan acceptance (belum diotorisasi)

1. **Spike round-trip:** satu highlight, ink dan image signature pada PDF sintetis;
   simpan, buka ulang di aplikasi dan viewer pembanding; tentukan writer dan policy.
2. **Fondasi bersama + highlight/pen:** identitas anotasi, geometri, hit-test,
   history, dirty, rendering dua view, ekspor dan reopen.
3. **Tanda tangan:** canvas draw/import, preview, drag/drop, resize, delete,
   duplikat dan penyimpanan lokal opsional.
4. **Komentar dan markup tambahan:** sidebar, text box, underline/strikeout, shapes.

Acceptance wajib: teks multi-baris, scan, halaman terotasi/cropped, zoom 50–400%,
dua view, reorder/delete/Undo, close guard, source changed, cancel ekspor, target
sudah ada dan receipt revisi lama. Uji alpha gambar, file rusak/terlalu besar,
canvas kosong, drag di luar halaman, keyboard/input fokus, dan batas memori aset/
jumlah stroke/history. Hasil ekspor harus tetap memuat teks asli yang searchable,
posisi/warna benar dan hash sumber tidak berubah. Uji interoperabilitas mencatat
versi viewer serta tipe yang benar-benar dapat diedit kembali, bukan hanya terlihat.

## Hal yang belum terverifikasi

- Uji langsung UI/build/rollout, warna/opacity default vendor, gesture lintas halaman,
  penghapus segmental, shortcut detail, pressure dan palm rejection.
- Interoperabilitas output, appearance/flattening, edit setelah reopen, preservation
  annotation impor dan efek terhadap accessibility tags.
- Batas akun gratis/berbayar, retensi upload/template vendor dan perilaku offline.
  Tidak ada kesimpulan privasi layanan berdasarkan halaman pemasaran saja.
- Writer pada dependency pin lokal, font untuk FreeText, alpha/blend highlighter,
  rendering anotasi dalam PDFium/PNG, dan batas angka resource final.

## Sumber primer

Semua diakses 17 November 2026. Referensi versi di bawah adalah versi dokumen,
bukan versi aplikasi yang diuji.

- [E1 — Microsoft Learn: PDF reader](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-pdf)
- [C1 — Chrome Help: Manage PDFs, Computer](https://support.google.com/chrome/answer/16215622?co=GENIE.Platform%3DDesktop&hl=en)
- [F1 — Firefox Help: View and edit PDF files](https://support.mozilla.org/en-US/kb/view-pdf-files-firefox-or-choose-another-viewer), diperbarui 18 Mei 2026.
- [A1 — Acrobat desktop: Add markups](https://helpx.adobe.com/acrobat/desktop/share-and-review-documents/review-documents/add-markups.html), 22 Oktober 2026.
- [A2 — Acrobat desktop: Shapes, lines and drawings](https://helpx.adobe.com/acrobat/desktop/share-and-review-documents/review-documents/add-shapes-drawings.html), 23 September 2025.
- [A3 — Acrobat desktop: Modify e-signatures](https://helpx.adobe.com/acrobat/desktop/e-sign-documents/fill-sign-documents/modify-esign.html)
- [I1 — iLovePDF: self-sign/request workflow](https://www.ilovepdf.com/blog/how-to-digitally-sign-pdf), 17 Februari 2021; dibaca bersama halaman tool/help live.
- [I2 — iLovePDF: Sign PDF tool](https://www.ilovepdf.com/sign-pdf)
- [I3 — iLovePDF: User guide, Sign PDF](https://www.ilovepdf.com/help/documentation)
- [S1 — Smallpdf: Document signing app](https://smallpdf.com/blog/document-signing-app), 14 Juli 2026.
- [S2 — Smallpdf: Add signature image](https://smallpdf.com/blog/add-signature-image-to-pdf), 17 Maret 2026.
- [T1 — PDFium annotation API, upstream main](https://pdfium.googlesource.com/pdfium/+/main/public/fpdf_annot.h), branch bergerak; belum verifikasi pin lokal.
- [T2 — QPDF: CLI overlay/flatten](https://qpdf.readthedocs.io/en/stable/cli.html), halaman stable terindeks 12.4.1; bukan bukti binary pin 12.4.2 telah diuji untuk fitur ini.


## Keputusan implementasi 17 November 2026

Pemilik mengotorisasi anotator lebih dahulu dan menunda tanda tangan. Implementasi
inti mencakup highlight teks, pen/stabilo, warna/ketebalan, pilih/hapus, history,
unsaved guard serta Save As/reopen edit. Detail aktual dan batas terdapat di
[Anotasi](../annotations.md). Daftar kandidat fitur lain dalam riset bukan klaim
fitur tersedia atau izin mengerjakan tanda tangan saat ini.
