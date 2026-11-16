param([Parameter(Mandatory=$true)][string]$ZipPath)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'distribution-support.ps1')
$package = & (Join-Path $PSScriptRoot 'verify-portable.ps1') -ZipPath $ZipPath
$package = @($package)[-1]
function Expect-Rejection([scriptblock]$Action,[string]$Reason) {
    $message = $null
    try { & $Action | Out-Null } catch { $message = $_.Exception.Message }
    if (-not $message -or -not $message.Contains($Reason)) { throw "Expected rejection '$Reason', got '$message'" }
    Write-Host "PASS rejection: $Reason"
}
$readme = Join-Path $package 'README.txt'
$original = [IO.File]::ReadAllBytes($readme)
$extra = Join-Path $package 'owned-unexpected-test-file.txt'
try {
    [IO.File]::WriteAllText($extra,'unexpected')
    Expect-Rejection { & (Join-Path $PSScriptRoot 'verify-portable.ps1') -Directory $package -VerifyOnly } 'Package file count mismatch'
} finally { if (Test-Path -LiteralPath $extra) { Remove-Item -LiteralPath $extra } }
try {
    [IO.File]::AppendAllText($readme,'tampered')
    Expect-Rejection { & (Join-Path $PSScriptRoot 'verify-portable.ps1') -Directory $package -VerifyOnly } 'Package integrity mismatch'
} finally { [IO.File]::WriteAllBytes($readme,$original) }
$runtime = Join-Path $package 'vcruntime140.dll'
$backup = $runtime + '.owned-test-backup'
try {
    Move-Item -LiteralPath $runtime -Destination $backup
    Expect-Rejection { Get-DistributionImports $package } 'Unbundled dependency'
    Expect-Rejection { & (Join-Path $PSScriptRoot 'verify-portable.ps1') -Directory $package -VerifyOnly } 'Package integrity mismatch'
} finally { if (Test-Path -LiteralPath $backup) { Move-Item -LiteralPath $backup -Destination $runtime } }
$null = & (Join-Path $PSScriptRoot 'verify-portable.ps1') -Directory $package
$badZip = Join-Path (Split-Path $package) 'owned-checksum-test.zip'
Copy-Item -LiteralPath $ZipPath -Destination $badZip
[IO.File]::WriteAllText(($badZip+'.sha256'),(('0'*64)+'  owned-checksum-test.zip'))
Expect-Rejection { & (Join-Path $PSScriptRoot 'verify-portable.ps1') -ZipPath $badZip -VerifyOnly } 'ZIP SHA-256 mismatch'
Write-Host 'PASS restored package and negative integrity/import tests'
Write-Output $package
