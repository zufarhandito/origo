param([string]$ExecutablePath='target/tab-background-check/release/pdf-studio.exe')
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile release -TestName 'signature-library-ui' -IsolatedState -ExecutablePath $ExecutablePath
Add-Type 'using System;using System.Runtime.InteropServices;public static class SignatureLibraryTestFocus{[DllImport("user32.dll")]public static extern IntPtr GetForegroundWindow();}'
function Control-Exists([string]$Name) {
 $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
 $condition=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Name)
 return $null -ne $ui.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)
}
function Set-Field([string]$Name,[string]$Value) {
 (Find $Name).GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue($Value)
}
function Press-Key([string]$Key) {
 if([SignatureLibraryTestFocus]::GetForegroundWindow() -ne $app.MainWindowHandle){throw 'The test window must be in front for inline rename keys'}
 [Windows.Forms.SendKeys]::SendWait($Key)
 Start-Sleep -Milliseconds 150
}
function Double-Click([string]$Name) {
 $rect=(Find $Name).Current.BoundingRectangle
 $point=New-Object OrganizerClick+Point
 $point.X=[int]($rect.X+$rect.Width/2);$point.Y=[int]($rect.Y+$rect.Height/2)
 [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$point)|Out-Null
 $pos=[IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
 for($i=0;$i -lt 2;$i++){
  [OrganizerClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$pos)|Out-Null
  [OrganizerClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$pos)|Out-Null
  Start-Sleep -Milliseconds 60
 }
}
function Save-Template([string]$Text,[string]$Label,[string]$ExpectedLabel) {
 Invoke 'Ketik';Set-Field 'Nama atau inisial tanda tangan' $Text
 $null=Wait-State {param($s) $s.signatureAsset -and -not $s.dirty}
 if(Control-Exists 'Nama template tanda tangan'){throw 'Template name visible before opt-in'}
 Invoke 'Simpan ke template lokal';$null=Find 'Nama template tanda tangan'
 Set-Field 'Nama template tanda tangan' $Label
 Invoke 'Gunakan Tanda Tangan'
 $null=Wait-State {param($s) $s.signaturePlacing -and -not $s.signaturePanel -and -not $s.dirty}
 Invoke 'Tanda tangan';$null=Find "Pakai template: $ExpectedLabel"
 $null=Wait-State {param($s) -not $s.signaturePlacing -and $s.signatureCount -eq 0}
 if(Control-Exists 'Simpan ke template lokal'){throw 'Creation controls leaked into gallery'}
}
function Finish-Owned {
 Close-Window
 if(-not $app.WaitForExit(3000)){
  $owned=Get-CimInstance Win32_Process -Filter "ProcessId=$($app.Id)"
  if($owned.ExecutablePath -ne $exe -or -not $owned.CommandLine.Contains($directory)){throw 'Not an owned fixture'}
  Stop-Process -Id $app.Id
 }
}
try{
 $null=Wait-State {param($s) $s.pageCount -gt 0}
 Invoke 'Anotasi';Invoke 'Tanda tangan';$null=Find 'Area gambar tanda tangan'
 if(Control-Exists 'Kembali ke Tersimpan'){throw 'Empty library must open the creation form'}
 Save-Template 'Nama Pribadi' 'Template lokal' 'Template lokal'
 Invoke 'Buat Baru';$null=Find 'Kembali ke Tersimpan'
 Invoke 'Kembali ke Tersimpan';$null=Find 'Pakai template: Template lokal'
 Invoke 'Buat Baru';Save-Template 'Tanda Pertama' '' 'Tanda Tangan 1'
 Invoke 'Buat Baru';Save-Template 'Tanda Kedua' '' 'Tanda Tangan 2'
 $null=Find 'Tersimpan (3)'
 $first,$second,$third=@('Template lokal','Tanda Tangan 1','Tanda Tangan 2') | ForEach-Object {
  (Find "Pakai template: $_").Current.BoundingRectangle
 } | Sort-Object Y,X
 if($first.X -ge $second.X -or [Math]::Abs($first.Y-$second.Y) -gt 2 -or $third.Y -le $first.Y){throw 'Gallery is not a two-column grid'}
 if((Find 'Gunakan Tanda Tangan').Current.IsEnabled){throw 'Gallery footer enabled without a selection'}
 Capture 'gallery-two-columns-autonaming.png'
 Write-Host 'PASS signature library: smart entry, two-column gallery, opt-in save and autonaming'
 Invoke 'Batal tanda tangan'
 Finish-Owned
 $app=Start-Process -FilePath $exe -ArgumentList ('"'+$a+'" --metrics "'+$metrics+'" --state-dir "'+$stateDirectory+'"') -WorkingDirectory (Split-Path $exe) -WindowStyle Normal -PassThru
 $end=(Get-Date).AddSeconds(20);do{Start-Sleep -Milliseconds 200;$app.Refresh()}while($app.MainWindowHandle -eq 0 -and (Get-Date)-lt $end)
 $null=Wait-State {param($s) $s.pageCount -gt 0}
 Invoke 'Anotasi';Invoke 'Tanda tangan';$null=Find 'Pakai template: Template lokal';Capture 'after-restart.png'
 Invoke 'Opsi template: Template lokal';Invoke 'Ubah nama'
 $rename=Find 'Nama template: Template lokal'
 if(-not $rename.Current.HasKeyboardFocus){throw 'Rename input was not focused automatically'}
 Set-Field 'Nama template: Template lokal' 'Discard rename';Press-Key '{ESC}'
 $null=Find 'Pakai template: Template lokal'
 if((Control-Exists 'Nama template: Template lokal') -or (Control-Exists 'Pakai template: Discard rename')){throw 'Escape did not cancel inline rename'}
 Write-Host 'PASS signature library: restart and inline rename cancellation'
 Invoke 'Opsi template: Template lokal';Invoke 'Ubah nama'
 Set-Field 'Nama template: Template lokal' 'Nama baru';Press-Key '{ENTER}'
 $null=Find 'Pakai template: Nama baru'
 Invoke 'Opsi template: Nama baru';Invoke 'Ubah nama'
 Set-Field 'Nama template: Nama baru' 'Nama akhir';Click-Control 'Pakai template: Tanda Tangan 1'
 $null=Find 'Pakai template: Nama akhir'
 Capture 'rename-inline-esc-enter-blur.png'
 Write-Host 'PASS signature library: inline rename on Enter and outside click'
 Invoke 'Pakai template: Nama akhir'
 if(-not (Find 'Gunakan Tanda Tangan').Current.IsEnabled){throw 'Single click did not enable Use'}
 $null=Wait-State {param($s) $s.signaturePanel -and -not $s.signaturePlacing -and -not $s.dirty}
 Invoke 'Gunakan Tanda Tangan'
 $null=Wait-State {param($s) $s.signaturePlacing -and -not $s.signaturePanel -and -not $s.dirty}
 Invoke 'Tanda tangan';$null=Find 'Pakai template: Nama akhir'
 Double-Click 'Pakai template: Nama akhir'
 $null=Wait-State {param($s) $s.signaturePlacing -and -not $s.signaturePanel -and -not $s.dirty}
 Invoke 'Isi halaman 1'
 $null=Wait-State {param($s) $s.signatureCount -eq 1 -and $s.dirty}
 Write-Host 'PASS signature library: single selection and double-click placement'
 Invoke 'Tanda tangan';Invoke 'Opsi template: Nama akhir';Invoke 'Hapus'
 $null=Find 'Ya, Hapus';Capture 'delete-inline-confirmation.png'
 Invoke 'Batal hapus template';$null=Find 'Pakai template: Nama akhir'
 Invoke 'Opsi template: Nama akhir';Invoke 'Hapus';Invoke 'Ya, Hapus'
 $null=Wait-State {param($s) -not $s.opening -and $s.signatureCount -eq 1}
 $null=Find 'Tersimpan (2)'
 $entries=@((Get-Content -Raw (Join-Path $stateDirectory 'signature-library.json')|ConvertFrom-Json).entries)
 if($entries.Count -ne 2 -or $entries.label -contains 'Nama akhir'){throw 'Template delete not persisted'}
 if((Get-FileHash $a).Hash -ne $hashA){throw 'Source changed'}
 Capture 'deleted-template-object-retained.png'
 Invoke 'Opsi template: Tanda Tangan 1';Invoke 'Hapus';Invoke 'Ya, Hapus';$null=Find 'Tersimpan (1)'
 Invoke 'Opsi template: Tanda Tangan 2';Invoke 'Hapus';Invoke 'Ya, Hapus';$null=Find 'Area gambar tanda tangan'
 if(Control-Exists 'Kembali ke Tersimpan'){throw 'Removing the last template did not restore empty-library creation'}
 Invoke 'Batal tanda tangan';Finish-Owned
 Write-Host "PASS gallery entry, two columns, opt-in and autonaming, restart, inline rename/delete, single/double-click selection, and source preservation: $directory"
}catch{Capture 'failure.png';throw}
