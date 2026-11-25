# Kompresi PDF Pintar (Smart / Size Presets)

Origo menyediakan dua mode pengecilan PDF di menu **Alat**, 100% lokal (offline) dan *no-clobber* (tidak menimpa file asli):

1. **`Alat → Optimize PDF (lossless)…`** — membersihkan struktur & object stream via `qpdf` tanpa mengubah gambar atau teks vektor.
2. **`Alat → Kompres PDF (Ukuran)…`** — merasterisasi dan menyandikan ulang halaman ke JPEG terkompresi (`/DCTDecode`) dengan pilihan preset DPI/kualitas dan opsi **Skala abu-abu (Grayscale)**, sangat efektif untuk dokumen hasil scan, foto, atau lampiran formulir dengan batas ukuran ketat.

## Preset Kualitas & Ukuran

| Preset | DPI | Kualitas JPEG | Kegunaan |
|---|---:|---:|---|
| **Tinggi (Cetak / Arsip)** | 200 | 85% | Teks & foto tetap tajam untuk cetak dan arsip |
| **Seimbang (Email / Scan)** *(default)* | 150 | 75% | Ideal untuk dokumen scan & lampiran email |
| **Ukuran Minimal (Upload)** | 100 | 60% | Ukuran terkecil untuk unggah formulir/CPNS/WhatsApp |

- **Skala abu-abu (Grayscale)**: mengubah halaman ke `/DeviceGray` (8-bit luma) untuk penghematan tambahan pada dokumen teks/scan hitam-putih.
- **Dimensi aman**: halaman berukuran sangat besar (misalnya poster arsitektur) otomatis di-clamp proporsional ke maksimum `4096 px` per sisi / `12 Mi-piksel` sambil tetap mempertahankan ukuran fisik `MediaBox` asli dalam poin PDF.
- **Editan aktif ikut terbawa**: susunan halaman, rotasi, nomor halaman, watermark, tanda tangan, dan anotasi teks pada tab aktif ikut dikompres ke PDF baru.
- **Validasi & atomik**: hasil ditulis ke direktori sementara pada volume tujuan, diverifikasi jumlah halaman & `MediaBox` lewat PDFium (`inspect_output`), lalu di-commit atomik (*no-clobber*). Nama file bawaan otomatis menambahkan akhiran `-kompres`, `-kompres-tinggi`, `-kompres-minimal`, serta `-bw` saat Grayscale aktif (dan penomoran `(2)` bila sudah ada).
- **Laporan jujur**: setelah selesai, banner menampilkan perbandingan ukuran sebelum → sesudah serta persentase penghematan (atau saran memakai *Optimize PDF lossless* bila file sumber sudah sangat ringan).

## Verifikasi

```powershell
. .\scripts\use-build-environment.ps1
cargo test -p pdf-application -p pdf-platform --locked --offline
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\compress-ui-smoke.ps1 -Profile debug
```
