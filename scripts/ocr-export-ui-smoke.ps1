param([string]$ExecutablePath='target/tab-background-check/release/pdf-studio.exe')
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -ExecutablePath $ExecutablePath -TestName 'ocr-export-ui' -Fixture '.research/ocr-fixtures/viewer.pdf'

function Choose-OcrOutput([string]$Path) {
    Invoke 'Menu aplikasi'
    Invoke 'Berkas'
    Picker ('Simpan PDF yang bisa dicari'+[char]0x2026)
    Fill-Picker $Path
}

try {
    $null=Wait-State {param($s) $s.pageCount -eq 3 -and -not $s.textLoading}
    Invoke 'Butuh OCR'
    $initial=Wait-State {param($s) $s.ocrPageCached}
    $cancelled=Join-Path $directory 'Cancelled.pdf'
    Choose-OcrOutput $cancelled
    Invoke 'Batal'
    $before=Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $initial.exportsCompleted}
    if(Test-Path -LiteralPath $cancelled){throw 'Cancelling partial export published a PDF'}

    $saved=Join-Path $directory 'Searchable.pdf'
    Choose-OcrOutput $saved
    Invoke 'Lanjutkan'
    $null=Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before.exportsCompleted}
    if(-not (Test-Path -LiteralPath $saved)){throw 'Searchable copy missing'}

    Picker 'Buka PDF'
    Fill-Picker $saved
    $null=Wait-State {param($s) $s.tabCount -eq 2 -and $s.activePage -eq 1 -and -not $s.textLoading}
    Invoke 'Cari teks'
    (Find 'Cari dalam PDF').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('Buku ini')
    $null=Wait-State {param($s) $s.searchScannedPages -eq 3 -and $s.searchMatches -eq 1 -and -not $s.ocrPageCached}
    if((Get-FileHash -LiteralPath $a).Hash -ne $hashA){throw 'Source PDF changed'}
    Close-Window
    if(-not $app.WaitForExit(4000)){throw 'Fixture Origo did not exit'}
    Write-Host 'PASS OCR export UI: partial confirmation, cancel, save, and native Find on the copy'
    Write-Host "OCR export UI evidence: $directory"
} catch {
    if(-not $app.HasExited) { Capture 'failure.png'; Close-Window }
    throw
}
