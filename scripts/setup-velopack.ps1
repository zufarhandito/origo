# Installs build tools locally, from pinned upstream archives.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$tools = New-Item -ItemType Directory -Path (Join-Path $root '.tools/velopack-1.2.161') -Force

function Get-VerifiedArchive([string]$Name, [string]$Url, [string]$Algorithm, [string]$Hash) {
    $path = Join-Path $tools.FullName $Name
    if (!(Test-Path -LiteralPath $path)) {
        & curl.exe -fLsS --retry 2 -o "$path.partial" $Url
        if ($LASTEXITCODE -or (Get-FileHash -LiteralPath "$path.partial" -Algorithm $Algorithm).Hash -ne $Hash) {
            throw "Download or checksum failed for $Name"
        }
        Move-Item -LiteralPath "$path.partial" -Destination $path
    }
    if ((Get-FileHash -LiteralPath $path -Algorithm $Algorithm).Hash -ne $Hash) { throw "Checksum failed for $Name" }
    $path
}

$dotnet = Join-Path $tools.FullName 'dotnet/dotnet.exe'
if (!(Test-Path -LiteralPath $dotnet)) {
    $runtime = Get-VerifiedArchive 'dotnet-runtime-8.0.34-win-x64.zip' 'https://builds.dotnet.microsoft.com/dotnet/Runtime/8.0.34/dotnet-runtime-8.0.34-win-x64.zip' 'SHA512' 'd86d165a29dcdb71df23dd150a3fc5db6386fa55da5cb18c0c22889a0dab5071a3a2d852572e6cebfecf12b489870ac76243ae35665a746ed5b1011e7c68979d'
    New-Item -ItemType Directory -Path (Split-Path $dotnet) -Force | Out-Null
    & tar.exe -xf $runtime -C (Split-Path $dotnet)
    if ($LASTEXITCODE) { throw '.NET runtime extraction failed' }
}
$vpk = Join-Path $tools.FullName 'tools/net8.0/any/vpk.dll'
if (!(Test-Path -LiteralPath $vpk)) {
    $archive = Get-VerifiedArchive 'vpk.1.2.161.nupkg' 'https://github.com/velopack/velopack/releases/download/1.2.161/vpk.1.2.161.nupkg' 'SHA256' '2b56ce117f803fc70c103cb423bd040e395e40370f6ff6e10818e9ff9c26a323'
    & tar.exe -xf $archive -C $tools.FullName 'tools/net8.0' 'vendor'
    if ($LASTEXITCODE) { throw 'Velopack extraction failed' }
}
if ((Get-Item -LiteralPath $vpk).VersionInfo.ProductVersion -notmatch '^1\.2\.161(?:\+|$)') { throw 'Velopack version check failed' }
& $dotnet $vpk --skip-updates pack --help | Out-Null
if ($LASTEXITCODE) { throw 'Velopack command could not run' }
Write-Host 'Velopack 1.2.161 ready'
