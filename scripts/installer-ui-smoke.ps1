param([Parameter(Mandatory)][string]$ExecutablePath, [string]$PreparedPayload)
$ErrorActionPreference = 'Stop'
$id = 'Origo.Preview.Fixture'
$installation = Join-Path $env:LOCALAPPDATA $id
$manifestPath = Join-Path $installation 'current/sq.version'
$installed = Join-Path $installation 'current/pdf-studio.exe'
if (Test-Path -LiteralPath $installation) { throw 'Installer fixture already exists. Use a fresh fixture installation.' }
$feed = if ($PreparedPayload) {
    (Get-Content -Raw -LiteralPath (Join-Path $PreparedPayload 'origo-update-feed.txt')).Trim()
} else {
    (New-Item -ItemType Directory -Path (Join-Path (Split-Path $PSScriptRoot -Parent) ('.research/installer-feed-' + [guid]::NewGuid().ToString('N')))).FullName
}
function Assert-Version([string]$Expected) {
    $manifest = [xml](Get-Content -LiteralPath $manifestPath -Raw)
    if ($manifest.package.metadata.id -ne $id -or $manifest.package.metadata.version -ne $Expected) { throw "Expected installed version $Expected" }
}
function Start-Installed([string]$MetricsPath, [switch]$Launcher) {
    $entry = if ($Launcher) { Join-Path $installation 'Origo Installer Fixture.exe' } else { $installed }
    Start-Process -FilePath $entry -ArgumentList ('"'+$a+'" --metrics "'+$MetricsPath+'"') `
        -WorkingDirectory (Split-Path $entry) -WindowStyle Normal -PassThru
}

$setups = & (Join-Path $PSScriptRoot 'package-installer.ps1') -Version @('0.1.0','0.1.1') -Fixture -OutputDirectory $feed -ExecutablePath $ExecutablePath -PreparedPayload $PreparedPayload
$install = Start-Process -FilePath $setups[0] -ArgumentList '--silent' -WindowStyle Hidden -PassThru -Wait
if ($install.ExitCode -ne 0 -or !(Test-Path -LiteralPath $manifestPath)) { throw 'Fixture installation failed' }
Assert-Version '0.1.0'
Write-Host 'PASS installer: version 0.1.0 installed per user'

. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -ExecutablePath $installed -TestName 'installer-ui' -IsolatedState:$false

try {
    $null = Wait-State {param($s) $s.pageCount -gt 0 -and !$s.textLoading -and $s.favoritesReady}
    Invoke 'Bintangi: A.pdf'
    $null = Wait-State {param($s) $s.starredTabIds.Count -eq 1 -and $s.pendingPreferences -eq 0}
    $package = "$id-0.1.1-preview-full.nupkg"
    $download = Join-Path $installation "packages/$package"
    $null = Find 'Unduh'
    $null = Find 'Nanti'
    if (Test-Path -LiteralPath $download) { throw 'Update downloaded before user confirmation' }
    Capture 'update-prompt.png'
    Invoke 'Nanti'
    Start-Sleep -Milliseconds 500
    if (Test-Path -LiteralPath $download) { throw 'Update downloaded after clicking Nanti' }
    Write-Host 'PASS updater: version 0.1.1 prompts before download and Nanti defers cleanly'

    Menu-Action 'Bantuan' 'Periksa pembaruan'
    $null = Find 'Unduh'
    Invoke 'Unduh'
    $limit = (Get-Date).AddSeconds(30)
    while (!(Test-Path -LiteralPath $download -PathType Leaf) -and (Get-Date) -lt $limit) { Start-Sleep -Milliseconds 150 }
    if (!(Test-Path -LiteralPath $download -PathType Leaf)) { throw 'Approved update did not finish downloading' }
    if ((Get-FileHash -LiteralPath $download).Hash -ne (Get-FileHash -LiteralPath (Join-Path $feed $package)).Hash) { throw 'Staged update checksum mismatch' }
    $null = Find 'Origo 0.1.1 siap dipasang. Keluar dan buka lagi untuk memperbarui.'
    Capture 'update-ready.png'
    Assert-Version '0.1.0'
    Write-Host 'PASS updater: version 0.1.1 downloaded after user confirmation'

    Invoke 'Panel alat'; Invoke 'Putar kanan'
    $null = Wait-State {param($s) $s.dirty}
    $duplicate = Start-Installed (Join-Path $directory 'duplicate.json')
    if (!$duplicate.WaitForExit(6000) -or $app.HasExited) { throw 'A second launch interfered with the active viewer' }
    Close-Window
    $null = Find 'Simpan perubahan sebelum melanjutkan?'
    Invoke 'Batal'
    $null = Wait-State {param($s) $s.dirty -and $s.transition -eq 'Idle'}
    Assert-Version '0.1.0'
    Write-Host 'PASS updater: pending update and a second launch preserve unsaved edits'

    $userData = Join-Path $installation 'UserData'
    $favorites = Join-Path $userData 'favorites.json'
    $favoriteHash = (Get-FileHash -LiteralPath $favorites).Hash
    Close-Window
    $null = Find 'Simpan perubahan sebelum melanjutkan?'
    Invoke 'Buang perubahan'
    if (!$app.WaitForExit(8000)) { throw 'Fixture viewer did not close gracefully' }
    $metrics = Join-Path $directory 'metrics-after.json'
    $null = Start-Installed $metrics -Launcher
    $deadline = (Get-Date).AddSeconds(45)
    do {
        Start-Sleep -Milliseconds 200
        $owner = Get-CimInstance Win32_Process -Filter "Name='pdf-studio.exe'" | Where-Object {
            $_.ExecutablePath -eq $installed -and $_.CommandLine.Contains($metrics)
        } | Select-Object -First 1
    } while ((!$owner -or !(Test-Path -LiteralPath $metrics)) -and (Get-Date) -lt $deadline)
    if (!$owner -or !(Test-Path -LiteralPath $metrics)) { throw 'Updated app did not reopen' }
    $app = Get-Process -Id $owner.ProcessId
    Assert-Version '0.1.1'
    $null = Find 'Origo 0.1.1 sekarang aktif.'
    Capture 'update-active.png'
    $null = Wait-State {param($s) $s.pageCount -gt 0 -and $s.starredTabIds.Count -eq 1}
    if ((Get-FileHash -LiteralPath $favorites).Hash -ne $favoriteHash -or (Get-FileHash -LiteralPath $a).Hash -ne $hashA) { throw 'Source or user data failed to survive the update' }
    Write-Host 'PASS updater: version 0.1.1 applied on relaunch and preserved local data'

    Close-Window
    if (!$app.WaitForExit(8000)) { throw 'Updated fixture did not close' }
    Move-Item -LiteralPath (Join-Path $feed 'releases.preview.json') -Destination (Join-Path $feed 'releases.preview.unavailable')
    $metrics = Join-Path $directory 'metrics-no-feed.json'
    $app = Start-Installed $metrics
    $null = Wait-State {param($s) $s.pageCount -gt 0}
    Assert-Version '0.1.1'
    Close-Window
    if (!$app.WaitForExit(8000)) { throw 'Offline fixture did not close' }
    Write-Host 'PASS updater: viewer remains usable when the feed is unavailable'
    Write-Host "Installer evidence: $directory; feed: $feed"
} finally {
    if ($app -and !$app.HasExited -and $app.MainWindowHandle -ne 0) { Close-Window }
}
