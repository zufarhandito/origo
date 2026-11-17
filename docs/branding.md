# Branding Origo

Nama tampilan aplikasi Windows kini **Origo**. Pemilik menyetujui pemasangan ikon
lingkaran yang dikirim sebagai ikon sementara; desain bisa diganti lagi.

Aset berada di [resources/branding](../apps/desktop/resources/branding/README.md):
PNG sumber asli 233x203 berlatar putih, ICO sembilan ukuran (16/20/24/32/40/48/64/128/256)
dan PNG 32px untuk title bar 16 logical px. Proporsi dipertahankan dengan padding.
Ukuran 256 merupakan upscale; tidak ada SVG/vector tracing atau penghapusan latar.

## Integrasi

`apps/desktop/build.rs` mengompilasi `origo.rc` dengan embed-resource 3.0.12 pada
Windows MSVC. Resource ikon **ID 1** sesuai loader native GPUI pin saat ini, sehingga
Windows memakai aset aplikasi untuk window class/taskbar/Alt+Tab dan executable.
VERSIONINFO menampilkan ProductName/FileDescription Origo. Manifest GPUI tidak diubah.
Title bar custom menampilkan logo dan nama; title native, menu About dan isi About
juga memakai Origo. Aset tertanam, tidak bergantung pada file PNG/ICO eksternal saat run.

Nama executable `pdf-studio.exe`, crate/package, folder state, env var dan nama paket
portable internal dipertahankan agar shortcut/riwayat tetap kompatibel. Rebranding
installer/Store, asosiasi file PDF, wordmark dan identitas visual lengkap belum dibuat.

## Mengganti ikon

Ganti `apps/desktop/resources/branding/origo-source.png`, lalu jalankan
`python scripts/update-origo-icon.py` (Pillow dibutuhkan pada mesin pengembang).
Script memperbarui ICO dan PNG title bar secara deterministik. Rebuild aplikasi,
jalankan `scripts/branding-ui-smoke.ps1` dengan executable terisolasi, lalu periksa
capture terang/gelap. Jangan menutup aplikasi pengguna untuk menimpa executable aktif.

Bukti pemeriksaan ada di [testing](testing.md). Windows dapat menyimpan cache ikon
shortcut, sehingga shortcut lama mungkin memerlukan refresh normal setelah update.
