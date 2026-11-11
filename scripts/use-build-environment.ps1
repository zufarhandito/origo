# Dot-source in PowerShell. All project build caches and temporary files stay on D:.
$root = Split-Path $PSScriptRoot -Parent
. (Join-Path $PSScriptRoot 'use-local-rust.ps1')
$temporary = Join-Path $root '.tools/build-temp'
New-Item -ItemType Directory -Force $temporary | Out-Null
$env:TEMP = $temporary
$env:TMP = $temporary
$env:CARGO_TARGET_DIR = Join-Path $root 'target'
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '4' }
$vs = Join-Path $root '.tools/vs2022'
$launcher = Join-Path $vs 'Common7/Tools/Launch-VsDevShell.ps1'
if (-not (Test-Path -LiteralPath $launcher)) {
    throw 'Project-local MSVC Build Tools installation is unavailable.'
}
& $launcher -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
if (-not (Get-Command link.exe -ErrorAction SilentlyContinue)) {
    throw 'MSVC linker is unavailable after environment setup.'
}
