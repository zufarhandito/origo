# Installer dan pembaruan Origo Preview

`Origo Preview` adalah pemasangan Windows x64 per pengguna dengan ID
`Origo.Preview`. Shortcut berada di Start Menu. Pemasangan mencakup PDFium,
QPDF, Tesseract, model Indonesia dan Inggris, dan runtime Windows yang
diperlukan. Velopack 1.2.161 menyiapkan installer dan paket pembaruan.

## Membuat preview lokal

Siapkan runtime PDFium/QPDF dan OCR seperti di panduan build. Jalankan dari
root proyek dengan toolchain Rust dan MSVC aktif. Pada checkout baru, jalankan
`cargo fetch --locked` sebelum membangun secara offline. Skrip setup mengunduh
perangkat pengemasan .NET dan Velopack ke `.tools` dengan checksum yang dipatok.

```powershell
. ./scripts/use-build-environment.ps1
./scripts/package-installer.ps1 -Version 0.1.0
```

Buka installer `*Setup.exe` yang dilaporkan skrip. `dist/origo-preview` adalah
folder sumber pembaruan default. Simpan folder tersebut di lokasi yang sama
dan jalankan skrip lagi dengan nomor versi yang lebih tinggi untuk menyediakan
update. `-ExecutablePath` dapat menunjuk ke build release Windows x64 yang sudah
dibuat. `-UpdateFeed` dapat menunjuk ke folder absolut lokal atau direktori HTTPS
yang menyediakan `releases.preview.json` dan paket `.nupkg` yang dirujuknya.
`-PreparedPayload` dapat memakai ulang hasil tahap `package-portable.ps1 -StageOnly`
setelah manifest dan feed yang tertanam diperiksa kembali.

## Siklus pembaruan

Origo mengecek feed setelah membuka aplikasi dan setiap 30 menit selama
berjalan. Paket versi baru disiapkan di latar belakang. Saat dibuka lagi,
Origo menerapkan paket yang sudah siap sebelum membuka jendela. Kegagalan
mengakses feed membiarkan aplikasi tetap tersedia. Versi terpasang terlihat di
**Bantuan > Tentang Origo**.

Origo Preview menggunakan satu proses per instalasi. Proses mempertahankan
kunci selama jendela atau tray aktif. Peluncuran kedua berhenti jika proses
pertama masih aktif. Saat pengguna benar-benar keluar, proses penutupan
menyelesaikan konfirmasi dokumen dan penghentian pekerjaan OCR. Riwayat dan
template preview disimpan di `%LOCALAPPDATA%\Origo.Preview\UserData`, di luar
folder `current` yang diganti Velopack. Instalasi pengembangan memakai state
yang sudah ada di lokasi sebelumnya. File PDF pengguna berada di lokasinya
masing-masing.

## Distribusi ke tester lain

Preview lokal memakai installer tanpa tanda tangan. Pengiriman ke tester lain
memerlukan keputusan feed HTTPS, tanda tangan penerbit, tinjauan redistribusi,
dan uji Windows bersih. Paket awal menggunakan pembaruan paket penuh.
Lihat [checklist rilis](release-checklist.md), [dokumentasi Velopack](https://docs.velopack.io/integrating/overview),
dan [penandatanganan Windows](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options).
