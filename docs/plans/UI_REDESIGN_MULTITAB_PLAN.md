# Rencana redesain UI desktop dan multi-tab

Tanggal: 16 November 2026. **Status: rancangan disetujui dan diimplementasikan.**
Default yang disetujui: panel alat muncul saat dipakai, maksimal 8 tab termasuk
loading. Default implementasi: window minimum 860 × 500, drawer di bawah 1100 px.
Perilaku aktual ada di [multi-tab](../multitab.md); hasil verifikasi aktual ada
di [testing](../testing.md). Bagian di bawah mempertahankan diskusi sebelum implementasi.
Pengguna meminta “plan dulu broo. jangan langsung eksekusi”, kemudian dokumentasi
handover. Dokumen ini mencatat rekomendasi untuk ditinjau, bukan izin implementasi.
Keadaan aplikasi, verifikasi dan Git ada di [handoff aktif](../handoff/PROJECT_HANDOFF.md).

## 1. Masalah dan prioritas

UI sekarang mencampur mode Baca/Atur halaman/Konversi, tindakan file dan tema dalam
header/menu Lainnya. Kontrol baca tersebar, sementara Konversi mendapat tempat utama
meskipun masih placeholder. Sulit memperkirakan lokasi fungsi. Open mengganti satu
workspace; pengguna belum bisa bergantian mengerjakan beberapa PDF.

Rekomendasi: **P1 untuk pengelompokan fungsi/navigasi sebelum fitur besar atau beta;
P2 untuk polesan visual.** Fitur tambahan akan menambah utang navigasi. Ambil pola
menu, tab dan panel kontekstual dari referensi Acrobat, tanpa menyalin seluruh alatnya.
Tab perlu dirancang bersama shell karena mengubah state dan lifecycle dokumen;
pekerjaannya lebih besar daripada menambahkan deretan tombol.

## 2. Struktur jendela

```text
Title bar: PDF Studio + window controls
Menu: Berkas | Edit | Tampilan | Alat | Bantuan
Tab: [ A.pdf • × ] [ B.pdf × ] [ + ] [ daftar tab ]
Toolbar tetap: Open / Save As | Undo / Redo | halaman | zoom / fit | panel alat
┌─────────────────┬──────────────────────────────┬─────────────────────┐
│ Thumbnail       │ Dokumen aktif                │ Alat / opsi konteks │
│ navigasi        │ scroll dan canvas            │                     │
└─────────────────┴──────────────────────────────┴─────────────────────┘
Status: halaman, sumber, pekerjaan dan keterangan singkat
```

| Menu | Fungsi |
|---|---|
| Berkas | Buka PDF, Simpan sebagai, Tutup tab, Keluar |
| Edit | Undo, Redo, Salin, seleksi sesuai konteks |
| Tampilan | Thumbnail, panel alat, navigasi halaman, zoom/fit, tema |
| Alat | Atur halaman, Impor/gabung halaman, Ekstrak, Split, Optimize |
| Bantuan | Daftar shortcut, Tentang |

Menu menjadi katalog stabil; toolbar hanya shortcut aksi rutin dengan label/tooltip
dan shortcut konsisten. Baca adalah keadaan default. Atur halaman membuka konteks
alat di panel kanan: seleksi, reorder, rotate/delete, impor dan opsi output. Jangan
menambah toolbar horizontal baru saat berganti tugas. Save As tetap aksi file.
“Gabung” berarti impor ke workspace aktif, bukan janji workflow merge baru.

Konversi yang belum berfungsi dikeluarkan dari navigasi utama; bila ditampilkan,
berupa informasi “Akan datang”. Tema berada di Tampilan. Menu Lainnya tidak menjadi
penampung fungsi yang tidak berkaitan. Operasi memakai engine yang sudah ada.

Usulan: panel alat terbuka pada jendela lebar, menjadi drawer di bawah **1100 px**;
thumbnail/panel dapat ditutup. Nilai ini perlu wireframe dan uji minimum width/DPI,
belum hasil benchmark atau keputusan produk. Urutan focus/Esc drawer dan modal jelas.

## 3. Perilaku tab v1

| Situasi | Perilaku usulan |
|---|---|
| Open / Ctrl+O / tombol + | Picker native menerima beberapa PDF; satu file satu tab |
| Impor PDF | Menambah halaman ke workspace aktif, tidak membuat tab |
| Urutan | Urutan pilihan dipertahankan; tab pertama yang berhasil menjadi aktif |
| Sumber utama sudah terbuka | Canonical path sama memfokuskan tab lama tanpa mereset edit |
| Nama sama, folder berbeda | Pembeda folder dan tooltip path lengkap |
| File hanya diimpor di tab lain | Boleh dibuka tersendiri; deduplikasi hanya sumber utama tab |
| Sebagian file gagal | Tab berhasil dipertahankan; satu ringkasan kegagalan |
| Kapasitas | Usulan maksimal 8 termasuk loading; permintaan melampaui sisa slot ditolak sebelum membuka |
| Overflow bar | Scroll horizontal dan daftar tab; tab aktif dapat ditemukan |
| Tab terakhir ditutup | Empty state dalam jendela yang sama |

Batas **8 tab** masih usulan untuk membatasi sumber daya, bukan hasil profiling.
Error kapasitas memberi alasan/langkah jelas. Pembukaan gagal harus melepas slot.
Canonicalization/path Windows memperhitungkan variasi huruf; kegagalannya ditangani
sebagai error terkontrol, tanpa menghilangkan dokumen lain.

Per tab: workspace/instance, history/dirty revision, seleksi halaman/teks, halaman
aktif, zoom/fit, scroll/pan, mode alat dan state panel dokumen. Switch memulihkan
konteks tersebut. Tema/preferensi umum milik jendela/aplikasi. Dirty ditandai titik;
loading/gagal memiliki state eksplisit. Ctrl+Tab/Ctrl+Shift+Tab berpindah, Ctrl+W
menutup tab, Ctrl+Shift+S tetap Save As. Audit shortcut/focus input/menu/modal dahulu.

Save As mempertahankan sumber dan history workspace; nama tab tetap nama sumber.
Tujuan ekspor tampil pada hasil sukses. Mengganti sumber menjadi output akan mengubah
semantics history/import/saved state dan bukan bagian v1.

## 4. Close, export dan async

- Tab dirty: **Simpan sebagai / Buang / Batal**. Save gagal/batal atau consent
  ditolak mempertahankan tab dan perubahannya.
- Window close: proses tab dirty kiri ke kanan; tutup semua hanya setelah seluruh
  rangkaian disetujui/selesai. Cancel di tengah mempertahankan semua tab; Save yang
  sudah berhasil tetap ada, dan tab terkait sudah clean.
- Kunci Open dan mutasi yang bisa mengubah sasaran saat rangkaian close/save aktif.
  Hanya satu transition berjalan.
- Awalnya satu ekspor per jendela. Tab pemilik menunggu ekspor dan cleanup sebelum
  ditutup. Tab clean lain boleh ditutup; close yang perlu Save As menunggu slot.
- Progress, consent, error dan receipt membawa identitas tab asal; dialog menyebut
  dokumennya. Penyelesaian tidak memaksa switch ke tab asal.
- Hanya full Save As dengan instance/revision cocok boleh clean. Extract/Split/
  Optimize tidak. Callback tab tertutup/generation usang ditolak dengan cleanup benar.

Ini memperluas [unsaved guard](../unsaved-changes.md). Source read-only, snapshot,
preflight, consent, recheck, validasi dan no-overwrite tetap mengikuti
[kebijakan ekspor](../adr/0003-export-preservation-policy.md).

## 5. State dan sumber daya

Saat ini `Viewer` di `apps/desktop/src/main.rs` memegang state jendela dan dokumen.
Pisahkan sebelum menggandakan dokumen; nama bagian berikut konseptual:

| Bagian | Tanggung jawab |
|---|---|
| WindowShell | Menu/tab, active TabId, tema, focus/modal, window transition |
| DocumentSession | Stable TabId, workspace/instance, history, seleksi, viewport, konteks alat |
| RenderService bersama | Worker/session PDFium, penjadwalan, cache seluruh jendela |
| ExportCoordinator | Satu job aktif, origin TabId, consent/progress/receipt, cleanup |

Semua permintaan render/metadata/ekspor membawa TabId dan generation/revision yang
relevan. Cache memerlukan identitas dokumen/sumber dan parameter render, bukan nomor
halaman saja. Callback lama tidak boleh mengubah tab baru di posisi indeks yang sama.

Budget CPU default **seluruh jendela** tetap page 128 MiB, thumbnail 16 MiB, text
16 MiB dengan override yang ada; jangan dikalikan per tab. Tab tidak aktif berhenti
meminta render yang tidak diperlukan, melepas GPU melalui lifecycle GPUI yang benar,
dan boleh kehilangan cache CPU. History/edit/posisi baca tetap disimpan. Batas sumber
1024 halaman/128 MiB per workspace tetap berlaku. Ukur total process memory juga:
budget cache tidak mencakup semua native handles, snapshot dan history.

Pertahankan reuse native session, PDFium global mutex, bounded worker, source-change
rejection dan deferred GPU drop. Renderer terpisah per tab tanpa batas bersama
berisiko memboroskan memory. GPUI Kit 0.9.1 memiliki Tab/TabBar/AppMenuBar; sumber
lokalnya sudah diperiksa, belum diintegrasikan atau diuji pada rancangan ini.

## 6. Tahap setelah mendapat arahan implementasi

1. **Wireframe dan mapping aksi.** Empty, reader, organizer, export aktif, close
   multi-dirty; sempit/lebar, label dan focus. Tinjau default sebelum mengunci desain.
2. **Pisahkan shell/session dengan satu dokumen dahulu.** Stable identity, renderer
   bersama, routing export; reader/organizer/output/unsaved guard tetap lulus regresi.
3. **Tab dan lifecycle.** Open multi-file, deduplikasi, switch/close, dirty guard,
   async origin, cache bersama dan kapasitas. Verifikasi isolasi state.
4. **Layout desktop.** Menu, tab strip, toolbar tetap, panel konteks, empty state,
   drawer; hapus kontrol lama yang sudah digantikan.
5. **Acceptance dan dokumentasi.** Uji lintas tab/native/UI, profiling memory,
   update docs perilaku nyata dan handoff dengan evidence.

Setiap tahap harus dapat direview. Estimasi belum dibuat; refactor state dan routing
async merupakan risiko utama. Jangan menganggap rencana ini sudah selesai atau disetujui.

## 7. Kriteria penerimaan

- Semua aksi aktif ditemukan melalui kelompok yang sesuai; placeholder tidak
  terlihat siap pakai. Keyboard, tooltip dan focus jelas.
- Edit A tidak memengaruhi B; Undo/Redo, dirty, seleksi, halaman, zoom dan scroll
  pulih saat switch. Open dan Impor memiliki perilaku yang jelas berbeda.
- Render/error/consent/receipt menuju tab asal ketika tab lain aktif/ditutup.
  Uji switch cepat, Open berturut-turut, source change dan callback usang.
- Close tab/window: clean/dirty, Save/Discard/Cancel, gagal, consent ditolak,
  ekspor aktif. Cancel di akhir rangkaian mempertahankan semua tab; Save sebelumnya benar.
- Duplicate path, nama sama beda folder, valid/corrupt campuran, partial open,
  kapasitas/loading dan tutup tab terakhir menghasilkan state deterministik.
- Switch berulang tidak menambah cache/tekstur tanpa batas. Ukur native/GPU/process
  memory, dan respons UI selama export.
- Light/dark, keyboard/modal, resize sekitar 860–1600 px serta DPI diuji. Tetapkan
  threshold drawer setelah hasil pengujian.
- Unit untuk isolasi/transition/race dan regresi relevan: fmt/Clippy/build, native
  smoke, viewer/organizer, M3/unsaved, Split dan Optimize UI. Tes harus memeriksa
  perilaku/race, bukan sekadar menggandakan implementasi.

## 8. Scope dan keputusan terbuka

V1 tidak mencakup autosave/session restore, detach/multi-window, drag reorder tab,
side-by-side, converter baru, OCR atau content editing. Keselamatan ekspor tetap.

Review berikutnya: batas 8 tab, drawer 1100 px, lebar minimum reader, label final
dan wireframe. Arah permintaan pengguna adalah pengelompokan desktop dan banyak PDF;
detail solusi di atas masih rekomendasi, belum keputusan implementasi terkunci.
