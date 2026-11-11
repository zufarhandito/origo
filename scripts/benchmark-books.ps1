# Run only generated fixtures. No customer documents or global UI input.
param([ValidateSet('debug','release')][string]$Profile='release')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$directory=Join-Path $root '.research/book-benchmark'
New-Item -ItemType Directory -Force $directory | Out-Null
$exe=Join-Path $root "target/$Profile/pdf-smoke.exe"
$process=Start-Process -FilePath $exe -ArgumentList '--benchmark-books' -WorkingDirectory $root -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $directory 'stdout.log') -RedirectStandardError (Join-Path $directory 'stderr.log')
$null=$process.Handle # Retain the native handle so ExitCode survives process exit on Windows PowerShell.
$peak=0L
while(-not $process.HasExited){
    $process.Refresh()
    $peak=[math]::Max($peak,$process.PeakWorkingSet64)
    Start-Sleep -Milliseconds 100
}
$process.WaitForExit()
Get-Content (Join-Path $directory 'stdout.log')
if($process.ExitCode -ne 0){Get-Content (Join-Path $directory 'stderr.log');throw "Book benchmark failed: $($process.ExitCode)"}
[pscustomobject]@{Profile=$Profile;PeakWorkingSetMiB=[math]::Round($peak/1MB,1);IncludesFixtureGeneration=$true;IncludesGPU=$false} | ConvertTo-Json | Set-Content (Join-Path $directory 'process.json')
Write-Host "PASS benchmark; process peak $([math]::Round($peak/1MB,1)) MiB (includes fixture generation, excludes GPU)"
