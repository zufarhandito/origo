# Prepares (and optionally uploads) an Origo Preview release for GitHub Releases
# with Velopack delta updates. Defaults to local staging only; pass -Upload to
# create a draft GitHub pre-release, and -Upload -Publish to publish immediately.
param(
    [Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version,
    [string]$RepoUrl = 'https://github.com/zufarhandito/origo',
    [string]$ExecutablePath,
    [string]$PreparedPayload,
    [string]$OutputDirectory,
    [ValidateSet('BestSpeed', 'BestSize', 'None')][string]$DeltaMode = 'BestSpeed',
    [string]$ReleaseNotes,
    [string]$Token,
    [switch]$SyncPrevious,
    [switch]$Upload,
    [switch]$Publish
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$RepoUrl = $RepoUrl.Trim().TrimEnd('/')
if ($RepoUrl -notmatch '^https://github\.com/([A-Za-z0-9._-]+)/([A-Za-z0-9._-]+)$' -or $Matches[1] -in @('.', '..') -or $Matches[2] -in @('.', '..') -or $RepoUrl.EndsWith('.git')) {
    throw "RepoUrl must look like 'https://github.com/<owner>/<repo>' (got '$RepoUrl')"
}
if ($Publish -and !$Upload) {
    throw '-Publish requires -Upload'
}
if (!$Token -and $env:GITHUB_TOKEN) {
    $Token = $env:GITHUB_TOKEN
}
if ($Upload -and !$Token) {
    throw 'Pass -Token or set GITHUB_TOKEN before using -Upload'
}
if (!$OutputDirectory) {
    $OutputDirectory = Join-Path $root 'dist/origo-preview-github'
}
$output = (New-Item -ItemType Directory -Path $OutputDirectory -Force).FullName
& (Join-Path $PSScriptRoot 'setup-velopack.ps1') | Out-Host
$tools = Join-Path $root '.tools/velopack-1.2.161'
$dotnet = Join-Path $tools 'dotnet/dotnet.exe'
$vpk = Join-Path $tools 'tools/net8.0/any/vpk.dll'

if ($SyncPrevious) {
    $downloadArgs = @(
        $vpk, '--skip-updates', 'download', 'github',
        '--repoUrl', $RepoUrl, '--channel', 'preview', '--pre',
        '--outputDir', $output
    )
    if ($Token) { $downloadArgs += @('--token', $Token) }
    Write-Host "Syncing latest GitHub preview release from $RepoUrl..."
    & $dotnet @downloadArgs | Out-Host
    if ($LASTEXITCODE) { throw 'Failed to download previous GitHub release (omit -SyncPrevious for the first release)' }
}

$packParams = @{
    Version = @($Version)
    OutputDirectory = $output
    UpdateFeed = $RepoUrl
    DeltaMode = $DeltaMode
}
if ($ExecutablePath) { $packParams['ExecutablePath'] = $ExecutablePath }
if ($PreparedPayload) { $packParams['PreparedPayload'] = $PreparedPayload }
if ($ReleaseNotes) { $packParams['ReleaseNotes'] = $ReleaseNotes }

$installer = & (Join-Path $PSScriptRoot 'package-installer.ps1') @packParams

$fullPath = Join-Path $output "Origo.Preview-$Version-preview-full.nupkg"
$deltaPath = Join-Path $output "Origo.Preview-$Version-preview-delta.nupkg"
$fullBytes = (Get-Item -LiteralPath $fullPath).Length
Write-Host ("Full package:  {0} ({1:N2} MiB)" -f (Split-Path $fullPath -Leaf), ($fullBytes / 1MB))
if (Test-Path -LiteralPath $deltaPath -PathType Leaf) {
    $deltaBytes = (Get-Item -LiteralPath $deltaPath).Length
    $savedPct = (1.0 - ($deltaBytes / [double]$fullBytes)) * 100.0
    Write-Host ("Delta package: {0} ({1:N2} MiB, {2:N1}% smaller)" -f (Split-Path $deltaPath -Leaf), ($deltaBytes / 1MB), $savedPct)
} else {
    Write-Host 'Delta package: none (first release in output directory or -DeltaMode None)'
}
$installerHash = (Get-FileHash -LiteralPath $installer).Hash
Write-Host "Installer:     $installer (SHA-256: $installerHash)"

if (!$Upload) {
    Write-Host "Prepared locally in $output for feed $RepoUrl. Pass -Upload (and optionally -Publish) when ready to upload to GitHub Releases."
    Write-Output $installer
    return
}

$uploadArgs = @(
    $vpk, '--skip-updates', 'upload', 'github',
    '--repoUrl', $RepoUrl, '--channel', 'preview', '--pre',
    '--outputDir', $output, '--token', $Token,
    '--tag', "v$Version-preview",
    '--releaseName', "Origo Preview $Version"
)
if ($Publish) { $uploadArgs += '--publish' }
$modeLabel = if ($Publish) { 'published pre-release' } else { 'draft pre-release' }
Write-Host "Uploading Origo Preview $Version to $RepoUrl as $modeLabel..."
& $dotnet @uploadArgs | Out-Host
if ($LASTEXITCODE) { throw 'Velopack GitHub upload failed' }
Write-Output $installer
