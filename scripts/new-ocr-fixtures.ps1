# Synthetic 300-DPI scan images with known text. No user PDF input.
param([string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path (Split-Path $PSScriptRoot -Parent) '.research/ocr-fixtures'
}
Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$font = New-Object Drawing.Font('Arial', 54, [Drawing.FontStyle]::Regular, [Drawing.GraphicsUnit]::Pixel)
function Write-Scan([string]$Name, [scriptblock]$Draw) {
    $bitmap = New-Object Drawing.Bitmap(2480,3508)
    $bitmap.SetResolution(300,300)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.Clear([Drawing.Color]::White)
        $graphics.TextRenderingHint = [Drawing.Text.TextRenderingHint]::AntiAliasGridFit
        & $Draw $graphics
        $bitmap.Save((Join-Path $OutputDirectory "$Name.png"), [Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}
try {
    $ink = [Drawing.Brushes]::Black
    $sentence = 'Buku ini memuat 123 halaman.'
    Write-Scan indonesia { param($g) $g.DrawString($sentence,$font,$ink,150,240) }
    Write-Scan english { param($g) $g.DrawString('A small library has 456 books.',$font,$ink,150,240) }
    Write-Scan mixed {
        param($g)
        $g.DrawString('Perpustakaan kota',$font,$ink,150,240)
        $g.DrawString('The library is open today.',$font,$ink,150,340)
    }
    Write-Scan blank { param($g) }
    Write-Scan dua-kolom {
        param($g)
        $g.DrawString("Kolom kiri`nmemuat catatan",$font,$ink,150,240)
        $g.DrawString("Kolom kanan`nberisi daftar",$font,$ink,1390,240)
    }
    Write-Scan miring {
        param($g)
        $g.TranslateTransform(150,240)
        $g.RotateTransform(3)
        $g.DrawString($sentence,$font,$ink,0,0)
    }
    Write-Scan buram {
        param($g)
        $image = [Drawing.Image]::FromFile((Join-Path $OutputDirectory 'indonesia.png'))
        $small = New-Object Drawing.Bitmap($image,620,877)
        try { $g.DrawImage($small,0,0,2480,3508) }
        finally { $small.Dispose(); $image.Dispose() }
    }
} finally { $font.Dispose() }
Write-Host "Synthetic OCR fixtures: $OutputDirectory"
