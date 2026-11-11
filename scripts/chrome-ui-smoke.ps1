param([ValidateSet('debug','release')][string]$Profile='debug')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type 'using System;using System.Runtime.InteropServices;public static class ChromeWindow{[DllImport("user32.dll")]public static extern uint GetDpiForWindow(IntPtr h);[DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);[StructLayout(LayoutKind.Sequential)]public struct Point{public int X,Y;}[DllImport("user32.dll")]public static extern bool ScreenToClient(IntPtr h,ref Point p);[StructLayout(LayoutKind.Sequential)]public struct Rect{public int L,T,R,B;}[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);[DllImport("user32.dll")]public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int height,bool repaint);[DllImport("user32.dll")]public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);[DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);}'
$directory=Join-Path $root ('.research/chrome-ui-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $directory|Out-Null
$fixture=Join-Path $directory 'Laporan pengujian navigasi dan toolbar dengan nama dokumen yang panjang.pdf'
Copy-Item (Join-Path $root tests/fixtures/text-selection.pdf) $fixture
$metrics=Join-Path $directory metrics.json
$old=@(Get-Process pdf-studio -ErrorAction SilentlyContinue|ForEach-Object Id)
(New-Object -ComObject Shell.Application).ShellExecute((Join-Path $root "target/$Profile/pdf-studio.exe"),('"'+$fixture+'" --metrics "'+$metrics+'"'),$root,'open',1)
Start-Sleep -Seconds 3
$app=Get-Process pdf-studio|Where-Object Id -NotIn $old|Select-Object -First 1
$ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
function Find($Name){return $ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Name))}
function Invoke($Name){$n=Find $Name;if(-not $n){throw "Missing $Name"};
    $pattern=$null
    if($n.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)) { $pattern.Invoke() } else {
        $r=$n.Current.BoundingRectangle
        $pt=New-Object ChromeWindow+Point
        $pt.X=[int]($r.X+$r.Width/2);$pt.Y=[int]($r.Y+$r.Height/2)
        [ChromeWindow]::ScreenToClient($app.MainWindowHandle,[ref]$pt)|Out-Null
        $lp=[IntPtr](($pt.Y -shl 16) -bor ($pt.X -band 0xffff))
        [ChromeWindow]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
        [ChromeWindow]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$lp)|Out-Null
    }
;Start-Sleep -Milliseconds 250}
function Capture($Name){
    $r=New-Object ChromeWindow+Rect;[ChromeWindow]::GetWindowRect($app.MainWindowHandle,[ref]$r)|Out-Null
    $bmp=New-Object Drawing.Bitmap(($r.R-$r.L),($r.B-$r.T));$g=[Drawing.Graphics]::FromImage($bmp);$dc=$g.GetHdc()
    try{$ok=[ChromeWindow]::PrintWindow($app.MainWindowHandle,$dc,2);if(-not $ok){throw 'Capture failed'}}finally{$g.ReleaseHdc($dc)}
    try{$bmp.Save((Join-Path $directory "$Name.png"))}finally{$g.Dispose();$bmp.Dispose()}
}
try {
    Capture initial-read
    Invoke 'Atur halaman'
    Capture initial-organize
    if(-not (Find 'Halaman 1')){throw 'Thumbnail not visible in Organize'}
    foreach($theme in @('Terang','Gelap')) {
        Invoke 'Lainnya';Invoke $theme
        foreach($width in @(860,1000,1200,1600)) {
            [ChromeWindow]::MoveWindow($app.MainWindowHandle,30,20,$width,900,$true)|Out-Null
            Start-Sleep -Milliseconds 500
            foreach($mode in @('Baca','Atur halaman')) {
                Invoke $mode
                $previous=(Find 'Halaman sebelumnya').Current.BoundingRectangle
                $next=(Find 'Halaman berikutnya').Current.BoundingRectangle
                $zoom=(Find 'Zoom').Current.BoundingRectangle
                if([math]::Abs($previous.Top-$zoom.Top) -gt 5 -or [math]::Abs($next.Top-$zoom.Top) -gt 5){throw 'Reading toolbar wrapped'}
                if($next.Right -gt $zoom.Left){throw 'Navigation overlaps zoom'}
                $delete=Find 'Hapus'
                if(($mode -eq 'Baca') -and $delete){throw 'Organizer control leaked into Read'}
                if(($mode -eq 'Atur halaman') -and -not $delete){throw 'Organizer control missing'}
                Capture "$theme-$width-$mode"
            }
        }
    }
    # Enter navigation replaces the old Go button; focused text is not overwritten.
    $input=Find 'Nomor halaman tujuan';$input.SetFocus()
    $input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('3')
    Start-Sleep -Milliseconds 200
    $r=New-Object ChromeWindow+Rect;[ChromeWindow]::GetWindowRect($app.MainWindowHandle,[ref]$r)|Out-Null
    $wheelPoint=[IntPtr]((($r.T+450) -shl 16) -bor ($r.L+700))
    [ChromeWindow]::SendMessage($app.MainWindowHandle,0x20A,[IntPtr](-120 -shl 16),$wheelPoint)|Out-Null
    Start-Sleep -Milliseconds 300
    if($input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).Current.Value -ne '3'){throw 'Focused page input overwritten'}
    [ChromeWindow]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]13,[IntPtr]0x1C0001)|Out-Null
    [ChromeWindow]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]13,[IntPtr]0xC01C0001)|Out-Null
    Start-Sleep -Milliseconds 500
    $s=Get-Content -Raw $metrics|ConvertFrom-Json
    if($s.activePage -ne 3 -or $s.dirty){throw 'Enter navigation or presentation-only state failed'}
    [pscustomobject]@{Profile=$Profile;Dpi=[ChromeWindow]::GetDpiForWindow($app.MainWindowHandle);Widths=@(860,1000,1200,1600);Themes=@('Terang','Gelap');PhysicalMixedDpiTested=$false}|ConvertTo-Json|Set-Content (Join-Path $directory 'acceptance.json')
    Write-Host "PASS chrome: modes, two themes, four widths, non-overlapping toolbar, Enter input. Evidence: $directory"
} finally {if(-not $app.HasExited){$null=$app.CloseMainWindow();$null=$app.WaitForExit(5000)}}
