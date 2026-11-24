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
update. Secara default `-DeltaMode BestSpeed` membuat paket delta (`.nupkg`)
terhadap rilis sebelumnya di folder output sehingga ukuran unduhan antar versi
jauh lebih kecil dibanding paket penuh; opsi `-DeltaMode BestSize` atau `None`
juga tersedia. `-ExecutablePath` dapat menunjuk ke build release Windows x64
yang sudah dibuat. `-UpdateFeed` dapat menunjuk ke:

- folder absolut lokal (hanya untuk build Preview),
- URL repositori GitHub (`https://github.com/<owner>/<repo>`) untuk membaca
  aset GitHub Releases melalui `GithubSource` (dengan pre-release aktif pada
  kanal Preview), atau
- URL direktori HTTPS statis lain yang menyediakan `releases.preview.json` dan
  paket `.nupkg` melalui `HttpSource`.

`-PreparedPayload` dapat memakai ulang hasil tahap `package-portable.ps1 -StageOnly`
setelah manifest dan feed yang tertanam diperiksa kembali. `-ReleaseNotes`
menyertakan berkas catatan rilis Markdown.

## Siklus pembaruan

Origo mengecek metadata feed setelah membuka aplikasi dan setiap 30 menit selama
berjalan tanpa mengunduh paket secara otomatis. Saat versi baru tersedia, Origo
menampilkan pemberitahuan di kanan bawah jendela beserta perkiraan ukuran
unduhan (ukuran paket delta jika tersedia, atau paket penuh) dan pilihan
**Unduh** atau **Nanti**. Paket hanya diunduh setelah pengguna memilih **Unduh**
(atau dapat dicek ulang kapan saja melalui **Bantuan > Periksa pembaruan**).
Jika paket delta tersedia terhadap versi yang terpasang, Velopack mengunduh
delta, memverifikasi checksum, lalu menambal paket lokal tanpa mengunduh ulang
paket penuh. Setelah selesai diunduh, Origo memberi tahu bahwa paket siap
dipasang. Saat dibuka lagi, Origo menerapkan paket yang sudah siap sebelum
membuka jendela, lalu memberi tahu versi yang sedang aktif. Kegagalan mengakses
feed membiarkan aplikasi tetap tersedia. Versi terpasang juga terlihat di
**Bantuan > Tentang Origo**.

Origo Preview menggunakan satu proses per instalasi. Proses mempertahankan
kunci selama jendela atau tray aktif. Peluncuran kedua berhenti jika proses
pertama masih aktif. Saat pengguna benar-benar keluar, proses penutupan
menyelesaikan konfirmasi dokumen dan penghentian pekerjaan OCR. Riwayat dan
template preview disimpan di `%LOCALAPPDATA%\Origo.Preview\UserData`, di luar
folder `current` yang diganti Velopack. Instalasi pengembangan memakai state
yang sudah ada di lokasi sebelumnya. File PDF pengguna berada di lokasinya
masing-masing.

## Menyiapkan dan mengunggah feed HTTPS (GitHub Releases)

Skrip `scripts/publish-preview-github.ps1` mengemas Origo Preview dengan feed
HTTPS GitHub (`https://github.com/zufarhandito/origo` secara default) dan delta
aktif. Secara default skrip hanya menaruh hasil di `dist/origo-preview-github`
tanpa mengunggah apa pun:

```powershell
. ./scripts/use-build-environment.ps1
# Rilis pertama (staging lokal saja):
./scripts/publish-preview-github.ps1 -Version 0.1.5
# Rilis berikutnya: tarik rilis sebelumnya agar paket delta terbentuk, lalu unggah draft:
./scripts/publish-preview-github.ps1 -Version 0.1.6 -SyncPrevious -Upload
# Tambahkan -Publish untuk langsung menerbitkan pre-release (memerlukan -Token atau GITHUB_TOKEN).
```

## Distribusi ke tester lain

Preview lokal memakai installer tanpa tanda tangan. Pengiriman ke tester lain
memerlukan keputusan lisensi proyek, tanda tangan penerbit, tinjauan
redistribusi (termasuk 3 crate tanpa berkas lisensi upstream), dan uji Windows
bersih. Lihat [checklist rilis](release-checklist.md),
[audit distribusi](distribution-audit.md),
[dokumentasi Velopack](https://docs.velopack.io/integrating/overview),
dan [penandatanganan Windows](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options).
