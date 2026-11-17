param([ValidateSet('debug','release')][string]$Profile='debug')
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'same-document-ui' -IsolatedState
$cases=New-Object Collections.Generic.List[object]
function Pass([string]$Name){Capture ($Name+'.png');$cases.Add([pscustomobject]@{Case=$Name;Status='passed'});$cases.ToArray()|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'cases.json');Write-Host "PASS same document: $Name"}
function In-Pane([string]$Side,[string]$Control){
    $deadline=(Get-Date).AddSeconds(8)
    do {$pane=Find ('Panel PDF: A.pdf ('+$Side+')')
    $node=$pane.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Control))
    if($node){return $node};Start-Sleep -Milliseconds 100}while((Get-Date) -lt $deadline)
    throw "Missing $Control in $Side"
}
function Click($Node){
    $r=$Node.Current.BoundingRectangle;$p=New-Object OrganizerClick+Point;$p.X=[int]($r.X+$r.Width/2);$p.Y=[int]($r.Y+[Math]::Min(12,$r.Height/2))
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$p)|Out-Null;$lp=[IntPtr](($p.Y -shl 16) -bor ($p.X -band 0xffff))
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x200,[IntPtr]0,$lp)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$lp)|Out-Null;Start-Sleep -Milliseconds 150
}
function Focus([string]$Side){Click (Find ('Panel PDF: A.pdf ('+$Side+')'))}
function Key([int]$Code){[OutputDialog]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]$Code,[IntPtr]0x10001)|Out-Null;[OutputDialog]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]$Code,[IntPtr]0xC0010001)|Out-Null;Start-Sleep -Milliseconds 150}
function Page([string]$Number){$field=Find 'Nomor halaman tujuan';$field.SetFocus();$field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue($Number);Key 13}
function Check-Shared([int]$Count,[bool]$Dirty){
    $s=Wait-State {param($s) $s.pageCount -eq $Count -and $s.dirty -eq $Dirty -and @($s.sessions|Where-Object {$_.pageCount -ne $Count -or $_.dirty -ne $Dirty}).Count -eq 0}
    if(($s.sessions[0].pagePlan|ConvertTo-Json -Compress) -ne ($s.sessions[1].pagePlan|ConvertTo-Json -Compress)){throw 'Views have divergent page plans'}
    if($s.totalPageCacheBytes -gt 128MB -or $s.totalThumbnailCacheBytes -gt 16MB -or $s.totalTextCacheBytes -gt 16MB -or $s.inactiveCacheBytes -ne 0 -or $s.inactiveGpuImages -ne 0 -or $s.windowGpuReleaseErrors -ne 0){throw 'Shared view resource bounds failed'}
    return $s
}
function Select-Text([string]$Side){
    Focus $Side;Menu-Action 'Tampilan' 'Sesuaikan halaman';Page '1'
    $null=Wait-State {param($s) $s.activePage -eq 1 -and $s.textCacheBytes -gt 0}
    Start-Sleep -Milliseconds 300
    $r=(In-Pane $Side 'Isi halaman 1').Current.BoundingRectangle
    # Normalized glyph centers in page 1 of the generated text-selection fixture.
    $probe=[pscustomobject]@{firstX=0.11001587;firstY=0.5516052;lastX=0.35012817;lastY=0.5516052}
    $start=New-Object OrganizerClick+Point;$start.X=[int]($r.X+$r.Width*$probe.firstX);$start.Y=[int]($r.Y+$r.Height*$probe.firstY)
    $end=New-Object OrganizerClick+Point;$end.X=[int]($r.X+$r.Width*$probe.lastX);$end.Y=[int]($r.Y+$r.Height*$probe.lastY)
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$start)|Out-Null;[OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$end)|Out-Null
    function Text-Mouse($msg,$flags,$x,$y){$lp=[IntPtr](([int]$y -shl 16) -bor ([int]$x -band 0xffff));[OrganizerClick]::SendMessage($app.MainWindowHandle,$msg,[IntPtr]$flags,$lp)|Out-Null}
    Text-Mouse 0x200 0 $start.X $start.Y;Text-Mouse 0x201 1 $start.X $start.Y
    1..12|ForEach-Object {Text-Mouse 0x200 1 ($start.X+($end.X-$start.X)*$_/12) ($start.Y+($end.Y-$start.Y)*$_/12);Start-Sleep -Milliseconds 25}
    Text-Mouse 0x202 0 $end.X $end.Y
    $null=Wait-State {param($s) $s.selectedChars -eq 6}
}
try {
    $null=Wait-State {param($s) $s.pageCount -eq 4}
    Menu-Action 'Tampilan' 'Sesuaikan halaman';Page '2'
    Menu-Action 'Tampilan' 'Tampilan kedua PDF ini'
    $s=Wait-State {param($s) $s.splitView -and $s.tabCount -eq 1 -and $s.visibleViewIds.Count -eq 2 -and $s.visibleTabIds.Count -eq 1}
    $left=$s.viewId;$right=($s.sessions|Where-Object viewId -ne $left).viewId
    if(@($s.sessions|Where-Object documentActor).Count -ne 1){throw 'Native actor was duplicated'}
    Focus 'kanan';Page '4';Invoke 'Zoom';Invoke '150%'
    $null=Wait-State {param($s) $s.viewId -eq $right -and $s.activePage -eq 4 -and $s.zoomPercent -eq 150 -and ($s.sessions|Where-Object viewId -eq $left).activePage -eq 2}
    Focus 'kiri';Invoke 'Cari teks';$input=In-Pane 'kiri' 'Cari dalam PDF';$input.SetFocus();$input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('page 2')
    $null=Wait-State {param($s) $s.searchMatches -eq 1 -and -not $s.searchRunning}
    Focus 'kanan';Invoke 'Cari teks';$input=In-Pane 'kanan' 'Cari dalam PDF';$input.SetFocus();$input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('second line')
    $null=Wait-State {param($s) $s.searchMatches -eq 4 -and -not $s.searchRunning -and ($s.sessions|Where-Object viewId -eq $left).searchMatches -eq 1}
    Pass 'one-model-two-independent-reading-and-search-states'
    Select-Text 'kiri'
    $s=Read-Metrics;if(($s.sessions|Where-Object viewId -eq $right).selectedChars -ne 0){throw 'Text selection leaked into other view'}
    Select-Text 'kanan'
    $s=Read-Metrics;if(($s.sessions|Where-Object viewId -eq $left).selectedChars -ne 6){throw 'Other view text selection was lost'}
    Pass 'independent-text-selections-in-both-visible-views'
    Invoke 'Panel alat';Page '4';Click (In-Pane 'kanan' 'Halaman 4');Invoke 'Putar kanan'
    $null=Check-Shared 4 $true
    Focus 'kiri';Invoke 'Undo';$null=Check-Shared 4 $false
    Focus 'kanan';Page '4';Focus 'kiri';Page '4';Click (In-Pane 'kiri' 'Halaman 4');Invoke 'Hapus'
    $s=Check-Shared 3 $true
    if(($s.sessions|Where-Object viewId -eq $right).activePage -ne 3){throw 'Deleted-page fallback did not choose nearest surviving page'}
    Invoke 'Undo';$null=Check-Shared 4 $false
    Pass 'shared-selection-edit-history-and-deleted-page-fallback'
    Focus 'kanan';Picker 'Impor PDF';Fill-Picker $b;$null=Check-Shared 8 $true
    $c=Join-Path $directory 'C.pdf';Copy-Item $a $c
    Focus 'kiri';Picker 'Impor PDF';Fill-Picker $c;$s=Check-Shared 12 $true
    if(@($s.pagePlan|ForEach-Object {$_[0]}|Select-Object -Unique).Count -ne 3){throw 'Import source identities collided'}
    $null=Wait-State {param($s) -not ($s.sessions|Where-Object searchRunning) -and @($s.sessions|Where-Object {$_.searchScannedPages -ne 12}).Count -eq 0}
    Pass 'imports-from-both-views-have-unique-sources-and-refresh-search'
    Focus 'kanan'
    Menu-Action 'Alat' 'Ekspor PNG…';$null=Find 'Ekspor PNG';Invoke 'Batal'
    Menu-Action 'Alat' 'Split PDF…';$null=Find 'Split PDF';Invoke 'Batal'
    $null=Wait-State {param($s) $s.viewId -eq $right -and $s.splitView -and -not $s.exporting}
    Pass 'png-and-split-dialogs-from-secondary-share-document-actor'
    Focus 'kanan';Page '3'
    Picker 'Simpan sebagai';Fill-Picker (Join-Path $directory 'saved-shared.pdf')
    Invoke 'Tutup tampilan: A.pdf (kiri)'
    $s=Wait-State {param($s) -not $s.splitView -and $s.tabCount -eq 1 -and $s.viewId -eq $right -and $s.exportSuccess -and -not $s.dirty}
    if(@($s.sessions|Where-Object visible).Count -ne 1 -or $s.inactiveCacheBytes -ne 0){throw 'Closing a view leaked hidden cache'}
    $deadline=(Get-Date).AddSeconds(5)
    do {Start-Sleep -Milliseconds 200;$snapshot=Get-Content (Join-Path $stateDirectory 'session.json') -Raw|ConvertFrom-Json}while(($snapshot.documents.Count -ne 1 -or $snapshot.documents[0].page -ne 2) -and (Get-Date) -lt $deadline)
    if($snapshot.documents.Count -ne 1 -or $snapshot.documents[0].page -ne 2){throw 'Session metadata did not use the preferred secondary view'}
    Picker 'Buka PDF';Fill-Picker $a
    Start-Sleep -Milliseconds 700
    $null=Wait-State {param($s) -not $s.tabsOpening -and $s.tabCount -eq 1 -and $s.viewId -eq $right -and $s.pageCount -eq 12}
    Menu-Action 'Tampilan' 'Tampilan kedua PDF ini'
    $null=Wait-State {param($s) $s.visibleViewIds.Count -eq 2 -and $s.sessions.Count -eq 2}
    Focus 'kanan';$null=Wait-State {param($s) $s.activePage -eq 3 -and $s.viewId -eq $right}
    Pass 'secondary-save-survives-closing-actor-view-and-duplicate-open'
    Click (In-Pane 'kanan' 'Halaman 3');Invoke 'Putar kanan';$null=Check-Shared 12 $true
    Invoke 'Tutup tab: A.pdf';$null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Batal'
    $null=Wait-State {param($s) $s.splitView -and $s.tabCount -eq 1 -and $s.dirty -and -not $s.closingTabs}
    Invoke 'Tutup tab: A.pdf';$null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Buang perubahan'
    $null=Wait-State {param($s) $s.tabCount -eq 0}
    Pass 'document-close-prompts-once-and-cancel-preserves-both-views'
    Menu-Action 'Berkas' 'Buka ulang tab tertutup  Ctrl+Shift+T';$null=Wait-State {param($s) $s.tabCount -eq 1 -and $s.pageCount -eq 4 -and $s.activePage -eq 3 -and -not $s.dirty}
    Menu-Action 'Tampilan' 'Tampilan kedua PDF ini';$null=Wait-State {param($s) $s.visibleViewIds.Count -eq 2}
    Invoke 'Tutup tampilan: A.pdf (kiri)';$null=Wait-State {param($s) -not $s.splitView -and $s.tabCount -eq 1}
    Invoke 'Panel alat';Invoke 'Putar kanan';$null=Wait-State {param($s) $s.dirty}
    Invoke 'Tutup tab: A.pdf';$null=Find 'Simpan perubahan sebelum melanjutkan?'
    Invoke 'Simpan perubahan sebagai PDF';Fill-Picker (Join-Path $directory 'saved-on-close.pdf')
    $null=Wait-State {param($s) $s.tabCount -eq 0}
    Pass 'save-on-document-close-with-only-secondary-visible'
    Menu-Action 'Berkas' 'Buka ulang tab tertutup  Ctrl+Shift+T';$null=Wait-State {param($s) $s.pageCount -eq 4 -and $s.activePage -eq 3 -and -not $s.dirty}
    Menu-Action 'Tampilan' 'Tampilan kedua PDF ini';$null=Wait-State {param($s) $s.visibleViewIds.Count -eq 2}
    [IO.File]::SetLastWriteTimeUtc($a,[DateTime]::UtcNow.AddMinutes(1))
    Focus 'kanan';Invoke 'Zoom';Invoke '200%'
    $null=Find 'Halaman belum dapat ditampilkan';Invoke 'Tutup'
    $null=Wait-State {param($s) @($s.sessions|Where-Object {$_.pageCount -ne 0}).Count -eq 0 -and $s.totalPageCacheBytes -eq 0}
    Pass 'reopen-keeps-preferred-position-and-source-change-invalidates-both'
    if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $b).Hash -ne $hashB){throw 'Source PDF bytes changed'}
    Close-Window;if(-not $app.WaitForExit(15000)){throw 'Owned fixture did not close'}
    Write-Host "PASS same document UI: $directory"
} catch {Write-Host $_.ScriptStackTrace;if($app -and -not $app.HasExited){Capture 'failure.png'};throw}
