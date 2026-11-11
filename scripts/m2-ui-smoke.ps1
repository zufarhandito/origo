param([ValidateSet('debug','release')][string]$Profile='debug')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$directory=Join-Path $root ('.research/m2-ui-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $directory | Out-Null
$a=Join-Path $directory 'A.pdf'; $b=Join-Path $directory 'B.pdf'
Copy-Item (Join-Path $root 'tests/fixtures/text-selection.pdf') $a
Copy-Item (Join-Path $root 'tests/fixtures/text-selection.pdf') $b
$hashA=(Get-FileHash $a).Hash; $hashB=(Get-FileHash $b).Hash
$metrics=Join-Path $directory 'metrics.json'
$exe=Join-Path $root "target/$Profile/pdf-studio.exe"
$oldIds=@(Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | ForEach-Object Id)
$desktop=New-Object -ComObject Shell.Application
$desktop.ShellExecute($exe,('"'+$a+'" --metrics "'+$metrics+'"'),$root,'open',1)
$deadline=(Get-Date).AddSeconds(20)
do { Start-Sleep -Milliseconds 200; $app=Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | Where-Object Id -NotIn $oldIds | Select-Object -First 1; if($app){$app.Refresh()} } while((Get-Date) -lt $deadline -and (-not $app -or $app.MainWindowHandle -eq 0))
if(-not $app -or $app.MainWindowHandle -eq 0){throw 'Visible test window unavailable'}
function Wait-State([scriptblock]$Predicate) {
    $deadline=(Get-Date).AddSeconds(30)
    do { Start-Sleep -Milliseconds 150
        if($app.HasExited){throw 'Viewer exited'}
        try {$s=Get-Content -Raw $metrics | ConvertFrom-Json} catch {continue}
        if(-not $s.opening -and -not $s.rendering -and $s.queue -eq 0 -and (& $Predicate $s)){return $s}
    } while((Get-Date) -lt $deadline)
    throw "Timeout: $($s|ConvertTo-Json -Compress)"
}
function Find($Name) {
    $deadline=(Get-Date).AddSeconds(8)
    do {
        $app.Refresh()
        if($app.HasExited){throw "Viewer exited while finding $Name"}
        $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
        $node=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Name))
        if($node){return $node}
        Start-Sleep -Milliseconds 100
    }while((Get-Date) -lt $deadline)
    $ui.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition) | ForEach-Object {$_.Current | Select-Object Name,ControlType,BoundingRectangle} | Out-String -Width 200 | Set-Content (Join-Path $directory 'missing-controls.txt')
    throw "Missing control: $Name. Evidence: $directory"
}
Add-Type 'using System;using System.Runtime.InteropServices;public static class OrganizerClick{[StructLayout(LayoutKind.Sequential)]public struct Point{public int X,Y;}[DllImport("user32.dll")]public static extern bool ScreenToClient(IntPtr h,ref Point p);[DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);}'
function Invoke($Name) {
    $n=Find $Name

    $pattern=$null
    if($n.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)) { $pattern.Invoke() } else {
        $r=$n.Current.BoundingRectangle
        $pt=New-Object OrganizerClick+Point
        $pt.X=[int]($r.X+$r.Width/2);$pt.Y=[int]($r.Y+$r.Height/2)
        [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$pt)|Out-Null
        $lp=[IntPtr](($pt.Y -shl 16) -bor ($pt.X -band 0xffff))
        [OrganizerClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
        [OrganizerClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$lp)|Out-Null
    }
    Start-Sleep -Milliseconds 200
}
try {
    $initial=Wait-State {param($s) $s.pageCount -eq 4}
    if($initial.organizing -or $initial.dirty){throw 'Expected clean Read mode'}
    Invoke 'Halaman 2'
    $null=Wait-State {param($s) $s.activePage -eq 2 -and $s.selectedPages -eq 0 -and -not $s.dirty}
    Invoke 'Atur halaman'
    $null=Wait-State {param($s) $s.organizing -and $s.selectedPages -eq 1 -and -not $s.dirty}
    Invoke 'Baca'
    $null=Wait-State {param($s) -not $s.organizing -and $s.selectedPages -eq 0 -and -not $s.dirty}
    Invoke 'Atur halaman'
    Add-Type 'using System;using System.Runtime.InteropServices;public static class M2DragInput{[StructLayout(LayoutKind.Sequential)]public struct Point{public int X,Y;}[DllImport("user32.dll")]public static extern bool ScreenToClient(IntPtr h,ref Point p);[DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);}'
    function DragPoint($node) {
        $r=$node.Current.BoundingRectangle
        $p=New-Object M2DragInput+Point;$p.X=[int]($r.X+$r.Width/2);$p.Y=[int]($r.Y+$r.Height/2)
        [M2DragInput]::ScreenToClient($app.MainWindowHandle,[ref]$p)|Out-Null;return $p
    }
    function DragMouse($message,$flags,$x,$y) {
        $packed=[IntPtr](([int]$y -shl 16) -bor ([int]$x -band 0xffff))
        [M2DragInput]::PostMessage($app.MainWindowHandle,$message,[IntPtr]$flags,$packed)|Out-Null
        Start-Sleep -Milliseconds 35
    }
    $from=DragPoint (Find 'Halaman 1');$to=DragPoint (Find 'Halaman 3')
    DragMouse 0x200 0 $from.X $from.Y
    DragMouse 0x201 1 $from.X $from.Y
    for($step=1;$step -le 12;$step++){ $fraction=$step/12;DragMouse 0x200 1 ($from.X+($to.X-$from.X)*$fraction) ($from.Y+($to.Y-$from.Y)*$fraction) }
    DragMouse 0x202 0 $to.X $to.Y
    $null=Wait-State {param($s) $s.pagePlan[1][1] -eq 0}
    Invoke 'Undo';$null=Wait-State {param($s) -not $s.dirty -and $s.pagePlan[0][1] -eq 0}
    # A canceled drag must not create an edit/history entry.
    $from=DragPoint (Find 'Halaman 1');$to=DragPoint (Find 'Halaman 3')
    DragMouse 0x201 1 $from.X $from.Y
    DragMouse 0x200 1 ($from.X+15) ($from.Y+15)
    [M2DragInput]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]27,[IntPtr]0x10001)|Out-Null
    [M2DragInput]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]27,[IntPtr]0xC0010001)|Out-Null
    Start-Sleep -Milliseconds 200
    DragMouse 0x202 0 $to.X $to.Y
    $null=Wait-State {param($s) -not $s.dirty -and $s.pagePlan[0][1] -eq 0 -and -not $s.pageDragging}

    Invoke 'Halaman 2'
    Invoke 'Putar kanan'
    $rotated=Wait-State {param($s) $s.dirty -and $s.pagePlan[1][2] -eq 1}
    Invoke 'Pindahkan'; Invoke 'Lebih akhir - Alt+Down'
    $moved=Wait-State {param($s) $s.pagePlan[2][1] -eq 1}
    Invoke 'Hapus'
    $null=Wait-State {param($s) $s.pageCount -eq 3}
    Invoke 'Undo'; $null=Wait-State {param($s) $s.pageCount -eq 4}
    Invoke 'Undo'; $null=Wait-State {param($s) $s.pagePlan[1][1] -eq 1}
    Invoke 'Undo'; $null=Wait-State {param($s) -not $s.dirty}
    Invoke 'Redo'; $null=Wait-State {param($s) $s.pagePlan[1][2] -eq 1}
    Invoke 'Seleksi'; Invoke 'Pilih semua'; Invoke 'Hapus'
    $null=Wait-State {param($s) $s.pageCount -eq 0}
    Invoke 'Undo'; $null=Wait-State {param($s) $s.pageCount -eq 4}
    Add-Type 'using System;using System.Runtime.InteropServices;public static class M2Input{[StructLayout(LayoutKind.Sequential)]public struct Point{public int X,Y;}[DllImport("user32.dll")]public static extern bool ScreenToClient(IntPtr h,ref Point p);[DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);}'
    $r=(Find 'Impor PDF').Current.BoundingRectangle
    $point=New-Object M2Input+Point; $point.X=[int]($r.X+$r.Width/2);$point.Y=[int]($r.Y+$r.Height/2)
    [M2Input]::ScreenToClient($app.MainWindowHandle,[ref]$point)|Out-Null
    $packed=[IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
    [M2Input]::PostMessage($app.MainWindowHandle,0x201,[IntPtr]1,$packed)|Out-Null
    [M2Input]::PostMessage($app.MainWindowHandle,0x202,[IntPtr]0,$packed)|Out-Null
    Start-Sleep -Seconds 2
    $app.Refresh()
    $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
    $dialog=$ui.FindFirst([Windows.Automation.TreeScope]::Children,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ClassNameProperty,'#32770'))
    if(-not $dialog){throw 'Import dialog not found'}
    Add-Type 'using System;using System.Text;using System.Runtime.InteropServices;public static class M2Dialog{public delegate bool EnumProc(IntPtr h,IntPtr l);[DllImport("user32.dll")]public static extern bool EnumChildWindows(IntPtr h,EnumProc p,IntPtr l);[DllImport("user32.dll")]public static extern int GetDlgCtrlID(IntPtr h);[DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,StringBuilder s,int n);[DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,string l);}'
    $script:filenameHandle=[IntPtr]::Zero
    $script:openHandle=[IntPtr]::Zero
    [M2Dialog]::EnumChildWindows([IntPtr]$dialog.Current.NativeWindowHandle,{
        param($h,$l)
        $id=[M2Dialog]::GetDlgCtrlID($h)
        $cls=New-Object Text.StringBuilder 256;[M2Dialog]::GetClassName($h,$cls,256)|Out-Null
        if($cls.ToString() -eq 'Button' -and $id -eq 1){$script:openHandle=$h}
        if($cls.ToString() -eq 'Edit' -and $id -eq 1148){$script:filenameHandle=$h}
        return $true
    },[IntPtr]::Zero)|Out-Null
    if($script:filenameHandle -eq [IntPtr]::Zero){throw 'Native filename control not found'}
    [M2Dialog]::SendMessage($script:filenameHandle,0xC,[IntPtr]::Zero,$b)|Out-Null
    Start-Sleep -Milliseconds 300
    [M2Input]::PostMessage($script:openHandle,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
    $merged=Wait-State {param($s) $s.pageCount -eq 8 -and $s.sourceCount -eq 2}
    if($merged.pagePlan[4][1] -ne 0 -or $merged.pagePlan[4][0] -eq $merged.pagePlan[0][0]){throw 'Wrong source mapping'}
    Invoke 'Undo'; $null=Wait-State {param($s) $s.pageCount -eq 4 -and $s.sourceCount -eq 1}
    Invoke 'Redo'; $final=Wait-State {param($s) $s.pageCount -eq 8 -and $s.sourceCount -eq 2}
    if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $b).Hash -ne $hashB){throw 'Source changed'}
    $final|ConvertTo-Json -Depth 5|Set-Content (Join-Path $directory 'final.json')
    Write-Host "PASS M2 UI: drag, rotate, move, delete, empty, history, native import, multi-source plan, unchanged sources. Evidence: $directory"
} finally {
    if(-not $app.HasExited){$null=$app.CloseMainWindow();if(-not $app.WaitForExit(3000)){$null=$app.CloseMainWindow();$null=$app.WaitForExit(3000)}}
}
