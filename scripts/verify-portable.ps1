[CmdletBinding(DefaultParameterSetName='Zip')]
param(
    [Parameter(Mandatory=$true,ParameterSetName='Zip')][string]$ZipPath,
    [Parameter(Mandatory=$true,ParameterSetName='Directory')][string]$Directory,
    [switch]$VerifyOnly
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
. (Join-Path $PSScriptRoot 'distribution-support.ps1')
if ($PSCmdlet.ParameterSetName -eq 'Zip') {
    $ZipPath = (Resolve-Path -LiteralPath $ZipPath).Path
    $expectedHash = (Get-Content -Raw -LiteralPath ($ZipPath + '.sha256')).Split(' ')[0].Trim()
    if ((Get-FileHash -LiteralPath $ZipPath).Hash.ToLowerInvariant() -ne $expectedHash) { throw 'ZIP SHA-256 mismatch' }
    $extract = Join-Path $root ('.research/portable-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $extract | Out-Null
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::OpenRead($ZipPath)
    try {
        $seen = New-Object 'Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
        foreach ($entry in $archive.Entries) {
            $target = [IO.Path]::GetFullPath((Join-Path $extract $entry.FullName))
            if (-not $target.StartsWith(($extract + '\'),[StringComparison]::OrdinalIgnoreCase) -or -not $seen.Add($target)) { throw 'Unsafe or duplicate ZIP entry' }
        }
    } finally { $archive.Dispose() }
    Expand-Archive -LiteralPath $ZipPath -DestinationPath $extract
    $folders = @(Get-ChildItem -LiteralPath $extract -Directory)
    if ($folders.Count -ne 1 -or @(Get-ChildItem -LiteralPath $extract -File).Count) { throw 'Unexpected ZIP root' }
    $Directory = Join-Path $extract ('PDF Studio ' + [char]0x65E5 + [char]0x672C + ' & test')
    $resolvedExtract = [IO.Path]::GetFullPath($extract).TrimEnd('\') + '\'
    if (-not $folders[0].FullName.StartsWith($resolvedExtract,[StringComparison]::OrdinalIgnoreCase) -or -not $Directory.StartsWith($resolvedExtract,[StringComparison]::OrdinalIgnoreCase)) { throw 'Relocation escaped extraction root' }
    Move-Item -LiteralPath $folders[0].FullName -Destination $Directory
}
$Directory = (Resolve-Path -LiteralPath $Directory).Path.TrimEnd('\')
$manifest = Get-Content -Raw -LiteralPath (Join-Path $Directory 'files.sha256.json') | ConvertFrom-Json
$actualFiles = @(Get-DistributionFiles $Directory | Where-Object Path -ne 'files.sha256.json')
if ($actualFiles.Count -ne $manifest.Count) { throw 'Package file count mismatch' }
$lookup = @{}
foreach ($file in $manifest) {
    if ($lookup.ContainsKey($file.Path) -or $file.Path -eq 'files.sha256.json' -or $file.Path.Contains('..') -or $file.Path.Contains(':') -or [IO.Path]::IsPathRooted($file.Path)) { throw 'Invalid file manifest path' }
    $lookup[$file.Path] = $file
}
foreach ($file in $actualFiles) {
    $expected = $lookup[$file.Path]
    if (-not $expected -or $file.Bytes -ne $expected.Bytes -or $file.Sha256 -ne $expected.Sha256) { throw "Package integrity mismatch: $($file.Path)" }
}
foreach ($required in @('pdf-studio.exe','sidecars/qpdf/bin/qpdf.exe','sidecars/pdfium/bin/pdfium.dll','vcruntime140.dll','LICENSES/dependencies/cargo-inventory.json','build-info.json','runtime-imports.json')) {
    if (-not $lookup.ContainsKey($required)) { throw "Required package file absent: $required" }
}
$imports = @(Get-DistributionImports $Directory)
$storedImports = Get-Content -Raw -LiteralPath (Join-Path $Directory 'runtime-imports.json') | ConvertFrom-Json
if (($imports | ConvertTo-Json -Compress) -ne ($storedImports | ConvertTo-Json -Compress)) { throw 'Runtime import evidence mismatch' }
if (-not $VerifyOnly) {
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName = Join-Path $Directory 'sidecars/qpdf/bin/qpdf.exe'
    $start.Arguments = '--version'; $start.WorkingDirectory = $Directory
    $start.UseShellExecute = $false; $start.CreateNoWindow = $true; $start.RedirectStandardOutput = $true
    $start.EnvironmentVariables['PATH'] = "$env:SystemRoot\System32;$env:SystemRoot"
    $process = [Diagnostics.Process]::Start($start)
    try {
        if (-not $process.WaitForExit(10000)) { $process.Kill(); throw 'Owned QPDF version probe timeout' }
        $version = $process.StandardOutput.ReadToEnd()
        $lock = Get-Content -Raw -LiteralPath (Join-Path $Directory 'sidecars.lock.json') | ConvertFrom-Json
        if ($process.ExitCode -ne 0 -or -not $version.Contains('qpdf version ' + $lock.qpdf.version)) { throw 'Packaged QPDF version mismatch' }
        Write-Host 'PASS packaged pinned QPDF version under restricted PATH'
    } finally { $process.Dispose() }
}
Write-Host "PASS package integrity ($($manifest.Count) files) and x64 import closure: $Directory"
Write-Output $Directory
