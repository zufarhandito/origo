param([ValidateSet('debug','release')][string]$Profile='release')
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'comparison-ui' -IsolatedState
$cases=New-Object Collections.Generic.List[object]
function Pass([string]$Name){if($app -and -not $app.HasExited){Capture ($Name+'.png')};$cases.Add([pscustomobject]@{Case=$Name;Status='passed'});$cases.ToArray()|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'cases.json');Write-Host "PASS comparison: $Name"}
function Node-InPane([string]$Name,[string]$Control){
    $deadline=(Get-Date).AddSeconds(8)
    do {$pane=Find ('Panel PDF: '+$Name);$node=$pane.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Control));if($node){return $node};Start-Sleep -Milliseconds 100}while((Get-Date) -lt $deadline)
    throw "Missing $Control in pane $Name"
}
function Click-Node($Node){
    $rect=$Node.Current.BoundingRectangle;Add-Content (Join-Path $directory 'coordinates.txt') ('Click '+$Node.Current.Name+' '+$rect.ToString());$point=New-Object OrganizerClick+Point
    if($rect.Width -le 0 -or $rect.Height -le 0){throw 'Cannot click an unrealized fixture control'}
    $point.X=[int]($rect.X+$rect.Width/2);$point.Y=[int]($rect.Y+$rect.Height/2)
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$point)|Out-Null
    $packed=[IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x200,[IntPtr]0,$packed)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$packed)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$packed)|Out-Null
}
function Focus-Pane([string]$Name){
    $rect=(Find ('Panel PDF: '+$Name)).Current.BoundingRectangle;$point=New-Object OrganizerClick+Point
    Add-Content (Join-Path $directory 'coordinates.txt') ('Focus '+$Name+' '+$rect.ToString())
    if($rect.Width -lt 300 -or $rect.Height -lt 100){throw 'Cannot focus an unrealized PDF pane'}
    $point.X=[int]($rect.X+60);$point.Y=[int]($rect.Y+12)
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$point)|Out-Null
    $packed=[IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x200,[IntPtr]0,$packed)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$packed)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$packed)|Out-Null
    Start-Sleep -Milliseconds 100
}
function Key([int]$Code){[OutputDialog]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]$Code,[IntPtr]0x10001)|Out-Null;[OutputDialog]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]$Code,[IntPtr]0xC0010001)|Out-Null;Start-Sleep -Milliseconds 150}
function Page([string]$Number){$field=Find 'Nomor halaman tujuan';$field.SetFocus();$field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue($Number);Key 13}
function Check-Bounds {
    $s=Read-Metrics
    if($s.totalPageCacheBytes -gt 128MB -or $s.totalThumbnailCacheBytes -gt 16MB -or $s.totalTextCacheBytes -gt 16MB -or $s.inactiveCacheBytes -ne 0 -or $s.inactiveGpuImages -ne 0 -or $s.windowGpuReleaseErrors -ne 0 -or $s.nativeActiveDocuments -gt 1 -or ($s.sessions|Measure-Object queue -Sum).Sum -gt 32){throw 'Comparison resource bounds failed'}
}
try {
    $s=Wait-State {param($s) $s.tabCount -eq 1};$idA=$s.tabId
    Menu-Action 'Tampilan' 'Sesuaikan halaman';Page '2'
    $null=Wait-State {param($s) $s.activePage -eq 2}
    Picker 'Buka PDF';Fill-Picker $b
    $s=Wait-State {param($s) $s.tabCount -eq 2};$idB=$s.tabId
    Menu-Action 'Tampilan' 'Sesuaikan halaman';Page '3'
    $null=Wait-State {param($s) $s.activePage -eq 3}
    $c=Join-Path $directory 'C.pdf';Copy-Item $a $c
    Picker 'Buka PDF';Fill-Picker $c
    $null=Wait-State {param($s) $s.tabCount -eq 3}
    Switch-Tab $a
    Menu-Action 'Tampilan' 'Bandingkan dua PDF…';Invoke $b
    $null=Wait-State {param($s) $s.splitView -and $s.visibleTabIds.Count -eq 2 -and $s.tabId -eq $idA -and $s.activePage -eq 2}
    $null=Find 'Panel PDF: A.pdf';$null=Find 'Panel PDF: B.pdf'
    Check-Bounds
    Pass 'two-visible-panels-and-hidden-tab-resource-release'
    Focus-Pane 'B.pdf'
    $null=Wait-State {param($s) $s.tabId -eq $idB -and $s.activePage -eq 3 -and $s.visibleTabIds.Count -eq 2}
    Page '4'
    $null=Wait-State {param($s) $s.activePage -eq 4 -and ($s.sessions|Where-Object tabId -eq $idA).activePage -eq 2}
    Focus-Pane 'A.pdf';Invoke 'Cari teks'
    $input=Node-InPane 'A.pdf' 'Cari dalam PDF';$input.SetFocus();$input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('page 2')
    $null=Wait-State {param($s) ($s.sessions|Where-Object tabId -eq $idA).searchMatches -eq 1 -and -not ($s.sessions|Where-Object tabId -eq $idA).searchRunning}
    Focus-Pane 'B.pdf';Invoke 'Cari teks'
    $input=Node-InPane 'B.pdf' 'Cari dalam PDF';$input.SetFocus();$input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('second line')
    $null=Wait-State {param($s) $s.searchMatches -eq 4 -and -not $s.searchRunning -and ($s.sessions|Where-Object tabId -eq $idA).searchMatches -eq 1}
    Pass 'focus-routing-and-independent-reading-search'
    # Text-field Delete must not delete PDF pages, including organizer mode.
    Invoke 'Panel alat'
    $null=Find 'Dokumen: B.pdf'
    $input=Node-InPane 'B.pdf' 'Cari dalam PDF';$input.SetFocus();$input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('page');Key 36;Key 46
    $null=Wait-State {param($s) $s.pageCount -eq 4 -and -not $s.dirty}
    if($input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).Current.Value -ne 'age'){throw 'Delete did not edit focused text input'}
    Focus-Pane 'A.pdf'
    $null=Find 'Dokumen: A.pdf'
    Click-Node (Node-InPane 'A.pdf' 'Halaman 2')
    Invoke 'Putar kanan'
    $null=Wait-State {param($s) $s.tabId -eq $idA -and $s.dirty -and ($s.sessions|Where-Object tabId -eq $idB).selectedPages -eq 1 -and -not ($s.sessions|Where-Object tabId -eq $idB).dirty}
    Invoke 'Undo'
    $null=Wait-State {param($s) -not $s.dirty}
    Pass 'one-organizer-per-document-selection-history-and-input-focus'
    # Check the real Dock splitter while held down as well as after release.
    $left=(Find 'Panel PDF: A.pdf').Current.BoundingRectangle
    $point=New-Object OrganizerClick+Point;$point.X=[int]$left.Right;$point.Y=[int]($left.Y+150)
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$point)|Out-Null
    $originX=$point.X;$originY=$point.Y
    function Mouse($message,$flags,$x,$y){$packed=[IntPtr](([int]$y -shl 16) -bor ([int]$x -band 0xffff));[OutputDialog]::PostMessage($app.MainWindowHandle,$message,[IntPtr]$flags,$packed)|Out-Null;Start-Sleep -Milliseconds 50}
    Mouse 0x200 0 $originX $originY;Mouse 0x201 1 $originX $originY
    Add-Content (Join-Path $directory 'coordinates.txt') ('Resize start '+$originX+' '+((Read-Metrics).splitGeometry|ConvertTo-Json -Compress))
    1..8|ForEach-Object {Mouse 0x200 1 ($originX-$_*35) $originY}
    Start-Sleep -Milliseconds 600
    Add-Content (Join-Path $directory 'coordinates.txt') ('Resize held '+((Read-Metrics).splitGeometry|ConvertTo-Json -Compress)+' '+(Find 'Panel PDF: A.pdf').Current.BoundingRectangle.ToString())
    if((Find 'Panel PDF: A.pdf').Current.BoundingRectangle.Width -lt 315 -or (Find 'Panel PDF: B.pdf').Current.BoundingRectangle.Width -lt 315){throw 'Dock panel became too small during drag'}
    Mouse 0x202 0 ($originX-280) $originY
    Start-Sleep -Milliseconds 500
    if([Math]::Abs((Find 'Panel PDF: A.pdf').Current.BoundingRectangle.Width-$left.Width) -lt 50){throw 'Dock splitter did not actually move'}
    if((Find 'Panel PDF: A.pdf').Current.BoundingRectangle.Width -lt 315 -or (Find 'Panel PDF: B.pdf').Current.BoundingRectangle.Width -lt 315){throw 'Dock minimum panel width failed'}
    Check-Bounds
    Pass 'dock-splitter-minimum-and-window-cache-bounds'
    $before=Read-Metrics
    $planA=($before.sessions|Where-Object tabId -eq $idA).pagePlan|ConvertTo-Json -Compress
    $planB=($before.sessions|Where-Object tabId -eq $idB).pagePlan|ConvertTo-Json -Compress
    $points=@()
    foreach($name in @('A.pdf','B.pdf')){
        $rect=(Node-InPane $name 'Halaman 2').Current.BoundingRectangle
        $point=New-Object OrganizerClick+Point;$point.X=[int]($rect.X+$rect.Width/2);$point.Y=[int]($rect.Y+$rect.Height/2)
        [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$point)|Out-Null;$points+=$point
    }
    Mouse 0x200 0 $points[0].X $points[0].Y;Mouse 0x201 1 $points[0].X $points[0].Y
    1..12|ForEach-Object {Mouse 0x200 1 ($points[0].X+($points[1].X-$points[0].X)*$_/12) ($points[0].Y+($points[1].Y-$points[0].Y)*$_/12)}
    Mouse 0x202 0 $points[1].X $points[1].Y
    $after=Wait-State {param($s) -not $s.dirty -and -not ($s.sessions|Where-Object tabId -eq $idA).pageDragging}
    if((($after.sessions|Where-Object tabId -eq $idA).pagePlan|ConvertTo-Json -Compress) -ne $planA -or (($after.sessions|Where-Object tabId -eq $idB).pagePlan|ConvertTo-Json -Compress) -ne $planB){throw 'Cross-document drag changed a PDF plan'}
    Pass 'cross-document-page-drag-is-rejected'
    Add-Type 'using System;using System.Runtime.InteropServices;public static class ComparisonWindow{[DllImport("user32.dll")]public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int height,bool repaint);}'
    $windowRect=New-Object M3Capture+Rect;[M3Capture]::GetWindowRect($app.MainWindowHandle,[ref]$windowRect)|Out-Null
    [ComparisonWindow]::MoveWindow($app.MainWindowHandle,$windowRect.L,$windowRect.T,860,600,$true)|Out-Null
    Menu-Action 'Tampilan' 'Gelap'
    Focus-Pane 'B.pdf';$null=Find 'Dokumen: B.pdf'
    $null=Wait-State {param($s) $s.splitView -and $s.visibleTabIds.Count -eq 2}
    Check-Bounds;Pass 'narrow-dark-organizer-drawer-and-focus'
    [ComparisonWindow]::MoveWindow($app.MainWindowHandle,$windowRect.L,$windowRect.T,1200,900,$true)|Out-Null
    Menu-Action 'Tampilan' 'Terang'
    Focus-Pane 'A.pdf'
    Menu-Action 'Tampilan' 'Keluar tampilan berdampingan'
    $null=Wait-State {param($s) -not $s.splitView -and $s.tabId -eq $idA -and $s.visibleTabIds.Count -eq 1}
    Menu-Action 'Tampilan' 'Bandingkan dua PDF…';Invoke $b
    $null=Wait-State {param($s) $s.splitView -and $s.visibleTabIds.Count -eq 2}
    Pass 'exit-and-reenter-split-keeps-both-documents'
    Focus-Pane 'A.pdf';Invoke 'Putar kanan'
    $null=Wait-State {param($s) $s.dirty}
    Invoke 'Tutup tab: A.pdf'
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Batal'
    $null=Wait-State {param($s) $s.splitView -and $s.tabCount -eq 3 -and $s.dirty -and -not $s.closingTabs}
    Focus-Pane 'B.pdf';Invoke 'Putar kiri'
    $null=Wait-State {param($s) $s.dirty}
    Focus-Pane 'A.pdf'
    Picker 'Simpan sebagai';Fill-Picker (Join-Path $directory 'saved-A.pdf')
    Focus-Pane 'B.pdf'
    $null=Wait-State {param($s) $s.tabId -eq $idB -and $s.dirty -and -not ($s.sessions|Where-Object tabId -eq $idA).dirty -and ($s.sessions|Where-Object tabId -eq $idA).exportSuccess}
    Pass 'dirty-close-cancel-and-export-receipt-keeps-origin'
    # A changed source in A must leave B usable and dirty.
    [IO.File]::SetLastWriteTimeUtc($a,[DateTime]::UtcNow.AddMinutes(1))
    Focus-Pane 'A.pdf';Invoke 'Zoom';Invoke '150%'
    $null=Find 'Halaman belum dapat ditampilkan';Invoke 'Tutup'
    Focus-Pane 'B.pdf'
    $null=Wait-State {param($s) $s.tabId -eq $idB -and $s.pageCount -eq 4 -and $s.dirty -and ($s.sessions|Where-Object tabId -eq $idA).pageCount -eq 0}
    Pass 'changed-source-in-one-pane-does-not-damage-other-document'
    Invoke 'Tutup tab: A.pdf'
    $null=Wait-State {param($s) -not $s.splitView -and $s.tabCount -eq 2 -and $s.tabId -eq $idB -and $s.dirty}
    Capture 'remaining-document.png'
    Invoke 'Tutup tab: B.pdf'
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Buang perubahan'
    $null=Wait-State {param($s) $s.tabCount -eq 1 -and -not $s.dirty}
    Close-Window
    if(-not $app.WaitForExit(15000)){throw 'Fixture did not close'}
    if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $b).Hash -ne $hashB -or (Get-FileHash $c).Hash -ne $hashA){throw 'Source PDF bytes changed'}
    Pass 'close-document-leaves-other-view-and-source-bytes-intact'
    Write-Host "PASS comparison UI: $directory"
} catch {Write-Host $_.ScriptStackTrace;if($app -and -not $app.HasExited){Capture 'failure.png'};throw}
