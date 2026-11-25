# Integrasi Klik Kanan Windows 11 File Explorer (`Origo > …`)

Origo menyediakan ekstensi shell modern Windows 11 berbasis **COM `IExplorerCommand` (`origo_shell_ext.dll`) + Sparse MSIX Package** sehingga muncul langsung di menu konteks modern Windows 11 (`App extensions` / klik kanan tanpa *Show more options*), sejajar dengan WinRAR, Notepad++, dan Zed.

## Fitur Saat Ini (Gambar PNG/JPEG → PDF)

Saat memilih 1 atau beberapa berkas `.png`, `.jpg`, atau `.jpeg` di File Explorer lalu klik kanan, muncul submenu **`Origo`** dengan ikon Origo:

1. **Buka dialog pengaturan (pre-filled):**
   - Bahasa Inggris OS: **`Create PDF from image…`** / **`Create PDF from images…`**
   - Bahasa Indonesia OS: **`Buat PDF dari gambar ini…`**
   - Membuka Origo (atau memfokuskan jendela Origo yang sudah berjalan via IPC lokal) dengan dialog **Gambar ke PDF** yang sudah terisi seluruh gambar terpilih sesuai urutan Explorer.
2. **Konversi langsung 1-klik (no-clobber):**
   - Bahasa Inggris OS: **`Create "<nama>.pdf" here`** (1 gambar) atau **`Combine into "<folder>.pdf"`** (banyak gambar)
   - Bahasa Indonesia OS: **`Buat "<nama>.pdf" di sini`** (1 gambar) atau **`Gabungkan ke "<folder>.pdf"`** (banyak gambar)
   - Langsung menggabungkan gambar ke folder yang sama dengan pengaturan default (A4, orientasi otomatis, margin kecil), memakai akhiran `-2`, `-3`, dst. bila nama file sudah ada (tidak pernah menimpa file lama), lalu membuka hasilnya di tab baru Origo.

Jika file yang dipilih bukan gambar PNG/JPEG (mis. `.txt` atau folder), `Origo` otomatis disembunyikan (`ECS_HIDDEN`) agar menu Explorer tetap bersih. Struktur [selection.rs](file:///D:/projects/bpdf/crates/shell-ext/src/selection.rs) dan [com.rs](file:///D:/projects/bpdf/crates/shell-ext/src/com.rs) dirancang agar mudah diperluas untuk aksi berkas `.pdf` ke depan (mis. *Gabungkan PDF*, *Kompres PDF*, *Ekspor ke PNG*).

## Deteksi Bahasa Sistem Operasi

- **Windows:** membaca `GetUserDefaultUILanguage()` (`primary LANGID == 0x21` → Bahasa Indonesia; selain itu → Bahasa Inggris).
- **macOS / Linux (lintas platform):** membaca `LC_ALL` / `LC_MESSAGES` / `LANG`.
- **Override pengujian:** set variabel lingkungan `ORIGO_SHELL_LANG=id` atau `ORIGO_SHELL_LANG=en`.

## Arsitektur & Keamanan

- **Manifest UTF-8 sementara (`--images-to-pdf-list` / `--images-to-pdf-quick-list` + `--delete-list`):** daftar path gambar dari `IShellItemArray` ditulis ke berkas sementara UTF-8 lalu dihapus otomatis setelah dibaca Origo, sehingga kebal batas panjang command line Windows 32 KB (mendukung hingga 200 gambar).
- **Single-Instance IPC ([apps/desktop/src/ipc.rs](file:///D:/projects/bpdf/apps/desktop/src/ipc.rs)):** memakai loopback `127.0.0.1` lokal dengan token acak 256-bit per `state_dir` (100% Rust aman, tanpa `unsafe` di `apps/desktop`). Peluncuran baru saat Origo sudah terbuka langsung meneruskan perintah ke jendela aktif lalu keluar dengan kode 0.
- **Isolasi `unsafe`:** boundary COM `IExplorerCommand` terisolasi sepenuhnya di [crates/shell-ext](file:///D:/projects/bpdf/crates/shell-ext), tanpa melemahkan `unsafe_code = "forbid"` pada crate inti Origo.
- **Surrogate COM (`<com:SurrogateServer>`):** berjalan out-of-proc di `dllhost.exe` dan memakai direktori staging per-registrasi agar proses build/update tidak terhambat *file lock* `explorer.exe`.

## Cara Memasang & Mencopot di Mode Pengembangan (Dev Mode)

Pasang (akan membuat sertifikat dev lokal `CN=Origo Dev`, meminta 1x izin Admin untuk memercayainya di `LocalMachine\TrustedPeople`, lalu mendaftarkan Sparse MSIX):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\register-shell-ext-dev.ps1 -Profile debug
```

Copot registrasi dev:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\unregister-shell-ext-dev.ps1
```

> **Catatan distribusi publik:** Windows mewajibkan Sparse MSIX ditandatangani sertifikat code-signing yang dipercaya. Untuk pemakaian lokal di mesin pengembang, sertifikat dev `CN=Origo Dev` di atas sudah cukup. Untuk rilis publik ke pengguna umum, paket `.msix` perlu ditandatangani dengan sertifikat code-signing terpercaya (mis. Azure Trusted Signing / sertifikat OV/EV).

## Verifikasi

```powershell
. ./scripts/use-build-environment.ps1
cargo test -p origo-shell-ext --locked --offline
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/shell-ext-ui-smoke.ps1 -Profile debug
```
