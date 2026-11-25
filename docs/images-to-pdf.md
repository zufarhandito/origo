# Gambar (PNG/JPEG) ke PDF

Gunakan **Alat → Gambar ke PDF…** (aktif bahkan saat belum ada tab PDF yang terbuka) atau tombol **Gambar ke PDF** di halaman awal. Pilih satu atau beberapa berkas PNG/JPEG (`.png`, `.jpg`, `.jpeg`), atur urutan dengan tombol naik/turun atau hapus dari daftar, tentukan ukuran halaman, orientasi, margin, lalu pilih lokasi simpan PDF baru.

Secara default opsi **Buka PDF setelah selesai** aktif, sehingga hasil PDF yang lolos validasi langsung dibuka di tab baru (atau dapat dimatikan jika hanya ingin menyimpan ke disk).

## Ukuran halaman, orientasi, dan margin

- **Ukuran halaman**:
  - **A4** (`595,28 × 841,89 pt`, `210 × 297 mm`) — default.
  - **Letter** (`612 × 792 pt`, `8,5 × 11 inci`).
  - **Sesuai gambar**: ukuran halaman mengikuti ukuran fisik gambar berdasarkan metadata DPI (`pHYs` pada PNG atau `APP0 JFIF` pada JPEG), dengan fallback `96 DPI` bila metadata DPI tidak tersedia atau di luar rentang wajar (`18–2400 DPI`), dan diklamp dalam batas halaman PDF `1–14400 pt`.
- **Orientasi** (untuk A4 dan Letter):
  - **Otomatis** (default): potret untuk gambar vertikal/persegi, lanskap untuk gambar mendatar (`lebar > tinggi`).
  - **Potret** atau **Lanskap**: memaksa orientasi tetap untuk semua halaman.
  - Pada mode **Sesuai gambar**, kontrol orientasi otomatis dinonaktifkan karena dimensi halaman mengikuti rasio asli gambar (margin tetap dapat dipilih bila ingin memberi bingkai putih di sekitar gambar).
- **Margin**:
  - **Tanpa margin** (`0 pt`), **Kecil** (`18 pt` ≈ `6,3 mm`, default), **Normal** (`36 pt` ≈ `12,7 mm`).
  - Gambar diposisikan terpusat dengan skala proporsional (*aspect-fit*) sehingga tidak pernah terpotong atau melar.

## Penanganan EXIF, warna, dan passthrough JPEG

- **Orientasi EXIF**: tag orientasi EXIF pada JPEG/PNG dibaca sebelum kalkulasi dimensi dan layout (termasuk menukar sumbu DPI X/Y untuk rotasi 90°/270°).
- **Direct `/DCTDecode` passthrough**: JPEG baseline (`SOF0`) dengan ruang warna RGB (3 komponen) atau Grayscale (1 komponen) dan tanpa transformasi EXIF disisipkan langsung ke stream PDF tanpa re-encoding ulang sehingga kualitas dan ukuran berkas asli terjaga.
- **Fallback JPEG**: JPEG progresif, CMYK, atau JPEG yang memerlukan rotasi/flip EXIF didekode, diorientasikan ulang, lalu di-encode ulang sebagai baseline JPEG (kualitas 92) sebelum disisipkan.
- **PNG & transparansi**: gambar PNG dikompresi lossless memakai `/FlateDecode` (`DeviceRGB`). Jika gambar memiliki kanal alfa, piksel dikompositkan ke latar belakang putih solid (`#FFFFFF`) secara deterministik.

## Batasan dan keamanan berkas

- **Sepenuhnya luring (100% offline)**: tidak memakai layanan eksternal maupun dependensi baru di luar crate terkunci (`image` + `flate2`).
- **Batas sumber daya**:
  - Maksimal **200 gambar** per konversi.
  - Maksimal **32 MiB** per berkas gambar.
  - Maksimal dimensi **8.192 px** per sisi dan **32 megapiksel** per gambar (`128 MiB` batas alokasi dekode).
  - Maksimal ukuran keluaran PDF **512 MiB** dengan penulisan streaming satu gambar per siklus (memori O(1) terhadap jumlah halaman).
- **Validasi & publikasi atomik (no-clobber)**:
  - PDF 1.4 ditulis ke berkas staging tersembunyi di direktori tujuan, divalidasi penuh lewat PDFium (`inspect_output`) untuk memastikan jumlah halaman dan dimensi cocok, lalu dipublikasikan secara atomik tanpa menimpa berkas yang sudah ada.
  - Berkas gambar sumber hanya dibaca (*read-only*) dan tidak pernah dimodifikasi.
  - Pembatalan atau kegagalan menghapus berkas staging secara otomatis tanpa meninggalkan keluaran korup.

## Integrasi jendela & tab

- Konversi berjalan di thread latar memakai slot ekspor jendela bersama dengan tombol **Batal ekspor** di bilah status atas.
- Pengguna tetap bisa berpindah tab atau membaca PDF lain selama konversi berlangsung.
- Penutupan jendela menunggu pekerjaan konversi selesai dengan aman.

## Verifikasi

```powershell
. ./scripts/use-build-environment.ps1
cargo test --workspace --locked --offline
cargo run --locked --offline -p pdf-smoke -- --images-to-pdf-smoke
./scripts/images-to-pdf-ui-smoke.ps1 -Profile debug
```

Pengujian native mencakup PNG transparan + `pHYs` 150 DPI, JPEG `/DCTDecode` passthrough, koreksi orientasi EXIF 90° CW, preflight QPDF (`--check`), mode `Sesuai gambar`, no-clobber terhadap berkas yang sudah ada, dan pembersihan staging saat berkas korup. Pengujian UI Windows memeriksa alur layar awal, menu **Alat → Gambar ke PDF…** dengan 0 tab, tambah & ubah urutan gambar, konversi & buka otomatis di tab baru, mode **Sesuai gambar**, serta hash sumber tetap identik. Lihat [testing](testing.md) untuk catatan hasil uji.
