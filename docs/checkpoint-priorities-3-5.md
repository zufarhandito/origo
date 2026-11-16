# Checkpoint prioritas 3–5

Baseline sesi: `582cd67` (`feat: tab`), main sama dengan cache origin/main saat
inspeksi. Cache ini bukan verifikasi remote baru; sesi ini tidak melakukan fetch/push.
Commit checkpoint lokal dapat dilihat melalui `git log`.

## Recheck satu per satu

| Prioritas | Hasil | Bukti / batas |
|---|---|---|
| 3 — migrasi M3 | Suite saat ini memakai Open tab, tab/window close dan receipt origin; 25 kasus diperiksa sendiri-sendiri | [Coverage map](m3-ui-coverage.md), `cases.json` run M3 di [testing](testing.md) |
| 4 — perangkat/input/aksesibilitas | Audit otomatis pada display terhubung dan targeted input toolkit; pemeriksaan fisik tidak disamakan dengan otomasi | [Matriks perangkat](device-acceptance.md); hanya satu display aktif 1920 × 1200, DPI aktual 96 teramati |
| 5 — docs/review/checkpoint | Dokumen perilaku/tes diselaraskan, snapshot historis dipertahankan, diff/link/syntax/quality gates diperiksa sebelum commit lokal | [Indeks](README.md), [handoff](handoff/PROJECT_HANDOFF.md), [testing](testing.md) |

## Batas penyelesaian prioritas 4

150/200% fisik, perpindahan monitor berskala berbeda, pinch/scroll trackpad fisik,
modifier shortcuts dari keyboard fisik dan pengalaman Narrator masih membutuhkan
hasil tester/perangkat. Presence HID atau unit DPI math tidak menutup gate tersebut.
Pertanyaan ketersediaan hardware telah diajukan; hasil yang belum diterima tidak
dianggap pass. Prioritas 4 belum dinyatakan selesai penuh.

Command untuk fixture manual siap pakai:

```powershell
./scripts/device-acceptance.ps1 -Profile release -LeaveOpen
```

Checkpoint ini tidak mengerjakan portable ZIP, packaging, converter, OCR, migrasi
milestone lain, atau publikasi remote. Prioritas 6/7 menunggu arahan berikutnya;
gate fisik prioritas 4 tetap terlihat agar tidak terlewati saat rilis.
