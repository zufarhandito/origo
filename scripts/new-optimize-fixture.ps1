# Generated color pattern only; no external image or user document.
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
$root=Split-Path $PSScriptRoot -Parent
$path=Join-Path $root 'tests/fixtures/optimize-photo.jpg'
$bitmap=New-Object Drawing.Bitmap(32,24)
try {
    for($y=0;$y -lt 24;$y++){
        for($x=0;$x -lt 32;$x++){
            $bitmap.SetPixel($x,$y,[Drawing.Color]::FromArgb(($x*8),($y*10),(($x+$y)*4)))
        }
    }
    $bitmap.Save($path,[Drawing.Imaging.ImageFormat]::Jpeg)
} finally {$bitmap.Dispose()}
Write-Host 'Generated synthetic JPEG fixture for lossless Optimize regression.'
