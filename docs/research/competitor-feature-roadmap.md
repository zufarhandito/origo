# Riset Kompetitor & Roadmap Fitur Origo (Tier 1–3)

Tanggal: 26 November 2026  
Kompetitor acuan: **Adobe Acrobat Standard/Pro**, **PDF-XChange Editor**, **Foxit PDF Editor**, **PDFgear**.

## Ringkasan Posisi Origo

Origo sudah memiliki fondasi pembaca & utilitas harian offline yang kuat:
multi-tab cepat, buka PDF berpassword di viewer, Find, Outline, Print, Split View,
Organizer (merge/split/extract/rotate/reorder), Highlight + Pen + Tanda Tangan visual,
OCR lokal + batch, Optimize lossless, Kompresi PDF Pintar (DPI + Grayscale),
Nomor Halaman, Watermark, Enkripsi AES-256 & Buka Kunci, PDF ↔ PNG/JPEG, serta
ekstensi klik kanan Windows 11 Explorer.

Dokumen ini menyimpan urutan implementasi **Tier 1** (dikerjakan berurutan 1 → 5)
serta daftar backlog **Tier 2** dan **Tier 3** untuk fase berikutnya setelah Tier 1 selesai.

---

## Tier 1 — Prioritas Aktif (Urutan 1 → 5)

| Prioritas | Fitur | Lingkup Inti | Status |
|---|---|---|---|
| **1** | **Fill & Sign + Anotasi Lengkap** | Text Box / Typewriter (ketik bebas + ukuran/warna font), stamp cepat `✓` / `✗` / `●` / tanggal, Sticky Notes (catatan tempel), Shape (persegi, elips, garis, panah), Underline & Strikethrough pada teks; full Undo/Redo, pilih/edit/hapus, ekspor PDF standar + roundtrip Save As. | ✅ Done |
| **2** | **Sensor Permanen (True Redaction) + Sanitize Metadata** | Tandai area sensor pada halaman/rentang, terapkan redaction secara permanen (menghancurkan teks & grafis asli di bawah kotak sensor sehingga tidak bisa di-copy/diekstrak ulang) + opsi bersihkan seluruh metadata Info/XMP tersembunyi. | ✅ Done |
| **3** | **Crop Halaman & Standardisasi Ukuran Kertas** | Potong margin halaman (preset / custom mm/pt, halaman aktif / rentang / semua) serta standardisasi ukuran kertas (A4, Letter, F4, Legal) dengan fit proporsional. | ✅ Done |
| **4** | **Flatten PDF & Editor Properti/Metadata PDF** | Flatten anotasi/TTD/form menjadi konten halaman statis (tidak bisa digeser/dihapus penerima) + dialog lihat/edit/hapus Properti & Metadata PDF (Title, Author, Subject, Keywords, Creator, Producer). | ⏳ In Progress |
| **5** | **Ekspor Cepat Tambahan: PDF → JPEG, PDF → TXT, & Ekstrak Gambar Asli** | Ekspor halaman ke JPEG (DPI + kualitas), ekspor teks satu dokumen ke UTF-8 TXT sesuai urutan page plan, dan ekstrak gambar embedded asli dari PDF ke folder tujuan. | ⬜ Queued |

---

## Tier 2 — Backlog Fase Berikutnya (Power-User & Kantor)

*Disimpan untuk dikerjakan setelah seluruh Tier 1 selesai dan disetujui pemilik.*

1. **Isi Form PDF Interaktif (AcroForm)**
   - Deteksi & isi field bawaan PDF (text input, checkbox, radio button, dropdown/listbox) langsung di viewer melalui API form PDFium, simpan ke file baru + opsi flatten form.
2. **Compare PDF Otomatis (Visual & Text Diff)**
   - Peningkatan dari fitur *Bandingkan* (Split View): deteksi otomatis perubahan teks dan perbedaan visual antar dua versi dokumen PDF dengan daftar loncatan perubahan.
3. **Editor Bookmark / Daftar Isi (Outline Editor)**
   - Tambah, ubah judul, atur hierarki (indent/outdent), arahkan ke halaman tujuan, dan hapus bookmark langsung dari panel Daftar Isi lalu simpan ke PDF output.
4. **Header & Footer Lengkap + Bates Numbering**
   - Perluasan fitur Nomor Halaman: 6 zona (kiri/tengah/kanan atas & bawah), variabel dinamis (tanggal, nama file, `Halaman X dari Y`), serta penomoran Bates berurutan lintas halaman/dokumen untuk legal & arsip.
5. **Konversi Office (DOCX / XLSX / PPTX ↔ PDF)**
   - Evaluasi engine lokal offline (mis. headless LibreOffice untuk Office → PDF dan pipeline rekonstruksi layout untuk PDF → DOCX) sesuai gate di [conversion-roadmap.md](../conversion-roadmap.md).

---

## Tier 3 — Backlog Jangka Panjang (Enterprise & Lanjutan)

*Disimpan untuk tahap pengembangan lanjutan setelah Tier 2.*

1. **Edit Teks & Gambar Asli di Dalam PDF (Direct Content Editing)**
   - Mengubah teks eksisting (perbaikan typo, reflow paragraf, font fallback/matching) serta memindahkan/mengganti gambar bawaan langsung di objek konten PDF.
2. **Tanda Tangan Digital Kriptografi (`.p12` / `.pfx` / PAdES)**
   - Penandatanganan dokumen berbasis sertifikat digital lokal, timestamp opsional, serta panel verifikasi integritas & deteksi modifikasi pasca-TTD.
3. **AI Assistant Privasi-First (BYOK / LLM Lokal)**
   - Model ala PDF-XChange: ringkas dokumen, tanya-jawab konteks PDF, dan ekstraksi poin penting menggunakan API key pengguna sendiri (OpenAI/Anthropic/Google) atau model lokal (Ollama) tanpa wajib cloud/langganan.
