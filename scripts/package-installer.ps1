param(
    [Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+$')][string[]]$Version,
    [string]$ExecutablePath,
    [string]$PreparedPayload,
    [string]$OutputDirectory,
    [string]$UpdateFeed,
    [ValidateSet('BestSpeed', 'BestSize', 'None')][string]$DeltaMode = 'BestSpeed',
    [string]$ReleaseNotes,
    [switch]$Fixture
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$appId = if ($Fixture) { 'Origo.Preview.Fixture' } else { 'Origo.Preview' }
$title = if ($Fixture) { 'Origo Installer Fixture' } else { 'Origo Preview' }
if (!$OutputDirectory) { $OutputDirectory = Join-Path $root 'dist/origo-preview' }
$output = (New-Item -ItemType Directory -Path $OutputDirectory -Force).FullName
if (!$UpdateFeed) { $UpdateFeed = $output }
$UpdateFeed = $UpdateFeed.Trim()
if ($UpdateFeed.StartsWith('https://')) {
    $rest = $UpdateFeed.Substring('https://'.Length).TrimEnd('/')
    $authority = ($rest -split '/')[0]
    if (!$rest -or $rest -match '\s' -or !$authority -or $authority.Contains('@')) {
        throw 'HTTPS update feed URL is invalid or contains embedded credentials'
    }
} elseif (![IO.Path]::IsPathRooted($UpdateFeed)) {
    throw 'Update feed must be an absolute directory or HTTPS URL'
}
if ($ExecutablePath) { $ExecutablePath = (Resolve-Path -LiteralPath $ExecutablePath).Path }
if ($ReleaseNotes) { $ReleaseNotes = (Resolve-Path -LiteralPath $ReleaseNotes).Path }
& (Join-Path $PSScriptRoot 'setup-velopack.ps1') | Out-Host
$ownsStage = $false
if ($PreparedPayload) {
    $payload = (Resolve-Path -LiteralPath $PreparedPayload).Path
    if ((Get-Content -Raw -LiteralPath (Join-Path $payload 'origo-update-feed.txt')).Trim() -ne $UpdateFeed) { throw 'The prepared payload points to a different feed' }
    & (Join-Path $PSScriptRoot 'verify-portable.ps1') -Directory $payload -VerifyOnly | Out-Host
    if ($ExecutablePath -and (Get-FileHash -LiteralPath (Join-Path $payload 'pdf-studio.exe')).Hash -ne (Get-FileHash -LiteralPath $ExecutablePath).Hash) { throw 'The prepared payload contains a different executable' }
} else {
    $payload = & (Join-Path $PSScriptRoot 'package-portable.ps1') -ExecutablePath $ExecutablePath -StageOnly -UpdateFeed $UpdateFeed
    $ownsStage = $true
}
$tools = Join-Path $root '.tools/velopack-1.2.161'
# Velopack reuses a scratch directory for commands under the same Windows user.
$packLock = New-Object Threading.Mutex($false, 'Local\Origo-Velopack-Package')
$locked = $false
try {
    try { $locked = $packLock.WaitOne() } catch [Threading.AbandonedMutexException] { $locked = $true }
    foreach ($number in $Version) {
        if (Test-Path -LiteralPath (Join-Path $output "$appId-$number-preview-full.nupkg")) {
            throw "Version $number is already packaged. Choose a higher version."
        }
        $packArgs = @(
            (Join-Path $tools 'tools/net8.0/any/vpk.dll'), '--skip-updates', 'pack',
            '--packId', $appId, '--packTitle', $title, '--packAuthors', 'Origo', '--packVersion', $number,
            '--channel', 'preview', '--runtime', 'win-x64', '--shortcuts', 'StartMenuRoot',
            '--delta', $DeltaMode, '--noPortable',
            '--packDir', $payload, '--mainExe', 'pdf-studio.exe', '--outputDir', $output,
            '--icon', (Join-Path $root 'apps/desktop/resources/branding/origo.ico')
        )
        if ($ReleaseNotes) { $packArgs += @('--releaseNotes', $ReleaseNotes) }
        & (Join-Path $tools 'dotnet/dotnet.exe') @packArgs | Out-Host
        if ($LASTEXITCODE) { throw 'Velopack packaging failed' }
        $installer = Get-ChildItem -LiteralPath $output -File -Filter '*Setup.exe' | Select-Object -First 1
        if (!$installer) { throw 'Windows installer is missing' }
        if ($Version.Count -eq 1) {
            Write-Output $installer.FullName
        } else {
            $snapshot = Join-Path $output "$appId-Setup-$number.exe"
            Copy-Item -LiteralPath $installer.FullName -Destination $snapshot
            Write-Output $snapshot
        }
    }
} finally {
    if ($locked) { $packLock.ReleaseMutex() }
    $packLock.Dispose()
    if ($ownsStage -and $payload) {
        Remove-Item -LiteralPath $payload -Recurse -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath ($payload + '-verified-inputs') -Recurse -Force -ErrorAction SilentlyContinue
    }
}
