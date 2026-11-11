# Headed Windows test using only generated documents and targeted HWND/UIA input.
param([ValidateSet('debug','release')][string]$Profile='release')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$directory=Join-Path $root ('.research/text-ui-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $directory | Out-Null
$fixture=Join-Path $root 'tests/fixtures/text-selection.pdf'
$hash=(Get-FileHash -LiteralPath $fixture).Hash
$source=Join-Path $directory 'source.pdf'
Copy-Item -LiteralPath $fixture -Destination $source
$metrics=Join-Path $directory 'metrics.json'
$probes=Get-Content -Raw -LiteralPath (Join-Path $root '.research/text-selection/probe.json') | ConvertFrom-Json
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public static class PdfTextWindow{[DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);[StructLayout(LayoutKind.Sequential)]public struct Point{public int X,Y;}[StructLayout(LayoutKind.Sequential)]public struct Rect{public int L,T,R,B;}[DllImport("user32.dll")]public static extern bool ScreenToClient(IntPtr h,ref Point p);[DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);[DllImport("user32.dll")]public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);}'
$oldIds=@(Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | ForEach-Object Id)
$shell=New-Object -ComObject Shell.Application
$shell.ShellExecute((Join-Path $root "target/$Profile/pdf-studio.exe"),('"'+$source+'" --metrics "'+$metrics+'"'),$root,'open',1)
Start-Sleep -Seconds 2
$app=Get-Process -Name pdf-studio | Where-Object Id -NotIn $oldIds | Select-Object -First 1
if(-not $app){throw 'Synthetic viewer not found'}
$app.Id | Set-Content (Join-Path $directory 'pid.txt')
$ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
function Find-Control([string]$Name){
    $condition=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Name)
    $node=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)
    if(-not $node){throw "Control not found: $Name"}
    return $node
}
function Invoke-Control([string]$Name){
    $node=Find-Control $Name

    $pattern=$null
    if($node.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)) { $pattern.Invoke() } else {
        $r=$node.Current.BoundingRectangle
        $pt=New-Object PdfTextWindow+Point
        $pt.X=[int]($r.X+$r.Width/2);$pt.Y=[int]($r.Y+$r.Height/2)
        [PdfTextWindow]::ScreenToClient($app.MainWindowHandle,[ref]$pt)|Out-Null
        $lp=[IntPtr](($pt.Y -shl 16) -bor ($pt.X -band 0xffff))
        [PdfTextWindow]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
        [PdfTextWindow]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$lp)|Out-Null
    }

    Start-Sleep -Milliseconds 150
}
function Wait-State([scriptblock]$Predicate){
    $deadline=(Get-Date).AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 100
        try{$state=Get-Content -Raw -LiteralPath $metrics|ConvertFrom-Json}catch{continue}
        if(-not $state.opening -and -not $state.textLoading -and -not $state.rendering -and $state.queue -eq 0 -and (& $Predicate $state)){return $state}
    }while((Get-Date) -lt $deadline)
    throw "Text viewer timeout: $($state|ConvertTo-Json -Compress)"
}
function Copy-Enabled { try { return (Find-Control 'Salin teks').Current.IsEnabled } catch { return $false } }
function Set-Fit {
    Invoke-Control 'Zoom'
    $node=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Sesuaikan halaman'))
    if(-not $node){throw 'Missing fit-page menu item'}

    $pattern=$null
    if($node.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)) { $pattern.Invoke() } else {
        $r=$node.Current.BoundingRectangle
        $pt=New-Object PdfTextWindow+Point
        $pt.X=[int]($r.X+$r.Width/2);$pt.Y=[int]($r.Y+$r.Height/2)
        [PdfTextWindow]::ScreenToClient($app.MainWindowHandle,[ref]$pt)|Out-Null
        $lp=[IntPtr](($pt.Y -shl 16) -bor ($pt.X -band 0xffff))
        [PdfTextWindow]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
        [PdfTextWindow]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$lp)|Out-Null
    }

    Start-Sleep -Milliseconds 150
}
function Jump([int]$Page){
    $input=Find-Control 'Nomor halaman tujuan'
    $input.SetFocus()
    $input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue([string]$Page)
    [PdfTextWindow]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]13,[IntPtr]0x1C0001)|Out-Null
    [PdfTextWindow]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]13,[IntPtr]0xC01C0001)|Out-Null
    $null=Wait-State {param($s) $s.activePage -eq $Page -and $s.textLoads -ge $Page}
}
function Client-Point($Rect,[double]$X,[double]$Y){
    $p=New-Object PdfTextWindow+Point
    $p.X=[int]($Rect.Left+$Rect.Width*$X);$p.Y=[int]($Rect.Top+$Rect.Height*$Y)
    [PdfTextWindow]::ScreenToClient($app.MainWindowHandle,[ref]$p)|Out-Null
    return $p
}
function Mouse([uint32]$Message,[int]$Flags,$Point){
    $packed=[IntPtr](($Point.Y -shl 16) -bor ($Point.X -band 0xffff))
    [PdfTextWindow]::SendMessage($app.MainWindowHandle,$Message,[IntPtr]$Flags,$packed)|Out-Null
}
function Select-Label([int]$Page,[bool]$Reverse=$false){
    Start-Sleep -Milliseconds 250 # Allow accessibility bounds to follow the last fit/layout frame.
    $rect=(Find-Control "Isi halaman $Page").Current.BoundingRectangle
    $probe=$probes[$Page-1]
    $a=Client-Point $rect $probe.firstX $probe.firstY
    $b=Client-Point $rect $probe.lastX $probe.lastY
    if($Reverse){$swap=$a;$a=$b;$b=$swap}
    Mouse 0x200 0 $a
    Mouse 0x201 1 $a
    Mouse 0x200 0 $a # A hover/repaint update must not cancel the pressed selection.
    for($i=1;$i -le 12;$i++){
        $point=New-Object PdfTextWindow+Point
        $point.X=[int]($a.X+($b.X-$a.X)*$i/12);$point.Y=[int]($a.Y+($b.Y-$a.Y)*$i/12)
        Mouse 0x200 1 $point
        Start-Sleep -Milliseconds 25
    }
    Mouse 0x202 0 $b
    $null=Wait-State {param($s) $s.selectedChars -eq 6}
    Invoke-Control 'Salin teks'
    $copied=[Windows.Forms.Clipboard]::GetText()
    if($copied -ne "PAGE $Page"){throw 'Copied synthetic selection does not match expected label'}
    Write-Host "PASS native drag/copy page $Page, reverse=$Reverse"
}
function Capture {
    $rect=New-Object PdfTextWindow+Rect
    [PdfTextWindow]::GetWindowRect($app.MainWindowHandle,[ref]$rect)|Out-Null
    $bitmap=New-Object Drawing.Bitmap(($rect.R-$rect.L),($rect.B-$rect.T))
    $graphics=[Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
    try{$ok=[PdfTextWindow]::PrintWindow($app.MainWindowHandle,$dc,2)}finally{$graphics.ReleaseHdc($dc)}
    try{if(-not $ok){throw 'Capture failed'};$bitmap.Save((Join-Path $directory 'selection.png'))}finally{$graphics.Dispose();$bitmap.Dispose()}
}
$null=Wait-State {param($s) $s.pageCount -eq 4 -and $s.textCacheBytes -gt 0}
Set-Fit
$null=Wait-State {param($s) $s.pageCount -eq 4}
# Materialize existing clipboard formats before testing, never print their contents.
$saved=[Windows.Forms.Clipboard]::GetDataObject()
$backup=New-Object Windows.Forms.DataObject
if($saved){foreach($format in $saved.GetFormats($false)){$data=$saved.GetData($format,$false);if($null -ne $data){$backup.SetData($format,$data)}}}
try {
    $rect=(Find-Control 'Isi halaman 1').Current.BoundingRectangle
    $point=Client-Point $rect $probes[0].firstX $probes[0].firstY
    Mouse 0x200 0 $point;Mouse 0x201 1 $point;Mouse 0x202 0 $point
    $null=Wait-State {param($s) $s.selectedChars -eq 0}
    if((Copy-Enabled)){throw 'Single click selected text'}
    Start-Sleep -Milliseconds ([Windows.Forms.SystemInformation]::DoubleClickTime+100)
    $jitter=New-Object PdfTextWindow+Point;$jitter.X=$point.X+1;$jitter.Y=$point.Y+1
    Mouse 0x201 1 $point;Mouse 0x200 1 $jitter;Mouse 0x202 0 $jitter
    $null=Wait-State {param($s) $s.selectedChars -eq 0}
    Write-Host 'PASS single click and small pointer jitter leave selection empty'
    Start-Sleep -Milliseconds ([Windows.Forms.SystemInformation]::DoubleClickTime+100)
    Mouse 0x200 0 $point
    Mouse 0x201 1 $point;Mouse 0x202 0 $point
    Start-Sleep -Milliseconds 30
    Mouse 0x201 1 $point;Mouse 0x202 0 $point
    $null=Wait-State {param($s) $s.selectedChars -eq 6}
    Invoke-Control 'Salin teks'
    if([Windows.Forms.Clipboard]::GetText() -ne 'PAGE 1'){throw 'Double click did not select the complete sentence/label'}
    Write-Host 'PASS double click selects the full sentence/label'
    Start-Sleep -Milliseconds ([Windows.Forms.SystemInformation]::DoubleClickTime+100)
    Select-Label 1
    Select-Label 1 $true
    $beforeZoom=Get-Content -Raw $metrics|ConvertFrom-Json
    Invoke-Control 'Perbesar'
    $afterZoom=Wait-State {param($s) $s.zoomPercent -gt $beforeZoom.zoomPercent -and $s.selectedChars -eq 6}
    Invoke-Control 'Salin teks'
    if([Windows.Forms.Clipboard]::GetText() -ne 'PAGE 1'){throw 'Selection lost on zoom'}
    if($afterZoom.textLoads -ne $beforeZoom.textLoads){throw 'Zoom unexpectedly re-extracted text'}
    Select-Label 1
    Capture
    Write-Host 'PASS zoom preserves selection and reuses extracted coordinates'
    $beforeMode=Get-Content -Raw $metrics|ConvertFrom-Json
    Invoke-Control 'Atur halaman';Invoke-Control 'Baca'
    $null=Wait-State {param($s) -not $s.organizing -and $s.selectedChars -eq $beforeMode.selectedChars}
    if(-not (Copy-Enabled)){throw 'Mode switch lost text selection'}
    Write-Host 'PASS mode switch preserves text selection'

    foreach($page in 2..4){Jump $page;Set-Fit;$null=Wait-State {param($s) $s.activePage -eq $page};Select-Label $page}
    Jump 1
    for($i=0;$i -lt 5;$i++){
        $zoom=Get-Content -Raw $metrics|ConvertFrom-Json
        if($zoom.zoomPercent -le 50){break}
        Invoke-Control 'Perkecil'
    }
    $null=Wait-State {param($s) $s.zoomPercent -eq 50}
    $firstRect=(Find-Control 'Isi halaman 1').Current.BoundingRectangle
    $secondRect=(Find-Control 'Isi halaman 2').Current.BoundingRectangle
    $a=Client-Point $firstRect $probes[0].firstX $probes[0].firstY
    $b=Client-Point $secondRect $probes[1].lastX $probes[1].lastY
    Mouse 0x200 0 $a;Mouse 0x201 1 $a
    for($i=1;$i -le 20;$i++){
        $point=New-Object PdfTextWindow+Point
        $point.X=[int]($a.X+($b.X-$a.X)*$i/20);$point.Y=[int]($a.Y+($b.Y-$a.Y)*$i/20)
        Mouse 0x200 1 $point;Start-Sleep -Milliseconds 25
    }
    Mouse 0x202 0 $b
    $null=Wait-State {param($s) (Copy-Enabled)}
    Invoke-Control 'Salin teks'
    $lines=@([Windows.Forms.Clipboard]::GetText().Replace("`r`n","`n").Split("`n")|Sort-Object)
    if(($lines -join '|') -ne 'PAGE 1|PAGE 2|Second line|Second line'){throw 'Cross-page selected content mismatch'}
    Write-Host 'PASS drag/copy across two simultaneously visible pages'
    $rect=(Find-Control 'Isi halaman 1').Current.BoundingRectangle
    $blank=Client-Point $rect 0.9 0.05
    Mouse 0x200 0 $blank;Mouse 0x201 1 $blank;Mouse 0x202 0 $blank
    $final=Wait-State {param($s) $s.selectedChars -eq 0}
    if((Copy-Enabled)){throw 'Copy remains enabled with empty selection'}
    Set-Fit
    $final=Wait-State {param($s) $s.activePage -eq 1 -and $s.selectedChars -eq 0}
    if($final.textCacheBytes -gt 16MB){throw 'Text cache exceeded byte budget'}
    $final|ConvertTo-Json|Set-Content (Join-Path $directory 'final.json')
    if((Get-FileHash -LiteralPath $fixture).Hash -ne $hash -or (Get-FileHash -LiteralPath $source).Hash -ne $hash){throw 'Synthetic source changed'}
    Write-Host "PASS text-selection UI, 4 rotations, zoom, clear, bounded text cache, unchanged sources. Evidence: $directory"
} finally {
    if($saved){[Windows.Forms.Clipboard]::SetDataObject($backup,$true)}else{[Windows.Forms.Clipboard]::Clear()}
}



