# Multi-tab hardening dan profiling memori

Follow-up implementasi 16 November 2026. Cakupan yang diminta: hardening multi-tab,
profiling sumber daya delapan dokumen besar, dan perbaikan nama PDF/tombol tutup
yang sebelumnya terlihat terpisah. Tidak menambah converter atau format ekspor.

## Perilaku yang diperbaiki

- Strip memakai GPUI Kit `TabBar`/`Tab` variant folder, dengan tombol × sebagai
  suffix di dalam tab. Nama dan close berbagi permukaan/tab state; close tetap
  aksi tersendiri yang menghentikan bubbling agar tidak ikut memilih tab.
- Daftar tab adalah satu tombol outline dengan label dan caret, membuka daftar
  path lengkap. Label panjang dipotong tanpa mendorong × keluar dari tab.
- Close pada tab clean yang tidak aktif tidak mengganti tab aktif. Close dirty
  tetap memakai prompt dan mengembalikan konteks sebelumnya bila dibatalkan.
- Copy seleksi yang masih ada tetapi teksnya sudah keluar dari cache memuat ulang
  halaman yang diperlukan secara bertahap, termasuk halaman offscreen. Buffer
  hasil dibatasi **1 MiB UTF-8 termasuk pemisah halaman**; tidak menahan semua
  TextPage sekaligus. Clipboard ditulis setelah hasil lengkap siap.
- Job copy dibatalkan ketika tab menjadi nonaktif, seleksi berganti/dihapus,
  workspace berubah, atau source/token tidak lagi cocok. Tidak menulis clipboard
  dari completion lama ke konteks tab baru.

## Bukti yang diuji

`scripts/harden-tabs-ui-smoke.ps1` memakai fixture/jendela sendiri dan memulihkan
clipboard. Pemeriksaan mencakup containment geometris × dalam aksesibilitas TabItem,
trigger Daftar tab, close tab nonaktif, copy seleksi offscreen setelah switch,
switch/close cepat, source berubah pada satu tab lalu reopen tanpa merusak tab lain,
receipt Save As menuju tab asal saat tab lain aktif, serta cancellation ekspor.
Race tambahan: close tab dirty menunggu slot ekspor tab lain, lalu cancellation job
membatalkan close batch tanpa membuang dokumen. Lihat [testing](testing.md) untuk
run/evidence yang benar-benar lulus.

## Diagnostics

File `--metrics` tetap hanya berisi angka, boolean, ordered source IDs dan operation
states; tanpa path atau isi PDF. Proyeksi aktif dipertahankan untuk suite lama.
`sessions` dan total window mencakup tab nonaktif, job latar dan empty state.
Penulisan berjalan off-thread, memakai shared sequence/lock, dengan snapshot berkala
500 ms saat diagnostics diminta; tidak melakukan file I/O di render callback.

- `totalPageCacheBytes`, `totalThumbnailCacheBytes`, `totalTextCacheBytes` mengukur
  seluruh session. Default tetap 128/16/16 MiB, bukan dikalikan delapan.
- `inactiveCacheBytes`/`inactiveGpuImages` harus nol setelah switch; copy buffer
  mempunyai total terpisah. `totalGpuImages` adalah jumlah residency yang dilacak,
  bukan pengukuran alokasi VRAM driver.
- Counter GPU release berlaku sepanjang umur jendela, termasuk tab yang ditutup.
- `nativeActiveDocuments` mengukur retained document di shared PDFium worker:
  maksimal satu; menjadi nol setelah idle timeout 15 detik. Ini bukan byte size native heap.

## Protokol profil yang dapat diulang

```powershell
. ./scripts/use-build-environment.ps1
./scripts/benchmark-books.ps1 # bila generated book fixtures belum ada
./scripts/harden-tabs-ui-smoke.ps1 -Profile release
./scripts/profile-tabs.ps1 -Profile release
./scripts/profile-tabs.ps1 -Profile release -HighZoom
```

Profil default menjalankan **3 putaran**, masing-masing membuka 8 PDF 1000 halaman
(4 vector, 4 simulated scan), dua sweep semua tab, lalu menutup semuanya dan menunggu
native session idle. File tetap sama sepanjang putaran agar perubahan mencerminkan
open/switch/close, bukan pertumbuhan corpus. Hash sumber diperiksa setelah selesai.

`samples.csv` mencatat private bytes, working set/peak, handle/thread/GDI/User counts,
Windows GPU process-memory dedicated/shared, serta cache/native counters. Counter
GPU difilter ke PID fixture sendiri; data yang tidak tersedia diberi status unavailable,
bukan dianggap nol. `summary.json` memuat baseline, warmed idle endpoints dan maxima.

Toleransi acceptance untuk corpus ini: endpoint putaran terakhir dibanding warmed
endpoint pertama tidak bertambah lebih dari **64 MiB** private/GPU memory atau
**32 handles**; cache/inactive/native invariants diperiksa pada setiap sample.
Allocator/driver pool tidak dituntut kembali nol. Ambang ini adalah guard regresi
untuk workload terukur, bukan janji batas RAM universal atau pembuktian tidak ada leak.

Private bytes mencakup Rust, native engine, framework dan allocator; tidak dipisahkan
sebagai PDFium-only bytes. GPU dedicated/shared diukur terpisah dan tidak dijumlahkan
ke process RAM sebagai satu total. Max private bytes adalah sampled maximum; peak
working set berasal dari counter proses. Generated scans tidak mewakili semua buku nyata.
Run memakai diagnostics `--metrics`; overhead introspeksi tersebut ikut terukur.
Opsi `-HighZoom` menambahkan zoom 400% lalu fit kembali pada satu vector dan satu
simulated scan di setiap putaran, sementara delapan dokumen tetap terbuka.

## Hasil release yang diukur

Run `.research/tabs-memory-27c7c4d8de0d4a1dab31cbfe93da46cb` lulus tiga putaran,
55 sample, 24 pembukaan/penutupan buku dan 48 perpindahan tab setelah startup fixture.
Corpus: empat vector dan empat simulated-scan, masing-masing 1000 halaman.

| Metrik | Hasil |
|---|---:|
| Peak process working set | 209,53 MiB |
| Sampled maximum private bytes | 162,81 MiB |
| Maximum GPU shared / dedicated | 70,50 / 0 MiB |
| Idle private setelah putaran 1 → 2 → 3 | 97,42 → 97,56 → 97,86 MiB |
| Private growth setelah warm-up | +0,438 MiB |
| GPU growth setelah warm-up | −0,035 MiB |
| Idle handles putaran 1 → 2 → 3 | 785 → 786 → 792 |
| GDI / User objects pada ketiga warmed endpoints | 77 / 37 |
| Maximum tracked GPU images | 5 |
| Sampled maximum page cache | 13,88 MiB |

Pada ketiga empty-idle endpoints, cache CPU, tracked GPU images dan retained native
documents semuanya nol; GPU-release error nol. GPU driver pool tetap sekitar
55,8 MiB dan allocator/framework tetap mempunyai baseline, sehingga tidak diklaim
seluruh RAM/VRAM kembali nol. Pengukuran ini tidak menunjukkan pertumbuhan berlanjut
di workload tiga putaran tersebut; bukan pembuktian tidak ada leak pada semua PDF.

Run tambahan `.research/tabs-memory-723a63a16ec146108b0826cc2c276314` dengan
`-HighZoom` juga lulus tiga putaran / 61 sample:

| Metrik, termasuk fase zoom 400% | Hasil |
|---|---:|
| Peak process working set | 509,99 MiB |
| Sampled maximum private bytes | 410,25 MiB |
| Maximum GPU shared / dedicated | 185,26 / 0 MiB |
| Warmed idle private putaran 1 → 2 → 3 | 165,58 → 165,98 → 166,45 MiB |
| Private growth setelah warm-up | +0,875 MiB |
| GPU growth setelah warm-up | 0 MiB |
| Sampled maximum page cache | 120,31 MiB |
| Idle handles putaran 1 → 2 → 3 | 744 → 758 → 764 |

Cache tab nonaktif, GPU-release errors dan empty-idle retained native documents
tetap nol. GPU pool setelah high zoom menetap sekitar 124,34 MiB; alokasi driver
ini tidak diklaim hilang saat tracked residency nol. Handle counts masih bergerak
dalam guard 32 handle pada tiga putaran; pengamatan ini tidak membuktikan stabilitas
handle untuk sesi tanpa batas. Profil durasi/corpus lebih panjang tetap pekerjaan
penerimaan rilis, bukan alasan menaikkan budget cache per tab.

Physical mixed-monitor DPI, trackpad, broader real-document corpus, clean-machine
portable dan isolasi native hostile-document tetap release gates terpisah.
