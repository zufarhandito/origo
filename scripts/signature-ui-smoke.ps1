param([ValidateSet('debug','release')][string]$Profile='release',[string]$ExecutablePath)
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'signature-ui' -IsolatedState -ExecutablePath $ExecutablePath
$cases=New-Object Collections.Generic.List[object]
function Pass([string]$Name){Capture ($Name+'.png');$cases.Add([pscustomobject]@{Case=$Name;Status='passed'});$cases.ToArray()|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'cases.json');Write-Host "PASS signature: $Name"}
function Stroke($x1,$y1,$x2,$y2,[string]$Control='Isi halaman 1',$Node=$null){
    $r=if($Node){$Node.Current.BoundingRectangle}else{(Find $Control).Current.BoundingRectangle}
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
    Invoke 'Anotasi';Invoke 'Tanda tangan'
    Invoke 'Ketik'
    $field=Find 'Nama atau inisial tanda tangan';$field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('Zufar Handito')
    $null=Wait-State {param($s) $s.signatureAsset -and -not $s.dirty}
    Invoke 'Dancing Script';Invoke 'Biru';Pass 'typed-font-color-offline-preview'
    $field=Find 'Nama template tanda tangan';$field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('Pribadi')
    Invoke 'Simpan template';$null=Wait-State {param($s) -not $s.opening -and -not $s.dirty}
    $null=Find 'Pakai template: Pribadi';Pass 'local-template-save-no-dirty'
    Invoke 'Pakai template: Pribadi'
    $field=Find 'Nama template tanda tangan';$field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('Pribadi baru')
    Invoke 'Ubah nama template';$null=Find 'Pakai template: Pribadi baru'
    Invoke 'Pakai template: Pribadi baru';Invoke 'Hapus template';$null=Wait-State {param($s) -not $s.opening}
    Pass 'local-template-use-rename-delete'
    Invoke 'Ketik'

    Invoke 'Inisial';$field=Find 'Nama atau inisial tanda tangan'
    if($field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).Current.Value -ne 'ZH'){throw 'Initials suggestion missing'}
    $field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('ZAH')
    Invoke 'Gunakan tanda tangan';Stroke 0.4 0.4 0.4 0.4
    $null=Wait-State {param($s) $s.signatureCount -eq 1 -and $s.dirty}
    Invoke 'Undo';$null=Wait-State {param($s) $s.signatureCount -eq 0 -and -not $s.dirty}
    Pass 'initials-edit-place-undo'
    Invoke 'Tanda tangan';Invoke 'Tanda tangan penuh';Invoke 'Gambar';Invoke 'Hapus semua'
    if((Find 'Gunakan tanda tangan').Current.IsEnabled){throw 'Empty drawing must be disabled'}
    Invoke 'Dari gambar';Invoke 'Gambar'
    Invoke 'Batal tanda tangan';$null=Wait-State {param($s) -not $s.signaturePanel -and -not $s.dirty}
    Invoke 'Tanda tangan';$null=Find 'Area gambar tanda tangan'
    Pass 'editor-methods-cancel-no-dirty'
    Stroke 0.15 0.4 0.75 0.7 'Area gambar tanda tangan'
    $null=Wait-State {param($s) $s.signatureStrokes -eq 1 -and -not $s.dirty}
    Invoke 'Undo goresan';$null=Wait-State {param($s) $s.signatureStrokes -eq 0}
    Stroke 0.15 0.4 0.75 0.7 'Area gambar tanda tangan'
    Stroke 0.2 0.7 0.65 0.3 'Area gambar tanda tangan'
    Pass 'draw-pad-empty-undo-preview-without-dirty'
    Invoke 'Gunakan tanda tangan'
    $null=Wait-State {param($s) $s.signaturePlacing -and -not $s.signaturePanel -and -not $s.dirty}
    Stroke 0.4 0.4 1.1 0.5
    $null=Wait-State {param($s) $s.signatureCount -eq 0 -and -not $s.dirty}
    Pass 'drop-outside-page-cancels-placement'
    Invoke 'Tanda tangan';Invoke 'Gunakan tanda tangan'
    Stroke 0.4 0.4 0.5 0.5
    $s=Wait-State {param($s) $s.signatureCount -eq 1 -and $s.dirty}
    Pass 'draw-place-drag-one-object'
    Invoke 'Duplikat tanda tangan';$null=Wait-State {param($s) $s.signatureCount -eq 2}
    Invoke 'Undo';$s=Wait-State {param($s) $s.signatureCount -eq 1}
    Pass 'duplicate-signature-single-undo'

    $before=$s.signatureRects[0]|ConvertTo-Json -Compress
    $r=$s.signatureRects[0];$x=($r[0][0]+$r[1][0])/2;$y=($r[0][1]+$r[1][1])/2
    Stroke $x $y ($x+0.12) ($y+0.12)
    $s=Wait-State {param($s) ($s.signatureRects[0]|ConvertTo-Json -Compress) -ne $before}
    Invoke 'Undo';$null=Wait-State {param($s) ($s.signatureRects[0]|ConvertTo-Json -Compress) -eq $before}
    Invoke 'Redo';$s=Wait-State {param($s) ($s.signatureRects[0]|ConvertTo-Json -Compress) -ne $before}
    Pass 'move-single-undo-redo'
    $r=$s.signatureRects[0];$width=$r[1][0]-$r[0][0]
    Stroke $r[1][0] $r[1][1] ($r[1][0]+0.1) ($r[1][1]+0.05)
    $s=Wait-State {param($s) ($s.signatureRects[0][1][0]-$s.signatureRects[0][0][0]) -gt $width+0.05}
    Pass 'proportional-corner-resize'
    $beforeNudge=$s.signatureRects[0][0][0]
    [Windows.Forms.SendKeys]::SendWait('%{RIGHT}')
    $null=Wait-State {param($s) $s.signatureRects[0][0][0] -gt $beforeNudge}
    Invoke 'Undo';$s=Wait-State {param($s) [Math]::Abs($s.signatureRects[0][0][0]-$beforeNudge) -lt 0.001}
    Pass 'keyboard-nudge-single-undo'

    $png=Join-Path $directory 'signature.png'
    $bitmap=New-Object Drawing.Bitmap 160,160;$graphics=[Drawing.Graphics]::FromImage($bitmap);$graphics.Clear([Drawing.Color]::White)
    $pen=New-Object Drawing.Pen ([Drawing.Color]::FromArgb(180,20,70,180)),4
    $graphics.DrawEllipse($pen,10,10,140,140);$graphics.DrawLine($pen,30,80,130,80);$bitmap.Save($png,[Drawing.Imaging.ImageFormat]::Png)
    $pen.Dispose();$graphics.Dispose();$bitmap.Dispose()
    Invoke 'Tanda tangan'
    Invoke 'Stempel'
    $bad=Join-Path $directory 'bad.png';[IO.File]::WriteAllText($bad,'invalid image')
    Picker 'Impor gambar';Fill-Picker $bad
    $null=Wait-State {param($s) $s.signatureError -and $s.signatureCount -eq 1}
    Pass 'invalid-image-keeps-document'
    Picker 'Impor gambar';Fill-Picker $png
    $null=Wait-State {param($s) $s.signatureAsset -and -not $s.signatureError}
    Invoke 'Hapus latar putih';$null=Wait-State {param($s) $s.signatureAsset -and -not $s.signatureError}
    Pass 'stamp-square-white-removal-preview'
    Invoke 'Gunakan tanda tangan';Stroke 0.4 0.7 0.4 0.7
    $null=Wait-State {param($s) $s.signatureCount -eq 2}
    Menu-Action 'Tampilan' 'Tampilan kedua PDF ini'
    $null=Wait-State {param($s) $s.splitView -and @($s.sessions|Where-Object {$_.signatureCount -ne 2}).Count -eq 0}
    Pass 'shared-signatures-two-views'
    $right=Find 'Panel PDF: A.pdf (kanan)'
    # Annotation trigger now belongs to the shared main toolbar. Focus the right
    # view first so that this toolbar edits that view's contextual tool state.
    $condition=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Isi halaman 1')
    $rightPage=$right.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)
    Stroke 0.03 0.03 0.03 0.03 'Isi halaman 1' $rightPage
    $null=Wait-State {param($s) $s.viewId -eq 3}
    Invoke 'Anotasi'
    $condition=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Pen')
    $right.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition).GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
    $condition=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Isi halaman 1')
    $rightPage=$right.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)
    Invoke 'Tanda tangan'
    Stroke 0.4 0.4 0.6 0.5 'Isi halaman 1' $rightPage
    $null=Wait-State {param($s) @($s.sessions|Where-Object {$_.annotationCount -ne 2}).Count -eq 0}
    Pass 'signature-popup-blocks-drawing-in-other-pane'
    $s=Wait-State {param($s) $s.annotationCount -eq 2}
    if(@($s.sessions|Where-Object signaturePanel).Count -gt 0){Invoke 'Tanda tangan'}
    Menu-Action 'Tampilan' 'Keluar tampilan berdampingan'
    Close-Window;Invoke 'Batal';$null=Wait-State {param($s) $s.signatureCount -eq 2 -and $s.dirty}
    $saved=Join-Path $directory 'Signed.pdf';$s=Save-Pdf 'Simpan sebagai' $saved
    if(-not $s.exportSuccess -or $s.dirty){throw 'Signature Save As failed'}
    Picker 'Buka PDF';Fill-Picker $saved
    $s=Wait-State {param($s) $s.tabCount -eq 2 -and $s.signatureCount -eq 2 -and -not $s.dirty}
    Pass 'close-guard-save-reopen-editable-signatures'
    Menu-Action 'Tampilan' 'Sesuaikan halaman'
    Invoke 'Anotasi';Invoke 'Pilih'
    $r=$s.signatureRects[0];Stroke (($r[0][0]+$r[1][0])/2) (($r[0][1]+$r[1][1])/2) (($r[0][0]+$r[1][0])/2) (($r[0][1]+$r[1][1])/2)
    Invoke 'Hapus anotasi';$null=Wait-State {param($s) $s.signatureCount -eq 1 -and $s.dirty}
    $removed=Join-Path $directory 'Removed.pdf';$s=Save-Pdf 'Simpan sebagai' $removed
    if(-not $s.exportSuccess -or $s.dirty){throw 'Signature deletion Save As failed'}
    Picker 'Buka PDF';Fill-Picker $removed
    $null=Wait-State {param($s) $s.signatureCount -eq 1 -and -not $s.dirty}
    Pass 'reopen-delete-save-reopen'
    if((Get-FileHash $a).Hash -ne $hashA){throw 'Source changed'}
    Close-Window
    if(-not $app.WaitForExit(3000)){
        # Only the synthetic fixture process owned by this suite may be cleaned up.
        $owned=Get-CimInstance Win32_Process -Filter "ProcessId=$($app.Id)"
        if($owned.ExecutablePath -ne $exe -or -not $owned.CommandLine.Contains($directory)){throw 'Refusing cleanup of non-test process'}
        Stop-Process -Id $app.Id
    }
    Write-Host "Signature evidence: $directory"
} catch {Capture 'failure.png';throw}
