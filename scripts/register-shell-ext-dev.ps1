param(
    [ValidateSet('debug', 'release')][string]$Profile = 'debug',
    [string]$TargetExe,
    [switch]$SkipBuild,
    [switch]$PackageOnly,
    [switch]$NoElevate
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent

function Find-SdkTool([string]$Name) {
    $kits = 'C:\Program Files (x86)\Windows Kits\10\bin'
    if (Test-Path -LiteralPath $kits) {
        $found = Get-ChildItem -LiteralPath $kits -Recurse -Filter $Name -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match '\\x64\\' } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
        if ($found) { return $found.FullName }
    }
    $cmd = Get-Command $Name -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    throw "Windows SDK tool '$Name' not found. Install Windows 10/11 SDK."
}

function New-ResizedPng([string]$SourcePng, [string]$DestPng, [int]$Size) {
    Add-Type -AssemblyName System.Drawing
    $src = [Drawing.Image]::FromFile($SourcePng)
    try {
        $bmp = New-Object Drawing.Bitmap($Size, $Size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
        try {
            $g = [Drawing.Graphics]::FromImage($bmp)
            try {
                $g.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
                $g.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::HighQuality
                $g.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
                $g.Clear([Drawing.Color]::Transparent)
                $g.DrawImage($src, 0, 0, $Size, $Size)
            } finally { $g.Dispose() }
            $bmp.Save($DestPng, [Drawing.Imaging.ImageFormat]::Png)
        } finally { $bmp.Dispose() }
    } finally { $src.Dispose() }
}

if (-not $SkipBuild) {
    . (Join-Path $PSScriptRoot 'use-build-environment.ps1')
    $buildArgs = @('build', '--locked', '--offline', '-p', 'origo-shell-ext', '-p', 'pdf-studio')
    if ($Profile -eq 'release') { $buildArgs += '--release' }
    Push-Location $root
    try {
        & cargo @buildArgs
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
    } finally { Pop-Location }
}

$targetDir = Join-Path $root "target\$Profile"
$dllPath = Join-Path $targetDir 'origo_shell_ext.dll'
$exePath = if ($TargetExe) { $TargetExe } else { Join-Path $targetDir 'pdf-studio.exe' }
$stageExePath = Join-Path $targetDir 'pdf-studio.exe'
$icoPath = Join-Path $root 'apps\desktop\resources\branding\origo.ico'
$srcPng = Join-Path $root 'apps\desktop\resources\branding\origo-source.png'
$manifestSrc = Join-Path $root 'packaging\shell-ext\AppxManifest.xml'

foreach ($required in @($dllPath, $stageExePath, $icoPath, $srcPng, $manifestSrc)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "Required file missing: $required"
    }
}

$makeappx = Find-SdkTool 'makeappx.exe'
$signtool = Find-SdkTool 'signtool.exe'

$devBase = Join-Path $env:LOCALAPPDATA 'Origo\ShellExtDev'
$runId = Get-Date -Format 'yyyyMMdd-HHmmss'
$extDir = Join-Path $devBase "ext-$runId"
$pkgStage = Join-Path $devBase 'msix-stage'
$assetsDir = Join-Path $pkgStage 'Assets'
$msixPath = Join-Path $devBase 'OrigoShellExt.msix'
$cerPath = Join-Path $devBase 'OrigoDev.cer'

New-Item -ItemType Directory -Path $extDir -Force | Out-Null
if (Test-Path -LiteralPath $pkgStage) { Remove-Item -LiteralPath $pkgStage -Recurse -Force }
New-Item -ItemType Directory -Path $assetsDir -Force | Out-Null

# Stage external location files (isolated per run to avoid dllhost.exe file-lock conflicts)
Copy-Item -LiteralPath $dllPath -Destination (Join-Path $extDir 'origo_shell_ext.dll') -Force
Copy-Item -LiteralPath $stageExePath -Destination (Join-Path $extDir 'pdf-studio.exe') -Force
Copy-Item -LiteralPath $icoPath -Destination (Join-Path $extDir 'origo.ico') -Force
# Write UTF-8 (no BOM) target pointer so the DLL launches the configured pdf-studio.exe directly
[IO.File]::WriteAllText((Join-Path $extDir 'origo-target.txt'), $exePath, (New-Object Text.UTF8Encoding($false)))

# Stage MSIX sparse package content (manifest + visual assets)
Copy-Item -LiteralPath $manifestSrc -Destination (Join-Path $pkgStage 'AppxManifest.xml') -Force
New-ResizedPng $srcPng (Join-Path $assetsDir 'StoreLogo.png') 50
New-ResizedPng $srcPng (Join-Path $assetsDir 'Square44x44Logo.png') 44
New-ResizedPng $srcPng (Join-Path $assetsDir 'Square150x150Logo.png') 150

if (Test-Path -LiteralPath $msixPath) { Remove-Item -LiteralPath $msixPath -Force }
& $makeappx pack /d $pkgStage /p $msixPath /o /nv | Out-Host
if ($LASTEXITCODE -ne 0) { throw "makeappx pack failed with exit code $LASTEXITCODE" }

# Create or reuse local self-signed dev code-signing certificate
$cert = Get-ChildItem 'Cert:\CurrentUser\My' |
    Where-Object { $_.Subject -eq 'CN=Origo Dev' -and $_.HasPrivateKey -and $_.NotAfter -gt (Get-Date).AddDays(7) } |
    Sort-Object NotAfter -Descending |
    Select-Object -First 1

if (-not $cert) {
    $cert = New-SelfSignedCertificate `
        -Type Custom `
        -Subject 'CN=Origo Dev' `
        -KeyUsage DigitalSignature `
        -FriendlyName 'Origo Explorer Extension Dev' `
        -CertStoreLocation 'Cert:\CurrentUser\My' `
        -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}') `
        -NotAfter (Get-Date).AddYears(3)
}

Export-Certificate -Cert $cert -FilePath $cerPath -Force | Out-Null

& $signtool sign /fd SHA256 /sha1 $cert.Thumbprint $msixPath | Out-Host
if ($LASTEXITCODE -ne 0) { throw "signtool sign failed with exit code $LASTEXITCODE" }

Write-Host "Sparse MSIX built and signed: $msixPath"
Write-Host "ExternalLocation: $extDir"

if ($PackageOnly) {
    exit 0
}

# Ensure dev cert is trusted by LocalMachine\TrustedPeople so Add-AppxPackage accepts it
$trusted = Get-ChildItem 'Cert:\LocalMachine\TrustedPeople' -ErrorAction SilentlyContinue |
    Where-Object { $_.Thumbprint -eq $cert.Thumbprint }
if (-not $trusted) {
    $trustedRoot = Get-ChildItem 'Cert:\LocalMachine\Root' -ErrorAction SilentlyContinue |
        Where-Object { $_.Thumbprint -eq $cert.Thumbprint }
    $trusted = $trustedRoot
}

if (-not $trusted) {
    $principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    $isAdmin = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if ($isAdmin) {
        Import-Certificate -FilePath $cerPath -CertStoreLocation 'Cert:\LocalMachine\TrustedPeople' | Out-Null
        Write-Host 'Trusted Origo Dev certificate in LocalMachine\TrustedPeople.'
    } elseif ($NoElevate) {
        Write-Host ''
        Write-Host 'Dev certificate is not yet trusted in LocalMachine\TrustedPeople.'
        Write-Host 'Run once in an Administrator PowerShell:'
        Write-Host "  Import-Certificate -FilePath `"$cerPath`" -CertStoreLocation 'Cert:\LocalMachine\TrustedPeople'"
        Write-Host 'Then re-run:'
        Write-Host "  powershell -NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`" -SkipBuild"
        exit 2
    } else {
        Write-Host 'Requesting one-time elevation to trust Origo Dev certificate in LocalMachine\TrustedPeople...'
        $cmd = "Import-Certificate -FilePath '$cerPath' -CertStoreLocation 'Cert:\LocalMachine\TrustedPeople' | Out-Null"
        $p = Start-Process -FilePath 'powershell.exe' -Verb RunAs -ArgumentList @('-NoProfile', '-Command', $cmd) -Wait -PassThru
        if ($p.ExitCode -ne 0) { throw 'Certificate trust elevation was cancelled or failed.' }
    }
}

# Replace any prior dev registration cleanly
$existing = Get-AppxPackage -Name 'Origo.ShellExtension' -ErrorAction SilentlyContinue
if ($existing) {
    $existing | Remove-AppxPackage
}

Add-AppxPackage -Path $msixPath -ExternalLocation $extDir

# Clean older unlocked ext-* directories
Get-ChildItem -LiteralPath $devBase -Directory -Filter 'ext-*' -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -ne $extDir } |
    ForEach-Object { Remove-Item -LiteralPath $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }

Write-Host 'PASS: Origo Windows 11 Explorer shell extension registered!'
Write-Host 'Right-click one or more PNG/JPEG files in File Explorer -> Origo.'
