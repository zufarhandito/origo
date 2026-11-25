$ErrorActionPreference = 'Stop'

$existing = Get-AppxPackage -Name 'Origo.ShellExtension' -ErrorAction SilentlyContinue
if ($existing) {
    $existing | Remove-AppxPackage
    Write-Host 'Removed Origo.ShellExtension sparse package.'
} else {
    Write-Host 'Origo.ShellExtension is not currently registered.'
}

$devBase = Join-Path $env:LOCALAPPDATA 'Origo\ShellExtDev'
if (Test-Path -LiteralPath $devBase) {
    Remove-Item -LiteralPath $devBase -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "Cleaned dev staging directory: $devBase"
}
