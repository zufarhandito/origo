$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$directory = Join-Path $root 'tests/fixtures'
New-Item -ItemType Directory -Force $directory | Out-Null
$target = Join-Path $directory 'two-pages.pdf'
# ASCII only: byte offsets equal string lengths. Entirely synthetic, no external data.
$red = "1 0 0 rg 0 0 300 420 re f`n1 1 1 rg BT /F1 24 Tf 25 200 Td (SOURCE PAGE 1 RED) Tj ET`n"
$blue = "0 0 1 rg 0 0 300 420 re f`n1 1 1 rg BT /F1 24 Tf 25 200 Td (SOURCE PAGE 2 BLUE) Tj ET`n"
$objects = @(
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 420] /Resources << /Font << /F1 5 0 R >> >> /Contents 6 0 R >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 420] /Resources << /Font << /F1 5 0 R >> >> /Contents 7 0 R >>',
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
    "<< /Length $($red.Length) >>`nstream`n${red}endstream",
    "<< /Length $($blue.Length) >>`nstream`n${blue}endstream"
)
$pdf = "%PDF-1.4`n"
$offsets = @(0)
for ($index = 0; $index -lt $objects.Count; $index++) {
    $offsets += $pdf.Length
    $pdf += "$($index + 1) 0 obj`n$($objects[$index])`nendobj`n"
}
$xref = $pdf.Length
$pdf += "xref`n0 8`n0000000000 65535 f `n"
foreach ($offset in $offsets[1..7]) { $pdf += ('{0:D10} 00000 n ' -f $offset) + "`n" }
$pdf += "trailer`n<< /Size 8 /Root 1 0 R >>`nstartxref`n$xref`n%%EOF`n"
[IO.File]::WriteAllBytes($target, [Text.Encoding]::ASCII.GetBytes($pdf))
Write-Host 'Generated synthetic two-page fixture.'

