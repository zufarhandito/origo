# Multi-document races and integrated tab controls, on generated/owned sources only.
param([ValidateSet('debug','release')][string]$Profile='release')
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'tabs-hardening'
$saved=[Windows.Forms.Clipboard]::GetDataObject();$backup=New-Object Windows.Forms.DataObject
if($saved){foreach($format in $saved.GetFormats($false)){$data=$saved.GetData($format,$false);if($null -ne $data){$backup.SetData($format,$data)}}}
function Assert-Copy {
    Invoke 'Salin teks'
    $null=Wait-State {param($s) -not $s.copyingText -and -not $s.textLoading}
    if([Windows.Forms.Clipboard]::GetText() -ne 'PAGE 1'){throw 'Selection text changed across tab/cache lifecycle'}
}
function Assert-TabContainsClose([string]$Label,[string]$Close) {
    $tab=Find $Label;$closeNode=Find $Close
    $t=$tab.Current.BoundingRectangle;$c=$closeNode.Current.BoundingRectangle
    if($c.Left -lt $t.Left -or $c.Right -gt $t.Right+1 -or $c.Top -lt $t.Top -or $c.Bottom -gt $t.Bottom+1){throw 'Close button is outside its document tab'}
    if($tab.Current.ControlType -ne [Windows.Automation.ControlType]::TabItem){throw 'Document is not exposed as a tab item'}
}
try {
    $initial=Wait-State {param($s) $s.pageCount -eq 4 -and -not $s.textLoading}
    $idA=$initial.tabId
    Assert-TabContainsClose 'A.pdf' 'Tutup tab: A.pdf'
    $list=Find 'Daftar tab'
    if(-not $list.Current.IsEnabled -or $list.Current.BoundingRectangle.Width -lt 70){throw 'Tab list trigger lost its label/control bounds'}
    Invoke 'Zoom';Invoke 'Sesuaikan halaman'
    $null=Wait-State {param($s) -not $s.textLoading}
    Start-Sleep -Milliseconds 300
    $rect=(Find 'Isi halaman 1').Current.BoundingRectangle
    $first=New-Object OrganizerClick+Point;$first.X=[int]($rect.Left+$rect.Width*0.11001587);$first.Y=[int]($rect.Top+$rect.Height*0.5516052)
    $last=New-Object OrganizerClick+Point;$last.X=[int]($rect.Left+$rect.Width*0.35012817);$last.Y=$first.Y
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$first)|Out-Null
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$last)|Out-Null
    $lp=[IntPtr](($first.Y -shl 16) -bor ($first.X -band 0xffff))
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x200,[IntPtr]::Zero,$lp)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
    for($step=1;$step -le 12;$step++){
        $x=[int]($first.X+($last.X-$first.X)*$step/12);$lp=[IntPtr](($first.Y -shl 16) -bor ($x -band 0xffff))
        [OrganizerClick]::SendMessage($app.MainWindowHandle,0x200,[IntPtr]1,$lp)|Out-Null
        Start-Sleep -Milliseconds 20
    }
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]::Zero,$lp)|Out-Null
    $null=Wait-State {param($s) $s.selectedChars -eq 6}
    Assert-Copy
    Invoke 'Halaman terakhir'
    $null=Wait-State {param($s) $s.activePage -eq 4}
    Picker 'Buka PDF';Fill-Picker $b
    $second=Wait-State {param($s) $s.tabCount -eq 2 -and $s.tabId -ne $idA}
    $idB=$second.tabId
    Switch-Tab $a
    $null=Wait-State {param($s) $s.tabId -eq $idA -and $s.activePage -eq 4 -and $s.selectedChars -eq 6 -and -not $s.textLoading}
    Assert-Copy # Page 1 is offscreen and its text cache was evicted on switch.
    Switch-Tab $b
    Click-Control 'Tutup tab: A.pdf'
    $null=Wait-State {param($s) $s.tabCount -eq 1 -and $s.tabId -eq $idB -and -not $s.dirty}
    # Repeated fast switches/close reject late render/text completions.
    Picker 'Buka PDF';Fill-Picker $a
    $idA=(Wait-State {param($s) $s.tabCount -eq 2 -and $s.tabId -ne $idB}).tabId
    foreach($round in 1..8){Click-Control 'B.pdf';Click-Control 'A.pdf'}
    Click-Control 'B.pdf';Click-Control 'Tutup tab: A.pdf'
    $null=Wait-State {param($s) $s.tabId -eq $idB -and $s.tabCount -eq 1 -and -not $s.textLoading}
    # A source edit while B is active must invalidate only A and remain reopenable.
    Picker 'Buka PDF';Fill-Picker $a
    $idA=(Wait-State {param($s) $s.tabCount -eq 2 -and $s.tabId -ne $idB}).tabId
    Switch-Tab $b
    $null=Wait-State {param($s) $s.tabId -eq $idB}
    [IO.File]::AppendAllText($a,"`n",(New-Object Text.UTF8Encoding($false)))
    $hashA=(Get-FileHash -LiteralPath $a).Hash
    Switch-Tab $a
    $null=Wait-State {param($s) $s.tabId -eq $idA -and $s.pageCount -eq 0}
    $errorTitle=Find 'Halaman belum dapat ditampilkan';Invoke 'Tutup'
    Switch-Tab $b
    $null=Wait-State {param($s) $s.tabId -eq $idB -and $s.pageCount -eq 4 -and -not $s.dirty}
    Picker 'Buka PDF';Fill-Picker $a
    $null=Wait-State {param($s) $s.pageCount -eq 4 -and $s.tabCount -eq 2 -and $s.tabId -ne $idA -and $s.tabId -ne $idB}
    # Hold an export at genuine loss consent, switch tabs, then complete/cancel it.
    $export=Join-Path $directory 'Export.pdf';Copy-Item (Join-Path $root '.research/export-fixtures/info.pdf') $export
    Picker 'Buka PDF';Fill-Picker $export
    $idExport=(Wait-State {param($s) $s.tabCount -eq 3 -and $s.pageCount -eq 1}).tabId
    Invoke 'Panel alat';Invoke 'Putar kanan'
    $null=Wait-State {param($s) $s.dirty}
    Picker 'Simpan sebagai';Fill-Picker (Join-Path $directory 'export-result.pdf')
    $null=Find 'Metadata dokumen dapat tidak terbawa'
    # Targeted UIA changes the background selection without sending global keys.
    Click-Control 'B.pdf'
    $null=Wait-State {param($s) $s.tabId -eq $idB -and $s.windowExportOwner -eq $idExport}
    Invoke 'Lanjutkan'
    $completed=Wait-State {param($s) $s.tabId -eq $idB -and $s.windowExportOwner -eq 0 -and ($s.sessions|Where-Object tabId -eq $idExport).exportsCompleted -eq 1}
    if($completed.dirty -or ($completed.sessions|Where-Object tabId -eq $idExport).dirty){throw 'Export receipt was routed to the wrong tab'}
    Switch-Tab $export;Invoke 'Putar kanan'
    Picker 'Simpan sebagai';Fill-Picker (Join-Path $directory 'cancelled.pdf')
    $null=Find 'Metadata dokumen dapat tidak terbawa'
    Invoke 'Batal'
    $null=Wait-State {param($s) -not $s.exporting -and $s.dirty}
    if(Test-Path (Join-Path $directory 'cancelled.pdf')){throw 'Cancelled export was published'}
    Switch-Tab $b;Invoke 'Panel alat';Invoke 'Putar kanan'
    $null=Wait-State {param($s) $s.tabId -eq $idB -and $s.dirty}
    Switch-Tab $export
    Picker 'Simpan sebagai';Fill-Picker (Join-Path $directory 'cancel-close-race.pdf')
    $null=Find 'Metadata dokumen dapat tidak terbawa'
    Click-Control ('B.pdf '+[char]0x2022)
    Click-Control 'Tutup tab: B.pdf'
    $null=Wait-State {param($s) $s.closingTabs -and $s.windowExportOwner -eq $idExport}
    Invoke 'Batal'
    $null=Wait-State {param($s) -not $s.closingTabs -and $s.tabCount -eq 3 -and $s.windowExportOwner -eq 0 -and ($s.sessions|Where-Object tabId -eq $idB).dirty}
    if(Test-Path (Join-Path $directory 'cancel-close-race.pdf')){throw 'Cancelled cross-tab export was published'}
    Capture 'integrated-tabs.png'
    if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $b).Hash -ne $hashB){throw 'Owned sources changed outside the explicit source-change test'}
    Close-Window
    foreach($ignored in 1..2){$null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Buang perubahan';Start-Sleep -Milliseconds 200}
    if(-not $app.WaitForExit(10000)){throw 'Owned hardening fixture did not close'}
    'PASS integrated tabs/close geometry, inactive close isolation, offscreen selection copy, rapid switch/close, source-change isolation/reopen and originating export receipts/cancel'|Set-Content (Join-Path $directory 'result.txt')
    Write-Host "PASS multi-tab hardening: $directory"
} catch {
    if($app -and -not $app.HasExited){Capture 'failure.png'}
    throw
} finally {
    [Windows.Forms.Clipboard]::SetDataObject($backup,$true)
}
