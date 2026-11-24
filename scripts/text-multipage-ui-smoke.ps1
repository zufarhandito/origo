param([ValidateSet('debug','release')][string]$Profile='debug',[string]$ExecutablePath)
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'text-multipage-ui' -IsolatedState -ExecutablePath $ExecutablePath
$cases=New-Object Collections.Generic.List[object]
function Pass([string]$Name){Capture ($Name+'.png');$cases.Add([pscustomobject]@{Case=$Name;Status='passed'});$cases.ToArray()|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'cases.json');Write-Host "PASS text-multipage: $Name"}
function Pointer-Control([string]$Name,[double]$X=0.5,[double]$Y=0.5,[int]$Message=0x200,[int]$Buttons=0) {
    $rect=(Find $Name).Current.BoundingRectangle
    $point=New-Object OrganizerClick+Point
    $point.X=[int]($rect.X+$rect.Width*$X)
    $point.Y=[int]($rect.Y+$rect.Height*$Y)
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$point)|Out-Null
    $pos=[IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
    [OrganizerClick]::SendMessage($app.MainWindowHandle,$Message,[IntPtr]$Buttons,$pos)|Out-Null
}
function Click-Page([double]$X,[double]$Y) {
    Pointer-Control 'Isi halaman 1' $X $Y 0x200 0
    Start-Sleep -Milliseconds 80
    Pointer-Control 'Isi halaman 1' $X $Y 0x201 1
    Start-Sleep -Milliseconds 60
    Pointer-Control 'Isi halaman 1' $X $Y 0x202 0
}
try {
    $null=Wait-State {param($s) $s.pageCount -eq 4 -and -not $s.textLoading}
    Menu-Action 'Tampilan' 'Sesuaikan halaman'
    Invoke 'Anotasi'
    Invoke 'Teks'
    $null=Wait-State {param($s) $s.textAnnotationPanel -and -not $s.dirty}
    Invoke 'Tanggal'
    $field=Find 'Isi anotasi teks'
    $dateValue=$field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).Current.Value
    if([string]::IsNullOrWhiteSpace($dateValue)){throw 'Tanggal preset did not fill text input'}
    if(-not (Find 'Tempatkan Teks').Current.IsEnabled){throw 'Tempatkan Teks must be enabled after Tanggal preset'}
    Pass 'text-date-preset-preview'

    Invoke 'Tempatkan Teks'
    $null=Wait-State {param($s) $s.textAnnotationPlacing -and -not $s.textAnnotationPanel -and -not $s.dirty}
    Pointer-Control 'Isi halaman 1' 0.35 0.35
    $null=Wait-State {param($s) $s.signatureGhost -and -not $s.dirty}
    Click-Page 0.35 0.35
    $placed=Wait-State {param($s) $s.textAnnotationCount -eq 1 -and $s.annotationCount -eq 1 -and $s.dirty -and $s.annotationSelected -and -not $s.textAnnotationPlacing}
    Pass 'text-ghost-place'

    Invoke 'Edit teks'
    $null=Wait-State {param($s) $s.textAnnotationPanel}
    $field=Find 'Isi anotasi teks'
    $field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('Budi Santoso')
    Invoke '18 pt'
    Invoke 'Warna teks Biru'
    Invoke 'Simpan Perubahan'
    $edited=Wait-State {param($s) -not $s.textAnnotationPanel -and $s.textAnnotationCount -eq 1 -and $s.revision -gt $placed.revision}
    Pass 'text-edit-in-place'

    Invoke 'Terapkan ke halaman'
    $null=Wait-State {param($s) $s.applyPagesPanel}
    Invoke 'Rentang'
    $rangeField=Find 'Rentang halaman tujuan'
    $rangeField.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('2-4')
    Start-Sleep -Milliseconds 150
    Invoke 'Terapkan'
    $null=Wait-State {param($s) -not $s.applyPagesPanel -and $s.textAnnotationCount -eq 4}
    Invoke 'Undo'
    $null=Wait-State {param($s) $s.textAnnotationCount -eq 1}
    Invoke 'Redo'
    $null=Wait-State {param($s) $s.textAnnotationCount -eq 4}
    Pass 'text-apply-range-atomic-undo-redo'

    Invoke 'Tanda tangan'
    Invoke 'Ketik'
    $sigField=Find 'Nama atau inisial tanda tangan'
    $sigField.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('Zufar')
    $null=Wait-State {param($s) $s.signatureAsset}
    Invoke 'Gunakan Tanda Tangan'
    $null=Wait-State {param($s) $s.signaturePlacing -and -not $s.signaturePanel}
    Click-Page 0.65 0.72
    $null=Wait-State {param($s) $s.signatureCount -eq 1 -and $s.annotationSelected}
    Invoke 'Terapkan ke halaman'
    $null=Wait-State {param($s) $s.applyPagesPanel}
    Invoke 'Terapkan'
    $null=Wait-State {param($s) -not $s.applyPagesPanel -and $s.signatureCount -eq 4}
    Invoke 'Undo'
    $null=Wait-State {param($s) $s.signatureCount -eq 1}
    Invoke 'Redo'
    $null=Wait-State {param($s) $s.signatureCount -eq 4}
    Pass 'signature-apply-all-pages-atomic-undo-redo'

    $saved=Join-Path $directory 'TextAndMultiPage.pdf'
    $s=Save-Pdf 'Simpan sebagai' $saved
    if(-not $s.exportSuccess -or $s.dirty){throw 'Save As failed for text + multipage stamps'}
    Picker 'Buka PDF';Fill-Picker $saved
    $null=Wait-State {param($s) $s.tabCount -eq 2 -and $s.textAnnotationCount -eq 4 -and $s.signatureCount -eq 4 -and -not $s.dirty}
    Pass 'save-and-reopen-text-and-multipage-stamps'

    if((Get-FileHash $a).Hash -ne $hashA){throw 'Source changed'}
    Close-Window
    Write-Host "Text+MultiPage evidence: $directory"
} catch {Capture 'failure.png';throw}
