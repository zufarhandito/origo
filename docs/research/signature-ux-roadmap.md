# Riset dan rencana peningkatan tanda tangan

Tanggal: 20 November 2026. Status: **usulan, belum izin implementasi**.
Metode: dokumentasi resmi dan ekstraksi teks halaman publik iLovePDF web;
pembanding Adobe Acrobat Reader desktop. Bukan uji interaktif vendor, tidak
mengunggah dokumen. Percobaan fetch HTML mentah tambahan ditolak HTTP 403;
tidak dipakai sebagai bukti perilaku. Detail yang bergantung runtime tetap unknown.

## Kebutuhan dan pemisahan konsep

Pemilik ingin merencanakan kelengkapan Sign PDF iLovePDF: handwriting font,
inisial, stempel, draw/import yang sudah ada, dan gambar dari HP via QR.
Jenis objek (tanda tangan/inisial/stempel) dipisahkan dari cara membuat
(ketik/gambar/impor). Handwriting font adalah teks dengan font bergaya tulisan
tangan, bukan generator yang meniru tanda tangan pribadi.

## Fakta bersumber

- [Halaman tool iLovePDF](https://www.ilovepdf.com/sign-pdf): signature dan initials
  memiliki pilihan teks, draw/mobile dan upload; format upload PNG/JPG/SVG.
  Company Stamp berupa upload. Ada field Name, Date, Text, Input serta pilihan
  Only me/Several people. Pengaturan request mencakup urutan, kedaluwarsa,
  notifikasi/reminder, bahasa, email, UUID, verifikasi dan branding; sebagian
  bertanda Premium. Digital Signature juga ditandai Premium. QR verifikasi
  audit berbeda tujuan dengan QR untuk menggambar di HP.
- [Panduan membuat signature iLovePDF](https://www.ilovepdf.com/blog/create-digital-signature-online),
  22 Maret 2023: pilih font dan warna pada Text, gambar dengan mouse atau scan QR
  memakai touchscreen HP pada Draw. Ini mengonfirmasi alur QR, bukan protokolnya.
- [Panduan self-sign iLovePDF](https://www.ilovepdf.com/blog/how-to-add-a-signature-to-a-pdf-document-online-using-ilovepdf),
  28 September 2023: nama/inisial, Text/Draw/Upload, Apply, letakkan field,
  Sign, lalu unduh hasil. Nama, tanggal dan teks juga dapat ditempatkan.
- [Panduan eSign iLovePDF](https://www.ilovepdf.com/blog/how-to-digitally-sign-pdf),
  17 Februari 2021: templates pengaturan, request dengan role penerima, tracking,
  serta opsi sertifikat. Ini layanan workflow selain pembuatan aset visual.
- [User guide iLovePDF](https://www.ilovepdf.com/help/documentation): penerima
  request memperoleh email; hasil selesai menyediakan unduhan dan audit trail.
- [Adobe Reader desktop](https://helpx.adobe.com/reader/desktop/sign-pdfs.html),
  diperbarui 21 Juni 2026: signature/initials memakai Type (pilihan style), Draw,
  Image; dapat digunakan ulang, dipindahkan dengan keyboard, resize/hapus.
  Save Signature dengan akun menyimpan ke Adobe Document Cloud. Alur mobile
  capture/cloud di dokumen tersebut bukan bukti QR yang sama dengan iLovePDF.

Belum terverifikasi: nama/jumlah font dan warna, default kontrol, pressure,
ketebalan draw vendor, crop/background removal, generator stempel, batas akun
persis, retensi aset, protokol/TTL QR, dukungan jaringan lokal, resize/rotate/
duplicate vendor, serta apakah output vendor bisa diedit kembali setelah reopen.
Jangan menyimpulkan kemampuan tersebut dari label HTML atau kondisi Origo.
Tidak membuat klaim validitas hukum Origo berdasarkan pemasaran vendor.

## Kondisi Origo yang diperiksa

Lihat [fitur aktual](../signatures.md) dan apps/desktop/src/signatures.rs.
Tersedia draw mouse, undo stroke/clear, PNG/JPEG, preview, klik/drag placement,
move/resize proporsional/delete, Undo/Redo dan Save As/reopen edit milik Origo.
Draft per view, aset sementara; belum pustaka permanen, type/font, initials
terpisah, stempel UI, QR, SVG, background removal, duplicate atau rotasi objek.

Output memakai Stamp dengan image/alpha dan metadata Origo. SignatureImage domain
membatasi 512 x 256 px; batas delapan aset/objek workspace ada dalam jalur existing.
Karena itu stempel bulat dan banyak paraf perlu audit kualitas/aspect ratio/budget,
bukan sekadar mengganti label atau menaikkan semua limit tanpa pengukuran.

## UX yang diusulkan

Dropdown Tanda tangan membuka pilihan aset tersimpan dan Buat baru. Editor
berukuran sedang menggantikan popover pad yang sempit ketika fitur bertambah.
Satu pilihan jenis: Tanda tangan / Inisial / Stempel. Untuk dua jenis pertama,
pilih metode Ketik / Gambar / Impor; Gambar menyediakan aksi Dari HP.
Stempel mula-mula Impor gambar. Tidak perlu menampilkan tiga pad sekaligus.

Ketik: input nama/inisial (inisial saran dapat diedit), beberapa preview font
berlisensi redistribusi dan warna. Gambar: pad lega, warna/ketebalan, Undo dan
Hapus. Impor: preview/crop transparansi, error yang jelas; jangan recolor stempel
berwarna secara otomatis. Preview akhir dan Gunakan konsisten untuk semua metode.
Batal mempertahankan dokumen; membuat draft tidak menandai dirty. Pustaka lokal
bersifat opt-in dengan label, hapus dan preview; menghapus template tidak menghapus
objek yang sudah ditempatkan. Default tanpa upload PDF.

## Urutan task dan acceptance usulan

1. **Fondasi UX dan jenis aset.** Pisahkan state jenis/metode, preview dan Cancel/
   Gunakan, tooltip/fokus keyboard dan error. Pertahankan alur draw/import serta
   isolasi per view. Verifikasi cancel/draft tidak dirty, placement satu Undo,
   tab/view switch dan operasi saat ekspor terkunci.
2. **Ketik + inisial.** Bundle beberapa font legal beserta lisensi; spike rendering
   shaping/glyph ke aset transparan yang konsisten dengan preview dan output.
   Uji nama panjang/Unicode, glyph unsupported, kosong, font offline, warna,
   ukuran dan reopen. Raster berkualitas dahulu; vector/font embedding keputusan
   tersendiri. Hindari ketergantungan font terpasang pada komputer pengguna.
3. **Stempel impor + kualitas.** PNG/JPEG, preview aspect ratio bebas termasuk bulat,
   crop/penghapusan latar putih opsional dengan preview dan reset. Audit batas
   dimensi/decode/total memori serta jumlah objek untuk dokumen multi-halaman.
   Uji alpha, warna, EXIF, gambar rusak, fidelity cetak dan kompatibilitas metadata
   lama. SVG tahap lanjutan setelah parser dibatasi tanpa resource eksternal.
4. **Pustaka lokal + penempatan.** Simpan opsional signature/initial/stamp, rename/
   delete, persist atomik dan error aman. Tambahkan duplicate, keyboard move,
   resize yang jelas; rotasi dan terapkan ke halaman terpilih sesudah fondasi
   budget. Uji restart, aset hilang/rusak, undo/group action dan tanpa overwrite
   sumber. Nama/tanggal/text field menjadi follow-up; jangan menyebutnya form
   interaktif sebelum model/export mendukung field sungguhan.
5. **Draw dari HP via QR.** Spike transport dan pairing dahulu, lalu implementasi.
   Desktop membuat sesi, QR membuka web pad di HP, pengguna menggambar dan Kirim,
   desktop menerima preview dan tetap meminta Gunakan sebelum placement.
   Status menunggu/terhubung/diterima/kedaluwarsa, batal dan buat QR baru.
   Token acak singkat umur, satu sesi terikat draft/tab, validasi ukuran payload,
   invalidasi saat tutup/batal, dan tolak kiriman stale/replay. PDF tetap lokal;
   cukup kirim aset/goresan. Uji dua perangkat nyata, disconnect, timeout,
   multi-tab, payload invalid, keyboard/touch serta pembatalan.
6. **Cakupan layanan penuh (epic terpisah).** Request signer/role, urutan, email,
   expiry/reminder, tracking, templates workflow, audit/verification dan sertifikat
   membutuhkan backend, identitas serta desain penyimpanan tersendiri. Tetap
   tercatat agar sasaran seluruh fitur tidak diam-diam dikecilkan ke self-sign.
   Nama/tanggal/teks dapat dikerjakan lebih awal sebagai objek lokal setelah
   keputusan model. Jangan memberi label Digital Signature pada stamp visual.

Setiap tahap harus lulus source-hash invariance, Save As/preflight/consent/
no-overwrite, output render/reopen dan history yang relevan. Uji multi-page serta
DPI/zoom/rotasi halaman untuk perubahan placement. Jangan menggandakan batas
memori tanpa budget dan bukti pengukuran.

## Pilihan transport QR yang masih perlu keputusan

- **LAN langsung:** aset tidak melalui server, tetapi HP/PC perlu jaringan yang
  dapat saling mengakses; firewall, guest Wi-Fi dan setup HTTPS dapat mengganggu.
- **Relay HTTPS:** lebih mudah lintas jaringan, tetapi perlu endpoint web,
  biaya/availability dan kebijakan retensi; aset tanda tangan melewati layanan.
  PDF tidak perlu dikirim. Token QR bukan bukti identitas penanda tangan.

Rekomendasi: kerjakan 1-3 dahulu, lalu 4, sementara 5 dimulai spike terpisah.
Pilih relay jika targetnya scan-and-use lintas jaringan; LAN jika prioritas utama
sepenuhnya lokal. Ini rancangan Origo, bukan klaim arsitektur internal iLovePDF.
Generator stempel teks/bulat adalah enhancement opsional Origo yang harus dipilih
secara eksplisit, bukan fitur vendor yang sudah terbukti. Seluruh tahap di atas
masih plan-only sampai pemilik memilih implementasi berikutnya.
