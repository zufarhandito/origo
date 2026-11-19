# Cetak PDF

Implementasi MVP Windows, 19 November 2026. Acuan perilaku dan sumber primer:
[riset cetak](research/pdf-printing.md).

Buka **Cetak PDF** dari toolbar, menu Berkas, atau **Ctrl+P**. Pilih semua halaman,
halaman yang sedang dibaca, atau rentang seperti `1-3,5`. Nomor mengikuti susunan
workspace sekarang; duplikat diabaikan dan hasil mengikuti urutan workspace.
Dari split view, Halaman ini mengikuti view yang memanggil perintah.

**Pratinjau cetak** menyiapkan snapshot halaman beserta anotasi/tanda tangan yang
sudah ditempatkan, termasuk perubahan yang belum disimpan. Draft gesture dan UI
pencarian/seleksi tidak dicetak. Jendela cetak menyediakan navigasi preview,
**Printer...** (printer, salinan, properti driver), **Kertas / orientasi...**, dan
**Fit ke area cetak**. Mematikan Fit memakai ukuran fisik 100%; bagian di luar area
cetak dapat terpotong. Kertas dipilih untuk seluruh job, bukan otomatis per halaman.
Pengaturan duplex/warna bergantung driver. Batas salinan 99.

Sumber selalu read-only. Cetak tidak mengubah Undo, revisi bersih, atau receipt
Save As. Batal sebelum submit tidak mengirim halaman; setelah submit, halaman yang
sudah masuk spooler mungkin tetap tercetak. Aplikasi tidak membatalkan job lain.
Job fisik sukses berarti diterima antrean, bukan bukti kertas sudah keluar.

## Output virtual dan batas MVP

Microsoft Print to PDF didukung melalui file sementara milik job, lalu picker
Origo meminta **file baru**. Sumber dan tujuan yang sudah ada ditolak; output
divalidasi native dan jumlah halamannya diperiksa sebelum publikasi atomik.
Tidak ada dialog save driver yang bebas menimpa sumber. Output virtual memakai
satu salinan. Printer virtual file lain (XPS, OneNote, Fax, FILE/PORTPROMPT) belum
didukung dan menghasilkan penjelasan. Port hardware yang didukung: USB, LPT, COM,
WSD dan port terdaftar Win32_TCPIPPrinterPort. Port custom/shared/IPP lain yang
tidak teridentifikasi ditolak agar dialog save driver tidak melewati guard Origo.

Semua halaman diraster 150 DPI dengan print intent PDFium; anotasi sumber mengikuti
flag printable/hidden. Hasil Print to PDF berisi gambar halaman, sehingga teksnya
tidak searchable. Gunakan Save As biasa untuk output yang mempertahankan teks.
Maksimum 8192 piksel per sisi / 16 Mi piksel per halaman, 512 MiB PNG sementara,
128 MiB PDF virtual dan budget job 30 menit. Halaman melampaui batas gagal dengan
aman; kualitas tidak diam-diam diturunkan. Preview memuat satu gambar pada satu waktu.

Adapter memakai script managed yang di-embed, Windows PowerShell 5.1 dan .NET
Framework Windows Forms/Drawing/Management. Tidak memerlukan PDF viewer eksternal,
instalasi dependency baru, pemanggilan API perubahan default printer, atau unsafe Rust. Helper
hanya menerima manifest/PNG di direktori sementara milik aplikasi. Kebijakan
Windows yang menolak helper atau komponen printer tidak tersedia akan dilaporkan.

Pengujian virtual tidak membuktikan hasil printer fisik, semua driver, DPI perangkat,
margin, warna atau duplex. Lihat [bukti dan gate](testing.md).
