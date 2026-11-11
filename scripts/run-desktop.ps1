param([string]$Pdf)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    & cargo build --locked -p pdf-studio
    if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed' }
    $destination = Join-Path $root 'target/debug/sidecars'
    New-Item -ItemType Directory -Force $destination | Out-Null
    foreach ($name in @('qpdf', 'pdfium')) {
        $component = Join-Path $destination $name
        New-Item -ItemType Directory -Force $component | Out-Null
        Get-ChildItem -LiteralPath (Join-Path $root "sidecars/$name") | Copy-Item -Destination $component -Recurse -Force
    }
    $executable = Join-Path $root 'target/debug/pdf-studio.exe'
    if ($Pdf) { & $executable $Pdf } else { & $executable }
    if ($LASTEXITCODE -ne 0) { throw 'Desktop process failed' }
} finally { Pop-Location }
