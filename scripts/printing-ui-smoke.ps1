param([ValidateSet('debug','release')][string]$Profile='release',[string]$ExecutablePath,[switch]$ProbePrinter)
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'printing-ui' -Fixture '.research/outline-print-fixtures/outline.pdf' -ExecutablePath $ExecutablePath -IsolatedState
$cases=New-Object Collections.Generic.List[object]
Add-Type 'using System;using System.Runtime.InteropServices;public class PrintActivation{[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr h);[DllImport("user32.dll")]public static extern bool IsWindowEnabled(IntPtr h);}'
function Pass([string]$Name){Capture ($Name+'.png');$cases.Add([pscustomobject]@{Case=$Name;Status='passed'});$cases.ToArray()|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'cases.json');Write-Host "PASS printing: $Name"}
function Capture-Helper($Ui){
 $handle=[IntPtr]$Ui.Current.NativeWindowHandle;$rect=New-Object M3Capture+Rect
 [M3Capture]::GetWindowRect($handle,[ref]$rect)|Out-Null
 $bitmap=New-Object Drawing.Bitmap(($rect.R-$rect.L),($rect.B-$rect.T))
 $graphics=[Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
 try{$null=[M3Capture]::PrintWindow($handle,$dc,2)}finally{$graphics.ReleaseHdc($dc);$graphics.Dispose()}
 try{$bitmap.Save((Join-Path $directory 'native-preview.png'))}finally{$bitmap.Dispose()}
}
function Helper {
 $deadline=(Get-Date).AddSeconds(35)
 do {
  $owned=Get-CimInstance Win32_Process -Filter "Name='powershell.exe'"|Where-Object {$_.ParentProcessId -eq $app.Id -and $_.CommandLine.Contains('print-helper.ps1')}|Select-Object -First 1
  if($owned){$script:helper=Get-Process -Id $owned.ProcessId -ErrorAction SilentlyContinue;if($helper){$helper.Refresh();if($helper.MainWindowHandle -ne 0){return [Windows.Automation.AutomationElement]::FromHandle($helper.MainWindowHandle)}}}
  Start-Sleep -Milliseconds 200
 }while((Get-Date)-lt $deadline)
 throw 'Owned print UI did not open'
}
function Native-Find($Ui,[string]$Name) {
 $deadline=(Get-Date).AddSeconds(60)
 do{$fresh=[Windows.Automation.AutomationElement]::FromHandle([IntPtr]$Ui.Current.NativeWindowHandle);$node=$fresh.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Name));if($node){return $node};Start-Sleep -Milliseconds 100}while((Get-Date)-lt $deadline)
 $Ui.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {$_.Current|Select-Object Name,ControlType,ClassName,NativeWindowHandle}|Out-String -Width 200|Set-Content (Join-Path $directory 'native-controls.txt')
 throw "Missing print UI control: $Name"
}
function Native-Invoke($Ui,[string]$Name){
 [PrintActivation]::SetForegroundWindow([IntPtr]$Ui.Current.NativeWindowHandle)|Out-Null
 Start-Sleep -Milliseconds 500
 $node=Native-Find $Ui $Name
 if($node.Current.ClassName -eq '#32770'){
  $condition=[Windows.Automation.AndCondition]::new(
   [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Name),
   [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ClassNameProperty,'Button'))
  $node=$Ui.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)
 }
 $pattern=$null
 if(-not $node){throw "Missing native button: $Name"}
 if($node.Current.NativeWindowHandle -ne 0){[OutputDialog]::PostMessage([IntPtr]$node.Current.NativeWindowHandle,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null}
 elseif($node.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)){$pattern.Invoke()}
 else {throw "Control cannot be invoked: $Name"}
 Start-Sleep -Milliseconds 200
}

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
 $null=Wait-State {param($s) $s.pageCount -eq 3}
 Invoke 'Cetak PDF'
 $rangeField=Find 'Rentang halaman cetak';$rangeField.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('99')
 Invoke 'Pratinjau cetak';$null=Wait-State {param($s) $s.printDialog -and -not $s.exporting}
 Pass 'invalid-range-stays-in-dialog'
 # Escape closes only the owned GPUI configuration modal.
 [OutputDialog]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]27,[IntPtr]0x10001)|Out-Null
 [OutputDialog]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]27,[IntPtr]0xC0010001)|Out-Null
 $null=Wait-State {param($s) -not $s.printDialog}
 Menu-Action 'Tampilan' 'Sesuaikan halaman'
 Invoke 'Anotasi';Invoke 'Pen';Invoke 'Pengaturan pen';Invoke 'Biru';Invoke 'Pengaturan pen'
 Stroke 0.2 0.3 0.6 0.4
 $null=Wait-State {param($s) $s.annotationCount -eq 1 -and $s.dirty}
 Invoke 'Cetak PDF';Invoke 'Halaman ini';Invoke 'Pratinjau cetak'
 $ui=Helper;Native-Invoke $ui 'Batal'
 $null=Wait-State {param($s) -not $s.exporting -and $s.dirty}
 Pass 'preview-cancel-keeps-unsaved-edits'
 Invoke 'Cetak PDF';Invoke 'Halaman ini';Invoke 'Pratinjau cetak'
 $ui=Helper
 Native-Invoke $ui 'Printer...'
 if($ProbePrinter){
  $ui.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|ForEach-Object {$_.Current|Select-Object Name,ControlType,ClassName,NativeWindowHandle}|Out-String -Width 200|Set-Content (Join-Path $directory 'printer-dialog-controls.txt')
  Write-Host "Printer probe: $directory"
  Native-Invoke $ui 'Cancel';Native-Invoke $ui 'Batal'
  $null=Wait-State {param($s) -not $s.exporting}
  Invoke 'Undo';Close-Window;exit 0
 }
 $null=Native-Find $ui 'Properties...'
 $fresh=[Windows.Automation.AutomationElement]::FromHandle([IntPtr]$ui.Current.NativeWindowHandle)
 $printer=$fresh.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ClassNameProperty,'ComboBox'))
 $combo=[IntPtr]$printer.Current.NativeWindowHandle
 $index=[OutputDialog]::SendMessage($combo,0x14D,[IntPtr](-1),'Microsoft Print to PDF')
 if($index.ToInt64() -lt 0){throw 'SAFETY: Microsoft Print to PDF is unavailable'}
 $dialog=Native-Find $ui 'Print';$id=[OutputDialog]::GetDlgCtrlID($combo)
 [OutputDialog]::PostMessage([IntPtr]$dialog.Current.NativeWindowHandle,0x111,[IntPtr]($id -bor (1 -shl 16)),$combo)|Out-Null
 Native-Invoke $ui 'OK'
 $ui=Helper
 $info=Native-Find $ui 'Halaman 1 (1/1) - Microsoft Print to PDF'
 if(-not $info){throw 'SAFETY: virtual printer was not confirmed; no job submitted'}
 $deadline=(Get-Date).AddSeconds(30)
 while(-not [PrintActivation]::IsWindowEnabled([IntPtr]$ui.Current.NativeWindowHandle)){
  if((Get-Date)-gt $deadline){throw 'SAFETY: printer settings are still modal; no job submitted'}
  Start-Sleep -Milliseconds 100
 }
 Capture-Helper $ui
 Pass 'printer-properties-and-preview'
 Native-Invoke $ui 'Cetak'
 $printed=Join-Path $directory 'Printed.pdf';Fill-Picker $printed
 $null=Wait-State {param($s) -not $s.exporting -and $s.exportSuccess -and $s.dirty}
 Pass 'virtual-pdf-new-file-keeps-dirty'
 Picker 'Buka PDF';Fill-Picker $printed
 $null=Wait-State {param($s) $s.tabCount -eq 2 -and $s.pageCount -eq 1 -and -not $s.dirty}
 Pass 'reopen-printed-current-page'
 if((Get-FileHash $a).Hash -ne $hashA){throw 'Source changed'}
 Close-Window;Invoke 'Buang perubahan'
 Write-Host "Printing evidence: $directory"
}catch {Capture 'failure.png';throw}
