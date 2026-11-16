# Creates an internal test candidate. Does not publish or certify a release.
param([switch]$SkipBuild, [string]$RuntimeDirectory)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
. (Join-Path $PSScriptRoot 'distribution-support.ps1')
Push-Location $root
try {
    if (-not $SkipBuild) {
        & cargo build --locked --offline --release -p pdf-studio
        if ($LASTEXITCODE) { throw 'Release build failed' }
    }
    $revision = (& git rev-parse --short HEAD).Trim()
    if ($LASTEXITCODE) { throw 'Git revision unavailable' }
    $dirty = [bool](& git status --porcelain --untracked-files=normal)
    $name = 'pdf-studio-windows-x64-candidate-' + $revision + '-' + [guid]::NewGuid().ToString('N').Substring(0,8)
    $dist = Join-Path $root 'dist'
    New-Item -ItemType Directory -Path $dist -Force | Out-Null
    $package = Join-Path $dist $name
    New-Item -ItemType Directory -Path $package | Out-Null
    Copy-Item -LiteralPath (Join-Path $root 'target/release/pdf-studio.exe') -Destination $package
    Copy-Item -LiteralPath (Join-Path $root 'sidecars.lock.json') -Destination $package
    $nativeStage = Join-Path $dist ($name + '-verified-inputs')
    New-Item -ItemType Directory -Path $nativeStage | Out-Null
    $lock = Get-Content -Raw (Join-Path $root 'sidecars.lock.json') | ConvertFrom-Json
    # Re-extract hash-verified archives; never trust mutable staged DLLs.
    foreach ($engine in @('qpdf','pdfium')) {
        $entry = $lock.$engine
        $extension = if ($engine -eq 'qpdf') {'zip'} else {'tgz'}
        $archive = Join-Path $root ('sidecars/' + $engine + '-' + $entry.version + '.' + $extension)
        if ((Get-FileHash -LiteralPath $archive).Hash.ToLowerInvariant() -ne $entry.sha256) { throw "Archive checksum mismatch: $engine" }
        $stage = Join-Path $nativeStage $engine
        New-Item -ItemType Directory -Path $stage | Out-Null
        if ($engine -eq 'qpdf') { Expand-Archive -LiteralPath $archive -DestinationPath $stage }
        else { & tar -xf $archive -C $stage; if ($LASTEXITCODE) { throw 'PDFium extraction failed' } }
        $source = if ($engine -eq 'qpdf') { Join-Path $stage ('qpdf-' + $entry.version + '-msvc64') } else { $stage }
        $destination = Join-Path $package ('sidecars/' + $engine)
        New-Item -ItemType Directory -Path $destination -Force | Out-Null
        # All QPDF runtime DLLs and its complete documentation/notices; no libs/includes.
        foreach ($part in $(if ($engine -eq 'qpdf') {@('bin','share')} else {@('bin','licenses','LICENSE','VERSION','args.gn')})) {
            Copy-Item -LiteralPath (Join-Path $source $part) -Destination $destination -Recurse
        }
    }
    if (-not $RuntimeDirectory) {
        if ($env:VCToolsRedistDir) { $RuntimeDirectory = Join-Path $env:VCToolsRedistDir 'x64/Microsoft.VC143.CRT' }
        else {
            $redistBase = Join-Path $root '.tools/vs2022/VC/Redist/MSVC'
            $version = Get-ChildItem -LiteralPath $redistBase -Directory | Where-Object Name -Match '^\d+\.\d+\.\d+$' | Sort-Object {[version]$_.Name} -Descending | Select-Object -First 1
            if (-not $version) { throw 'Pass -RuntimeDirectory pointing to the licensed x64 Microsoft.VC143.CRT redist folder' }
            $RuntimeDirectory = Join-Path $version.FullName 'x64/Microsoft.VC143.CRT'
        }
    }
    $runtime = @(Get-ChildItem -LiteralPath $RuntimeDirectory -Filter '*.dll' -File)
    if (-not ($runtime | Where-Object Name -eq 'vcruntime140.dll')) { throw 'MSVC release CRT directory missing' }
    foreach ($dll in $runtime) {
        Copy-Item -LiteralPath $dll.FullName -Destination $package
        # Align QPDF to the same verified runtime version instead of an older copy.
        Copy-Item -LiteralPath $dll.FullName -Destination (Join-Path $package 'sidecars/qpdf/bin') -Force
    }
    Copy-Item -LiteralPath (Join-Path $root 'LICENSES') -Destination $package -Recurse
    $redistNotice = Join-Path $root '.tools/vs2022/Licenses/1033/Redist.txt'
    if ($env:VSINSTALLDIR) { $redistNotice = Join-Path $env:VSINSTALLDIR 'Licenses/1033/Redist.txt' }
    if (Test-Path -LiteralPath $redistNotice) { Copy-Item -LiteralPath $redistNotice -Destination (Join-Path $package 'LICENSES/MSVC-Redist.txt') }
    $audit = Join-Path $package 'LICENSES/dependencies'
    $null = & (Join-Path $PSScriptRoot 'audit-distribution.ps1') -OutputDirectory $audit
    $imports = Get-DistributionImports $package
    Write-DistributionJson $imports (Join-Path $package 'runtime-imports.json')
    $runtimeInventory = @($runtime | ForEach-Object {[pscustomobject]@{Name=$_.Name;Version=$_.VersionInfo.FileVersion;Sha256=(Get-FileHash -LiteralPath $_.FullName).Hash.ToLowerInvariant()}})
    Write-DistributionJson ([pscustomobject]@{Revision=$revision;Dirty=$dirty;Candidate=$true;CreatedUtc=[DateTime]::UtcNow.ToString('o');Target=$lock.target;Runtime=$runtimeInventory;OpenGates=@('project-license-and-redistribution-review','missing-license-evidence-see-inventory','physical-device-acceptance','clean-Windows-10-and-11-acceptance','dependency-security-review')}) (Join-Path $package 'build-info.json')
    Copy-Item -LiteralPath (Join-Path $root 'docs/portable-readme.txt') -Destination (Join-Path $package 'README.txt')
    Copy-Item -LiteralPath (Join-Path $root 'docs/release-checklist.md') -Destination (Join-Path $package 'RELEASE-CHECKLIST.md')
    $files = Get-DistributionFiles $package
    Write-DistributionJson $files (Join-Path $package 'files.sha256.json')
    $zip = Join-Path $dist ($name + '.zip')
    # Some upstream archives use a Unix-epoch mtime, outside ZIP's 1980+ range.
    # Normalize only our owned copy; source archives/files remain untouched.
    Get-ChildItem -LiteralPath $package -Recurse -File | ForEach-Object { $_.LastWriteTimeUtc = [datetime]'2000-01-01T00:00:00Z' }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::CreateFromDirectory($package,$zip,[IO.Compression.CompressionLevel]::Optimal,$true)
    $zipHash = (Get-FileHash -LiteralPath $zip).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText(($zip + '.sha256'), ($zipHash + '  ' + (Split-Path $zip -Leaf) + "`n"), (New-Object Text.UTF8Encoding $false))
    Write-Host "PASS internal portable candidate: $zip"
    Write-Output $zip
} finally { Pop-Location }
