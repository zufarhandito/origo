param([Parameter(Mandatory=$true)][string]$PackageDirectory)
$ErrorActionPreference = 'Stop'
$PackageDirectory = (Resolve-Path -LiteralPath $PackageDirectory).Path
$null = & (Join-Path $PSScriptRoot 'verify-portable.ps1') -Directory $PackageDirectory
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -TestName 'portable-ui' -ExecutablePath (Join-Path $PackageDirectory 'pdf-studio.exe')
try {
    $null = Wait-State {param($s) $s.pageCount -eq 4 -and -not $s.textLoading}
    $modules = @($app.Modules | ForEach-Object {[pscustomobject]@{Name=$_.ModuleName;Path=$_.FileName}})
    foreach ($name in @('pdfium.dll','vcruntime140.dll')) {
        $module = $modules | Where-Object Name -eq $name
        if (-not $module -or -not $module.Path.StartsWith(($PackageDirectory+'\'),[StringComparison]::OrdinalIgnoreCase)) { throw "Packaged module did not load locally: $name" }
    }
    $modules | ConvertTo-Json | Set-Content -Encoding UTF8 (Join-Path $directory 'loaded-modules.json')
    Capture 'relocated-reader.png'
    Invoke 'Panel alat'; Invoke 'Putar kanan'
    $before = Wait-State {param($s) $s.dirty}
    $output = Join-Path $directory ('saved ' + [char]0x65E5 + [char]0x672C + ' & output.pdf')
    Picker 'Simpan sebagai'; Fill-Picker $output
    $after = Wait-State {param($s) -not $s.exporting -and $s.exportSuccess -and -not $s.dirty -and $s.exportsCompleted -gt $before.exportsCompleted}
    $qpdf = Join-Path $PackageDirectory 'sidecars/qpdf/bin/qpdf.exe'
    & $qpdf --check $output | Out-Null
    if ($LASTEXITCODE) { throw 'Packaged export invalid' }
    $json = & $qpdf --json=2 --json-key=pages --json-key=qpdf $output | ConvertFrom-Json
    if ($json.pages.Count -ne 4) { throw 'Packaged export page count mismatch' }
    $page = $json.qpdf[1].PSObject.Properties['obj:'+$json.pages[0].object].Value.value
    if ($page.'/Rotate' -ne 90) { throw 'Packaged export rotation mismatch' }
    Picker 'Buka PDF'; Fill-Picker $output
    $null = Wait-State {param($s) $s.tabCount -eq 2 -and $s.pageCount -eq 4 -and -not $s.dirty}
    Capture 'relocated-reopened.png'
    if ((Get-FileHash -LiteralPath $a).Hash -ne $hashA) { throw 'Portable test changed source' }
    if (Get-ChildItem -LiteralPath $directory -Directory -Filter '.pdf-studio-export-*') { throw 'Portable export leaked staging' }
    [pscustomobject]@{Status='passed';Package=$PackageDirectory;Checks=@('restricted-PATH-launch','local-pdfium-and-msvc','render-text','edit-save-reopen','qpdf-page-count-rotation','unicode-output','source-hash','staging-cleanup');Utc=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 (Join-Path $directory 'result.json')
    Write-Host "PASS relocated package UI/render/edit/export/reopen: $directory"
} finally {
    if ($app -and -not $app.HasExited) { Close-Window; $null=$app.WaitForExit(10000) }
}
