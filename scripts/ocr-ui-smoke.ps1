param([ValidateSet('debug','release')][string]$Profile='release',[string]$ExecutablePath)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -ExecutablePath $ExecutablePath -TestName 'ocr-ui' -Fixture '.research/ocr-fixtures/viewer.pdf'

function Query-Ocr([string]$Text) {
    $field=Find 'Cari dalam PDF'
    $field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue($Text)
    Start-Sleep -Milliseconds 300
    Wait-State {param($s) $s.searchOpen -and -not $s.searchRunning -and $s.searchScannedPages -eq 3}
}
try {
    $null=Wait-State {param($s) $s.pageCount -eq 3 -and -not $s.textLoading}
    $null=Find 'Butuh OCR'
    Invoke 'Cari teks'
    $before=Query-Ocr 'Buku ini'
    if($before.searchMatches -ne 0 -or $before.ocrPageCached){throw 'Search found OCR before user request'}

    Invoke 'Butuh OCR'
    $recognized=Wait-State {param($s) $s.ocrPageCached -and $s.searchMatches -eq 1 -and -not $s.dirty}
    if($recognized.activePage -ne 1){throw 'OCR changed the reading page'}
    Capture '01-local-ocr-and-live-find.png'
    Write-Host 'PASS OCR UI: OCR chip, live Find update, current page and clean PDF'

    $native=Query-Ocr 'Native text'
    if($native.searchMatches -ne 1){throw 'Native text search stopped working'}
    Invoke 'Hasil berikutnya'
    $null=Wait-State {param($s) $s.activePage -eq 3 -and -not $s.ocrPageCached}
    Write-Host 'PASS OCR UI: native text on a mixed PDF'

    Click-Control 'Tutup tab: A.pdf'
    $null=Wait-State {param($s) $s.tabCount -eq 0}
    Picker 'Buka PDF'
    Fill-Picker $a
    $null=Wait-State {param($s) $s.tabCount -eq 1 -and $s.activePage -eq 1 -and $s.ocrPageCached}
    if((Get-FileHash -LiteralPath $a).Hash -ne $hashA){throw 'OCR changed the source'}
    Write-Host 'PASS OCR UI: close and reopen reuse on-disk result; source hash identical'
    Close-Window
    if(-not $app.WaitForExit(4000)){throw 'Fixture Origo did not exit'}
    Write-Host "OCR UI evidence: $directory"
} catch {
    Capture 'failure.png'
    throw
}
