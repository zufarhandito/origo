param([string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
. (Join-Path $PSScriptRoot 'distribution-support.ps1')
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $root ('.research/distribution-audit-' + [guid]::NewGuid().ToString('N')) }
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Audit output must be a new directory' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
Push-Location $root
try {
    $json = & cargo metadata --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE) { throw 'Locked offline Cargo metadata failed; activate the pinned build environment' }
    $metadata = ($json -join "`n") | ConvertFrom-Json
    $nodes = @{}; foreach ($node in $metadata.resolve.nodes) { $nodes[$node.id] = $node }
    $todo = New-Object 'Collections.Generic.Queue[string]'
    $seen = New-Object 'Collections.Generic.HashSet[string]'
    $todo.Enqueue(($metadata.packages | Where-Object name -eq 'pdf-studio').id)
    while ($todo.Count) {
        $id = $todo.Dequeue()
        if ($seen.Add($id)) {
            foreach ($dependency in $nodes[$id].deps) {
                if (@($dependency.dep_kinds | Where-Object kind -ne 'dev').Count) { $todo.Enqueue($dependency.pkg) }
            }
        }
    }
    $records = @(foreach ($package in ($metadata.packages | Where-Object { $seen.Contains($_.id) } | Sort-Object name,version)) {
        $directory = Split-Path $package.manifest_path -Parent
        $relative = 'cargo/' + $package.name + '-' + $package.version
        $destination = Join-Path $OutputDirectory $relative
        New-Item -ItemType Directory -Path $destination -Force | Out-Null
        $notices = @(Get-ChildItem -LiteralPath $directory -Recurse -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|COPYRIGHT|NOTICE|UNLICENSE)([._-]|$)' })
        if ($package.license_file) { $notices += Get-Item -LiteralPath $package.license_file }
        foreach ($notice in ($notices | Sort-Object FullName -Unique)) {
            $noticeRelative = $notice.FullName.Substring($directory.Length + 1)
            $target = Join-Path $destination $noticeRelative
            New-Item -ItemType Directory -Path (Split-Path $target) -Force | Out-Null
            Copy-Item -LiteralPath $notice.FullName -Destination $target
        }
        $supplementalManifest = Join-Path $root 'LICENSES/supplemental/manifest.json'
        $supplemental = @()
        if (Test-Path -LiteralPath $supplementalManifest) {
            $entries = Get-Content -Raw -LiteralPath $supplementalManifest | ConvertFrom-Json
            $supplemental = @($entries | Where-Object { $_.Name -eq $package.name -and $_.Version -eq $package.version })
            foreach ($entry in $supplemental) {
                if ($entry.File -ne [IO.Path]::GetFileName($entry.File)) { throw 'Unsafe supplemental notice path' }
                $source = Join-Path (Split-Path $supplementalManifest) $entry.File
                if ((Get-FileHash -LiteralPath $source).Hash.ToLowerInvariant() -ne $entry.Sha256) { throw "Supplemental notice hash mismatch: $($package.name)" }
                Copy-Item -LiteralPath $source -Destination $destination
            }
        }
        # Preserve exact cached crate source for file-level copyleft review, without fetching.
        $sourceArchive = $null
        if ($package.license -match 'MPL|GPL|LGPL') {
            $registryRoot = Split-Path (Split-Path (Split-Path $directory))
            $cache = Join-Path $registryRoot ('cache/' + (Split-Path (Split-Path $directory) -Leaf) + '/' + $package.name + '-' + $package.version + '.crate')
            if (-not (Test-Path -LiteralPath $cache)) { throw "Missing source archive for $($package.name)" }
            Copy-Item -LiteralPath $cache -Destination $destination
            $sourceArchive = $relative + '/' + (Split-Path $cache -Leaf)
        }
        $noticePaths = @($notices | Sort-Object FullName -Unique | ForEach-Object {$relative + '/' + $_.FullName.Substring($directory.Length + 1).Replace('\','/')})
        $noticePaths += @($supplemental | ForEach-Object {$relative + '/' + $_.File})
        [pscustomobject]@{Name=$package.name;Version=$package.version;License=$package.license;Source=$package.source;Repository=$package.repository;Notices=$noticePaths;SupplementalSources=@($supplemental | ForEach-Object Url);SourceArchive=$sourceArchive;Review=if(-not $package.source){'project-license-unset'}elseif(-not $package.license -or -not $noticePaths.Count){'missing-license-evidence'}elseif($package.license -match 'MPL|GPL|LGPL'){'copyleft-source-review'}else{'notice-collected'}}
    })
    Write-DistributionJson $records (Join-Path $OutputDirectory 'cargo-inventory.json')
    Copy-Item -LiteralPath (Join-Path $root 'Cargo.lock') -Destination $OutputDirectory
    $records | Group-Object Review | Select-Object Count,Name | Format-Table -AutoSize | Out-Host
    Write-Host "Inventory of $($records.Count) desktop normal/build dependencies (workspace-feature superset): $OutputDirectory"
    Write-Output $OutputDirectory
} finally { Pop-Location }
