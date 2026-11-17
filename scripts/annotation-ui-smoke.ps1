param([ValidateSet('debug','release')][string]$Profile='debug',[string]$ExecutablePath)
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'annotation-ui' -IsolatedState -ExecutablePath $ExecutablePath
$cases=New-Object Collections.Generic.List[object]
function Pass([string]$Name){Capture ($Name+'.png');$cases.Add([pscustomobject]@{Case=$Name;Status='passed'});$cases.ToArray()|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'cases.json');Write-Host "PASS annotation: $Name"}
function Stroke($x1,$y1,$x2,$y2){
    $r=(Find 'Isi halaman 1').Current.BoundingRectangle
    $start=New-Object OrganizerClick+Point;$start.X=[int]($r.X+$r.Width*$x1);$start.Y=[int]($r.Y+$r.Height*$y1)
    $end=New-Object OrganizerClick+Point;$end.X=[int]($r.X+$r.Width*$x2);$end.Y=[int]($r.Y+$r.Height*$y2)
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$start)|Out-Null;[OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$end)|Out-Null
    function Mouse($msg,$flags,$x,$y){$lp=[IntPtr](([int]$y -shl 16) -bor ([int]$x -band 0xffff));[OrganizerClick]::SendMessage($app.MainWindowHandle,$msg,[IntPtr]$flags,$lp)|Out-Null}
    Mouse 0x200 0 $start.X $start.Y;Mouse 0x201 1 $start.X $start.Y
    1..12|ForEach-Object {Mouse 0x200 1 ($start.X+($end.X-$start.X)*$_/12) ($start.Y+($end.Y-$start.Y)*$_/12);Start-Sleep -Milliseconds 25}
    Mouse 0x202 0 $end.X $end.Y
}
try {
    $null=Wait-State {param($s) $s.pageCount -eq 4 -and -not $s.textLoading}
    Menu-Action 'Tampilan' 'Sesuaikan halaman'
    Invoke 'Anotasi';Invoke 'Pen';Invoke 'Pengaturan pen';Invoke 'Biru'
    $slider=Find 'Ketebalan'
    $r=$slider.Current.BoundingRectangle
    $point=New-Object OrganizerClick+Point;$point.X=[int]($r.Right-1);$point.Y=[int]($r.Y+$r.Height/2)
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$point)|Out-Null
    $lp=[IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x200,[IntPtr]0,$lp)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$lp)|Out-Null
    Start-Sleep -Milliseconds 300
    if((Find 'Ketebalan').GetCurrentPattern([Windows.Automation.RangeValuePattern]::Pattern).Current.Value -ne 12){throw 'Slider did not reach 12 pt'}
    $null=Wait-State {param($s) $s.annotationCount -eq 0 -and -not $s.dirty}
    Pass 'pen-options-preview';Invoke 'Pengaturan pen'
    Stroke 0.2 0.3 0.6 0.4
    $null=Wait-State {param($s) $s.annotationCount -eq 1 -and $s.dirty}
    Pass 'draw-and-dirty'
    Invoke 'Undo';$null=Wait-State {param($s) $s.annotationCount -eq 0 -and -not $s.dirty}
    Invoke 'Redo';$null=Wait-State {param($s) $s.annotationCount -eq 1 -and $s.dirty}
    Pass 'one-stroke-one-undo-redo'
    Invoke 'Highlighter';Stroke 0.2 0.45 0.6 0.45
    $null=Wait-State {param($s) $s.annotationCount -eq 2}
    Invoke 'Pengaturan highlighter';Invoke 'Hanya teks'
    $null=Wait-State {param($s) $s.annotationTextOnly}
    if((Find 'Ketebalan').Current.IsEnabled){throw 'Text highlight slider must be disabled'}
    Pass 'text-only-disables-width'
    Invoke 'Pengaturan highlighter';Stroke 0.11001587 0.5516052 0.35012817 0.5516052
    $null=Wait-State {param($s) $s.annotationCount -eq 3}
    Pass 'freehand-and-text-highlights'
    Menu-Action 'Tampilan' 'Tampilan kedua PDF ini'
    $null=Wait-State {param($s) $s.splitView -and $s.sessions.Count -eq 2 -and @($s.sessions|Where-Object {$_.annotationCount -ne 3}).Count -eq 0}
    Invoke 'Undo';$null=Wait-State {param($s) @($s.sessions|Where-Object {$_.annotationCount -ne 2}).Count -eq 0}
    Invoke 'Redo';$null=Wait-State {param($s) @($s.sessions|Where-Object {$_.annotationCount -ne 3}).Count -eq 0}
    Pass 'shared-annotations-and-history-across-two-views'
    Menu-Action 'Tampilan' 'Keluar tampilan berdampingan'
    $null=Wait-State {param($s) -not $s.splitView}
    Close-Window;$null=Find 'Batal';Invoke 'Batal'
    $null=Wait-State {param($s) $s.annotationCount -eq 3 -and $s.dirty -and $s.transition -eq 'Idle'}
    Pass 'unsaved-close-cancel-preserves-annotations'
    $saved=Join-Path $directory 'Annotated.pdf'
    $s=Save-Pdf 'Simpan sebagai' $saved
    if(-not $s.exportSuccess -or $s.dirty){throw 'Annotated Save As failed'}
    Picker 'Buka PDF';Fill-Picker $saved
    $null=Wait-State {param($s) $s.tabCount -eq 2 -and $s.annotationCount -eq 3 -and -not $s.dirty}
    Invoke 'Anotasi';Invoke 'Penghapus';Stroke 0.4 0.35 0.4 0.35
    $null=Wait-State {param($s) $s.annotationCount -eq 2 -and $s.dirty}
    Pass 'reopen-and-erase-persisted-ink'
    $removed=Join-Path $directory 'Removed.pdf';$s=Save-Pdf 'Simpan sebagai' $removed
    if(-not $s.exportSuccess -or $s.dirty){throw 'Deletion Save As failed'}
    Picker 'Buka PDF';Fill-Picker $removed
    $null=Wait-State {param($s) $s.tabCount -eq 3 -and $s.annotationCount -eq 2 -and -not $s.dirty}
    Pass 'save-and-reopen-deletion'
    if((Get-FileHash $a).Hash -ne $hashA){throw 'Source changed'}
    Close-Window
    Write-Host "Annotation evidence: $directory"
} catch {Capture 'failure.png';throw}
