$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$manifest = Get-Content -Raw (Join-Path $root 'sidecars.lock.json') | ConvertFrom-Json
$base = Join-Path $root 'sidecars'
New-Item -ItemType Directory -Force $base | Out-Null
foreach ($name in @('qpdf', 'pdfium')) {
    $entry = $manifest.$name
    $extension = if ($name -eq 'qpdf') { 'zip' } else { 'tgz' }
    $archive = Join-Path $base "$name-$($entry.version).$extension"
    if (-not (Test-Path -LiteralPath $archive)) {
        Invoke-WebRequest -UseBasicParsing -Uri $entry.url -OutFile $archive
    }
    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant()
    if ($actual -ne $entry.sha256) { throw "Checksum mismatch for $name. Remove the archive and retry." }
    $destination = Join-Path $base $name
    if (Test-Path -LiteralPath $destination) { throw "$name directory exists. Use a fresh sidecars directory for a verified install." }
    $stage = Join-Path $base ([Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory $stage | Out-Null
    if ($name -eq 'qpdf') {
        Expand-Archive -LiteralPath $archive -DestinationPath $stage
        $binary = @(Get-ChildItem -LiteralPath $stage -Recurse -Filter qpdf.exe)
        if ($binary.Count -ne 1) { throw 'Unexpected QPDF archive layout' }
        $content = $binary[0].Directory.Parent.FullName
    } else {
        & tar -xf $archive -C $stage
        if ($LASTEXITCODE -ne 0) { throw 'PDFium archive extraction failed' }
        $content = $stage
    }
    # Preserve every DLL, notice and license from the verified distribution.
    Copy-Item -LiteralPath $content -Destination $destination -Recurse
    Write-Host "Verified and extracted $name $($entry.version)"
}
if (-not (Test-Path -LiteralPath (Join-Path $base 'qpdf/bin/qpdf.exe'))) { throw 'Missing QPDF executable' }
if (-not (Test-Path -LiteralPath (Join-Path $base 'pdfium/bin/pdfium.dll'))) { throw 'Missing PDFium library' }
& (Join-Path $base 'qpdf/bin/qpdf.exe') --version
if ($LASTEXITCODE -ne 0) { throw 'QPDF runtime failed; check MSVC runtime prerequisites' }

