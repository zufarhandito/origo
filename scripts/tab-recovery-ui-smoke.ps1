param([ValidateSet('debug','release')][string]$Profile='release')
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'tab-recovery-ui' -IsolatedState
$cases=New-Object Collections.Generic.List[object]
function Pass([string]$Name){$cases.Add([pscustomobject]@{Case=$Name;Status='passed'});$cases.ToArray()|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'cases.json');Write-Host "PASS tab recovery: $Name"}
function Key([int]$Code){
    [OutputDialog]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]$Code,[IntPtr]0x10001)|Out-Null
    [OutputDialog]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]$Code,[IntPtr]0xC0010001)|Out-Null
    Start-Sleep -Milliseconds 200
}
function Set-TabFilter([string]$Text){
    $field=Find 'Cari tab';$field.SetFocus()
    $field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue($Text)
    Start-Sleep -Milliseconds 200
}
try {
    $s=Wait-State {param($s) $s.tabCount -eq 1}
    $firstId=$s.tabId
    Menu-Action 'Tampilan' 'Sesuaikan halaman'
    $field=Find 'Nomor halaman tujuan';$field.SetFocus()
    $field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('3');Key 13
    $saved=Wait-State {param($s) $s.activePage -eq 3}
    Invoke 'Tutup tab: A.pdf'
    $null=Wait-State {param($s) $s.tabCount -eq 0 -and $s.closedTabCount -eq 1}
    Menu-Action 'Berkas' 'Buka ulang tab tertutup  Ctrl+Shift+T'
    $s=Wait-State {param($s) $s.tabCount -eq 1 -and $s.activePage -eq 3 -and $s.closedTabCount -eq 0}
    if($s.tabId -eq $firstId -or $s.zoomPercent -ne $saved.zoomPercent){throw 'Reopen identity/reading position/zoom failed'}
    Pass 'reopen-original-reading-position-and-zoom'
    Picker 'Buka PDF';Fill-Picker $b
    $s=Wait-State {param($s) $s.tabCount -eq 2}
    $idB=$s.tabId
    Invoke 'Daftar tab';Set-TabFilter 'a.PDF';Key 13
    $null=Wait-State {param($s) $s.tabId -ne $idB -and -not $s.tabPickerOpen}
    Invoke 'Daftar tab';Set-TabFilter 'no-matching-tab'
    $null=Find 'Tidak ada tab yang cocok.'
    Key 27
    $null=Wait-State {param($s) -not $s.tabPickerOpen}
    Invoke 'Daftar tab';Set-TabFilter '';Key 40;Key 13
    $null=Wait-State {param($s) $s.tabId -eq $idB -and -not $s.tabPickerOpen}
    Pass 'tab-search-insensitive-empty-and-keyboard'
    Invoke 'Tutup tab: B.pdf'
    $null=Wait-State {param($s) $s.closedTabCount -eq 1}
    Remove-Item -LiteralPath $b
    Menu-Action 'Berkas' 'Buka ulang tab tertutup  Ctrl+Shift+T'
    $null=Wait-State {param($s) -not $s.tabsOpening -and $s.closedTabCount -eq 1 -and $s.tabCount -eq 1}
    Copy-Item (Join-Path $root 'tests/fixtures/text-selection.pdf') $b
    Menu-Action 'Berkas' 'Buka ulang tab tertutup  Ctrl+Shift+T'
    $null=Wait-State {param($s) $s.tabCount -eq 2 -and $s.closedTabCount -eq 0 -and $s.activePage -eq 1}
    Pass 'missing-source-retry-and-changed-source-first-page'
    Invoke 'Panel alat';Invoke 'Putar kanan'
    $null=Wait-State {param($s) $s.dirty}
    Invoke 'Tutup tab: B.pdf'
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Batal'
    $null=Wait-State {param($s) $s.tabCount -eq 2 -and $s.closedTabCount -eq 0 -and $s.dirty}
    Invoke 'Tutup tab: B.pdf'
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Buang perubahan'
    $null=Wait-State {param($s) $s.tabCount -eq 1 -and $s.closedTabCount -eq 1}
    Menu-Action 'Berkas' 'Buka ulang tab tertutup  Ctrl+Shift+T'
    $null=Wait-State {param($s) $s.tabCount -eq 2 -and -not $s.dirty -and $s.pagePlan[0][2] -eq 0}
    Pass 'cancel-does-not-record-and-discard-does-not-recover-edits'
    Invoke 'Tutup tab: B.pdf'
    $null=Wait-State {param($s) $s.closedTabCount -eq 1}
    Picker 'Buka PDF';Fill-Picker $b
    $s=Wait-State {param($s) $s.tabCount -eq 2}
    $existingId=$s.tabId
    Menu-Action 'Berkas' 'Buka ulang tab tertutup  Ctrl+Shift+T'
    $null=Wait-State {param($s) $s.tabCount -eq 2 -and $s.tabId -eq $existingId -and $s.closedTabCount -eq 0}
    Pass 'duplicate-reopen-focuses-existing-tab'
    Capture 'tab-search-and-reopen.png'
    Close-Window
    if(-not $app.WaitForExit(15000)){throw 'Fixture did not close'}
    if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $b).Hash -ne $hashB){throw 'Source changed'}
    Pass 'source-hashes-and-normal-close'
    Write-Host "PASS tab recovery UI: $directory"
} catch {if($app -and -not $app.HasExited){Capture 'failure.png'};throw}
