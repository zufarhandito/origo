# Device, keyboard dan accessibility acceptance

Prioritas 4 membedakan bukti otomatis dari gerakan/persepsi fisik. Satu monitor
aktif 1920 × 1200 dan HID touch pad terdeteksi pada sesi ini. Keberadaan perangkat
tidak membuktikan pengiriman gesture atau perilaku screen reader.

## Pemeriksaan otomatis yang dapat diulang

```powershell
./scripts/device-acceptance.ps1 -Profile release
# Untuk menyiapkan fixture yang tetap terbuka bagi tester fisik:
./scripts/device-acceptance.ps1 -Profile release -LeaveOpen
```

Script hanya membuka PDF sintetis/jendela sendiri, mengidentifikasi PID melalui
executable dan GUID fixture, dan tidak mengubah pengaturan DPI global. `checks.json`,
`displays.json`, metrics dan screenshot disimpan di `.research/device-acceptance-*`.
Status `pending-hardware`/`pending-human` tidak dihitung sebagai lulus.

Pemeriksaannya meliputi:

- Pindahkan window fixture ke setiap display yang benar-benar terhubung, baca
  `GetDpiForWindow`, lalu cocokkan dengan GPUI `deviceScaleMilli` dan viewport.
- Nama/bounds kontrol utama, TabItem dan Button tutup, serta ValuePattern input halaman.
- Targeted HWND Enter, PageUp/PageDown, Tab traversal dan Escape drawer. Ini menguji
  jalur input toolkit; bukan bukti delivery dari keyboard fisik atau modifier keys.
- Sumber tetap utuh dan window otomatis ditutup, kecuali `-LeaveOpen` diminta.

## Checklist fisik yang tetap perlu tester

Fokuskan window fixture **A.pdf** yang dibuat script, bukan dokumen pribadi.
Catat hasil per baris dengan perangkat, DPI aktual, pass/fail dan gejala bila gagal.

| Kasus | Langkah dan kriteria lulus | Status tanpa observasi tester |
|---|---|---|
| 100% / 150% / 200% aktual | Pada setiap skala, teks/thumbnail tajam, label/× tetap satu tab, toolbar tidak overlap, menu dan dialog dapat dibaca/diklik | Belum diverifikasi secara fisik pada semua skala |
| Pindah antar-monitor | Drag fixture bolak-balik antar display berskala berbeda; posisi baca/zoom tetap masuk akal, selection hit-test tepat, resize/fit tidak rusak | Pending perangkat kedua |
| Pinch trackpad | Dua jari pinch/spread di viewport mengubah zoom 50–400%, mempertahankan anchor; scroll biasa tetap scroll dan pan horizontal berfungsi | Pending gesture fisik |
| Modifier shortcuts | Ctrl+O buka tab, Ctrl+Tab/Ctrl+Shift+Tab switch, Ctrl+W close; Ctrl+Shift+S save guard; Ctrl+C copy seleksi | Pending keyboard fisik |
| Editing/focus | Input halaman menerima angka/Enter serta normal editing; Tab/focus ring terlihat; Esc menutup drawer dan modal dengan aman | Otomasi sebagian; delivery fisik pending |
| Screen reader | Dengan Narrator/alat yang dipakai tester, tab aktif/nama dokumen, ×, menu, input halaman dan prompt unsaved dapat dikenali; focus tidak tersesat di belakang modal | Pending mendengarkan assistive technology |

UIAutomation roles/names/focus dan screenshot adalah bukti struktural, bukan pengganti
mendengar announcement atau mencoba hardware. Tes DPI math di unit suite juga bukan
physical mixed-DPI acceptance. Tidak ada skala 150/200% atau pinch yang dinyatakan
lulus hanya karena tersedia API simulasi.

## Penyelesaian dan gate

Prioritas 4 baru penuh setelah baris fisik yang relevan mendapat hasil nyata.
Pekerjaan docs/checkpoint prioritas 5 dapat mencatat pending ini secara eksplisit;
checkpoint lokal bukan sertifikasi portable release atau penutupan gate perangkat.
Run/evidence aktual dicatat di [testing](testing.md).
