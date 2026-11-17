param([ValidateSet('debug','release')][string]$Profile='release')
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'search-ui' -Fixture '.research/search-fixtures/search.pdf'
$cases=New-Object Collections.Generic.List[object]
function Pass([string]$Name) {
    $cases.Add([pscustomobject]@{Case=$Name;Status='passed'})
    $cases.ToArray()|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'cases.json')
    Write-Host "PASS search UI: $Name"
}
function Query([string]$Text) {
    $field=Find 'Cari dalam PDF';$field.SetFocus()
    $field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue($Text)
    Start-Sleep -Milliseconds 300
    Wait-State {param($s) $s.searchOpen -and -not $s.searchRunning}
}
function Key([int]$Code) {
    [OutputDialog]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]$Code,[IntPtr]0x10001)|Out-Null
    [OutputDialog]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]$Code,[IntPtr]0xC0010001)|Out-Null
    Start-Sleep -Milliseconds 200
}
try {
    $initial=Wait-State {param($s) $s.pageCount -eq 7}
    $idA=$initial.tabId
    Invoke 'Cari teks'
    $state=Query 'hasil penelitian'
    if($state.searchCaseSensitive -or $state.searchMatches -ne 5 -or $state.searchScannedPages -ne 7 -or $state.searchCurrent -ne 1 -or $state.dirty){throw 'Phrase/default case/linebreak/scattered-word contract failed'}
    Pass 'default-insensitive-contiguous-phrase-and-linebreaks'
    $tab=(Find 'A.pdf').Current.BoundingRectangle;$input=(Find 'Cari dalam PDF').Current.BoundingRectangle
    if($input.Top -le $tab.Bottom){throw 'Find input is not below document tabs'}
    Capture 'phrase-highlight.png'
    Pass 'find-bar-inside-document-below-tabs'
    Invoke 'Hasil berikutnya'
    $null=Wait-State {param($s) $s.searchCurrent -eq 2 -and $s.activePage -eq 2 -and -not $s.textLoading}
    Capture 'rotated-highlight.png'
    Key 13
    $null=Wait-State {param($s) $s.searchCurrent -eq 3 -and $s.activePage -eq 3}
    Key 114 # F3, targeted to the fixture HWND only.
    $null=Wait-State {param($s) $s.searchCurrent -eq 4 -and $s.activePage -eq 4}
    Invoke 'Hasil berikutnya';Invoke 'Hasil berikutnya'
    $null=Wait-State {param($s) $s.searchCurrent -eq 1 -and $s.activePage -eq 1}
    Invoke 'Hasil sebelumnya'
    $null=Wait-State {param($s) $s.searchCurrent -eq 5 -and $s.activePage -eq 5}
    Pass 'next-previous-enter-f3-wrap-and-rotated-navigation'
    Invoke 'Opsi pencarian';Invoke 'Bedakan huruf besar-kecil'
    $null=Wait-State {param($s) -not $s.searchRunning -and $s.searchCaseSensitive -and $s.searchMatches -eq 3}
    Invoke 'Opsi pencarian';Invoke 'Bedakan huruf besar-kecil'
    $state=Query 'data'
    if($state.searchMatches -ne 2){throw 'Substring search failed'}
    Invoke 'Opsi pencarian';Invoke 'Kata utuh'
    $null=Wait-State {param($s) -not $s.searchRunning -and $s.searchWholeWords -and $s.searchMatches -eq 1}
    Invoke 'Opsi pencarian';Invoke 'Kata utuh'
    Pass 'optional-case-and-whole-word-matching'
    $state=Query 'no-such-phrase'
    if($state.searchMatches -ne 0){throw 'No-results count failed'}
    $null=Find 'Tidak ditemukan'
    $state=Query ''
    if($state.searchMatches -ne 0 -or $state.searchScannedPages -ne 0){throw 'Empty query did native work'}
    Pass 'empty-and-no-results'
    $state=Query ('x'*1025)
    if($state.searchMatches -ne 0 -or $state.searchScannedPages -ne 0){throw 'Oversized query did native work'}
    $null=Find 'Maksimal 1024 karakter'
    Pass 'oversized-query-stops-before-native-work'
    $field=Find 'Cari dalam PDF';$field.SetFocus()
    $value=$field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
    $value.SetValue('hasil');$value.SetValue('no-such-phrase');$value.SetValue('hasil penelitian')
    Start-Sleep -Milliseconds 400
    $null=Wait-State {param($s) -not $s.searchRunning -and $s.searchMatches -eq 5}
    Pass 'rapid-query-replacement-rejects-stale-results'
    Invoke 'Hasil berikutnya'
    Picker 'Buka PDF';Fill-Picker $b
    $idB=(Wait-State {param($s) $s.tabCount -eq 2 -and $s.tabId -ne $idA}).tabId
    if((Read-Metrics).searchOpen){throw 'Search leaked into new tab'}
    Invoke 'Cari teks';$state=Query 'data'
    if($state.searchMatches -ne 2){throw 'Second-tab search failed'}
    Switch-Tab $a
    $null=Wait-State {param($s) $s.tabId -eq $idA -and $s.searchOpen -and $s.searchMatches -eq 5 -and $s.searchCurrent -eq 2}
    if((Find 'Cari dalam PDF').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).Current.Value -ne 'hasil penelitian'){throw 'Per-tab query lost'}
    Pass 'independent-query-results-and-current-match-per-tab'
    $field=Find 'Cari dalam PDF';$field.SetFocus();Key 27
    $null=Wait-State {param($s) -not $s.searchOpen -and $s.searchMatches -eq 0}
    Invoke 'Cari teks'
    $null=Wait-State {param($s) -not $s.searchRunning -and $s.searchMatches -eq 5}
    Pass 'escape-closes-highlights-and-reopen-retains-query'
    Invoke 'Panel alat';Invoke 'Putar kanan'
    $null=Wait-State {param($s) -not $s.searchRunning -and $s.searchMatches -eq 5 -and $s.dirty}
    Invoke 'Impor PDF';Fill-Picker $b
    $null=Wait-State {param($s) $s.pageCount -eq 14 -and -not $s.searchRunning -and $s.searchMatches -eq 10}
    Invoke 'Hapus'
    $null=Wait-State {param($s) -not $s.searchRunning -and $s.pageCount -lt 14}
    Invoke 'Undo'
    $null=Wait-State {param($s) -not $s.searchRunning -and $s.pageCount -eq 14 -and $s.searchMatches -eq 10}
    Pass 'rotation-import-delete-undo-recompute-current-workspace'
    $blank=Join-Path $directory 'Blank.pdf';Copy-Item (Join-Path $root '.research/search-fixtures/blank.pdf') $blank
    Picker 'Buka PDF';Fill-Picker $blank
    $null=Wait-State {param($s) $s.pageCount -eq 1}
    Invoke 'Cari teks';$state=Query 'hasil penelitian'
    $null=Find 'Tidak ada teks yang dapat dicari; OCR diperlukan'
    if($state.searchMatches -ne 0){throw 'Empty PDF invented matches'}
    Pass 'image-or-empty-page-no-text-message'
    $long=Join-Path $directory 'Long.pdf';Copy-Item (Join-Path $root '.research/search-fixtures/long.pdf') $long
    Picker 'Buka PDF';Fill-Picker $long
    $idLong=(Wait-State {param($s) $s.pageCount -eq 1000}).tabId
    Invoke 'Cari teks'
    $field=Find 'Cari dalam PDF';$field.SetFocus()
    $field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('page')
    $deadline=(Get-Date).AddSeconds(10)
    do {Start-Sleep -Milliseconds 50;$progress=Read-Metrics}while((Get-Date) -lt $deadline -and (-not $progress.searchRunning -or $progress.searchScannedPages -eq 0))
    if(-not $progress.searchRunning -or $progress.searchScannedPages -eq 0){throw 'No progressive search state observed'}
    Click-Control 'B.pdf'
    $null=Wait-State {param($s) $s.tabId -eq $idB}
    $inactive=@((Read-Metrics).sessions|Where-Object tabId -eq $idLong)[0]
    $pausedAt=$inactive.searchScannedPages
    Start-Sleep -Milliseconds 400
    if(@((Read-Metrics).sessions|Where-Object tabId -eq $idLong)[0].searchScannedPages -ne $pausedAt){throw 'Inactive search continued scanning'}
    Switch-Tab $long
    $state=Wait-State {param($s) $s.tabId -eq $idLong -and -not $s.searchRunning -and $s.searchMatches -eq 1000}
    if($state.totalTextCacheBytes -gt 16MB -or $state.inactiveCacheBytes -ne 0){throw 'Search exceeded shared text cache bounds'}
    Pass 'thousand-page-progressive-search-pause-resume-and-cache-bounds'
    Click-Control 'Tutup tab: Long.pdf'
    Switch-Tab $b
    $field=Find 'Cari dalam PDF';$field.SetFocus()
    $field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('hasil penelitian')
    Click-Control 'Tutup tab: B.pdf'
    $null=Wait-State {param($s) $s.tabCount -eq 2 -and $s.tabId -ne $idB}
    Pass 'closing-searching-tab-rejects-late-completion'
    if((Get-FileHash -LiteralPath $a).Hash -ne $hashA -or (Get-FileHash -LiteralPath $b).Hash -ne $hashB){throw 'Search changed source PDFs'}
    Pass 'source-hashes-unchanged'
    Close-Window;$null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Buang perubahan'
    if(-not $app.WaitForExit(10000)){throw 'Owned search fixture did not close'}
    Write-Host "PASS search UI complete: $directory"
} catch {
    if($app -and -not $app.HasExited){Capture 'failure.png'}
    throw
}
