param([ValidateSet('debug','release')][string]$Profile='release',[string]$TestName='tab-hardening',[string]$Fixture='tests/fixtures/text-selection.pdf',[string]$ExecutablePath)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type 'using System;using System.Runtime.InteropServices;public static class M3Capture{[StructLayout(LayoutKind.Sequential)]public struct Rect{public int L,T,R,B;}[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);[DllImport("user32.dll")]public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);}'
$directory=Join-Path $root (('.research/'+$TestName+'-')+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $directory | Out-Null
$a=Join-Path $directory 'A.pdf'; $b=Join-Path $directory 'B.pdf'
Copy-Item (Join-Path $root $Fixture) $a
Copy-Item (Join-Path $root $Fixture) $b
$hashA=(Get-FileHash $a).Hash; $hashB=(Get-FileHash $b).Hash
$metrics=Join-Path $directory 'metrics.json'
$exe=if($ExecutablePath){(Resolve-Path -LiteralPath $ExecutablePath).Path}else{Join-Path $root "target/$Profile/pdf-studio.exe"}
$oldIds=@(Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | ForEach-Object Id)
$desktop=New-Object -ComObject Shell.Application
if($ExecutablePath){
    $savedPath=$env:PATH
    # This is the interactive UI acceptance window; UIA/PrintWindow need it visible.
    try{$env:PATH="$env:SystemRoot\System32;$env:SystemRoot";Start-Process -FilePath $exe -ArgumentList ('"'+$a+'" --metrics "'+$metrics+'"') -WorkingDirectory (Split-Path $exe) -WindowStyle Normal|Out-Null}
    finally{$env:PATH=$savedPath}
}else{$desktop.ShellExecute($exe,('"'+$a+'" --metrics "'+$metrics+'"'),$root,'open',1)}
$deadline=(Get-Date).AddSeconds(20)
do {
    Start-Sleep -Milliseconds 200
    $owned=Get-CimInstance Win32_Process -Filter "Name='pdf-studio.exe'" | Where-Object {
        $_.ExecutablePath -eq [IO.Path]::GetFullPath($exe) -and $_.CommandLine.Contains([IO.Path]::GetFileName($directory))
    } | Select-Object -First 1
    $app=if($owned){Get-Process -Id $owned.ProcessId -ErrorAction SilentlyContinue}else{$null}
    if($app){$app.Refresh()}
} while((Get-Date) -lt $deadline -and (-not $app -or $app.MainWindowHandle -eq 0))
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
    if($n.Current.NativeWindowHandle -ne 0) {
        $button=[IntPtr]$n.Current.NativeWindowHandle
        if($n.Current.ClassName -eq 'CCPushButton') {
            $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
            $dialog=$ui.FindFirst([Windows.Automation.TreeScope]::Children,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ClassNameProperty,'#32770'))
            # Pinned GPUI maps Other prompt answers to -(index+1). TaskDialog's
            # native child control IDs are unrelated to those logical button IDs.
            $answerId=if($Name -eq 'Batal'){-1}elseif($Name -in @('Lanjutkan','Simpan tanpa bookmark','Simpan tanpa tag')){-2}else{throw "Unknown prompt answer: $Name"}
            [OutputDialog]::PostMessage([IntPtr]$dialog.Current.NativeWindowHandle,0x466,[IntPtr]$answerId,[IntPtr]::Zero)|Out-Null
        } else {
            [OutputDialog]::PostMessage($button,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
        }
        Start-Sleep -Milliseconds 200
        return
    }
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
    if($Name -eq 'Tutup'){
        $deadline=(Get-Date).AddSeconds(5)
        do {
            Start-Sleep -Milliseconds 100
            $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
            $closing=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Tutup'))
        }while($closing -and (Get-Date) -lt $deadline)
        if($closing){throw 'Error popup did not close'}
    }
}

Add-Type 'using System;using System.Text;using System.Runtime.InteropServices;public static class OutputDialog{public delegate bool EnumProc(IntPtr h,IntPtr l);[DllImport("user32.dll")]public static extern bool EnumChildWindows(IntPtr h,EnumProc p,IntPtr l);[DllImport("user32.dll")]public static extern int GetDlgCtrlID(IntPtr h);[DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,StringBuilder s,int n);[DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,string l);[DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);}'
function Picker($Name){
    $n=Find $Name;$r=$n.Current.BoundingRectangle
    $point=New-Object OrganizerClick+Point;$point.X=[int]($r.X+$r.Width/2);$point.Y=[int]($r.Y+$r.Height/2)
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$point)|Out-Null
    $lp=[IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
    [OutputDialog]::PostMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
    [OutputDialog]::PostMessage($app.MainWindowHandle,0x202,[IntPtr]0,$lp)|Out-Null
}
function Fill-Picker([string]$Path,[switch]$Cancel){
    $deadline=(Get-Date).AddSeconds(15);$dialog=$null
    do {
        Start-Sleep -Milliseconds 150
        $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
        $dialog=$ui.FindFirst([Windows.Automation.TreeScope]::Children,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ClassNameProperty,'#32770'))
        if(-not $dialog){
            # Windows may create an unowned picker when the test app is inactive.
            # Match only this test process, never another application's dialog.
            $condition=[Windows.Automation.AndCondition]::new(
                [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty,[int]$app.Id),
                [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ClassNameProperty,'#32770'))
            $dialog=[Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children,$condition)
        }
    }while(-not $dialog -and (Get-Date) -lt $deadline)
    if(-not $dialog){throw 'Native output/input picker missing'}
    $script:filename=[IntPtr]::Zero;$script:accept=[IntPtr]::Zero;$script:cancel=[IntPtr]::Zero
    [OutputDialog]::EnumChildWindows([IntPtr]$dialog.Current.NativeWindowHandle,{
        param($h,$l)
        $id=[OutputDialog]::GetDlgCtrlID($h);$cls=New-Object Text.StringBuilder 256;[OutputDialog]::GetClassName($h,$cls,256)|Out-Null
        # IFileOpenDialog uses 1148; IFileSaveDialog uses the filename ComboBox Edit 1001.
        # The address bar's 1001 is ToolbarWindow32, so it cannot match this field.
        if(($id -eq 1148 -or $id -eq 1001) -and $cls.ToString() -eq 'Edit'){$script:filename=$h}
        if($id -eq 1 -and $cls.ToString() -eq 'Button'){$script:accept=$h}
        if($id -eq 2 -and $cls.ToString() -eq 'Button'){$script:cancel=$h}
        return $true
    },[IntPtr]::Zero)|Out-Null
    if($Cancel){[OutputDialog]::PostMessage($script:cancel,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null;return}
    if($script:filename -eq [IntPtr]::Zero){throw 'Native filename field missing'}
    [OutputDialog]::SendMessage($script:filename,0xC,[IntPtr]::Zero,$Path)|Out-Null
    Start-Sleep -Milliseconds 200
    [OutputDialog]::PostMessage($script:accept,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
}
function Save-Pdf($Name,$Path){
    $before=(Get-Content -Raw $metrics|ConvertFrom-Json).exportsCompleted
    Picker $Name;Fill-Picker $Path
    return Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before}
}

function Switch-Tab([string]$Name) {
    Invoke 'Daftar tab'
    Invoke $Name
}
function Menu-Action([string]$Menu,[string]$Action) {
    Invoke $Menu
    Invoke $Action
}
function Close-Window {
    [OutputDialog]::PostMessage($app.MainWindowHandle,0x10,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
}
function Capture([string]$Name) {
    $rect=New-Object M3Capture+Rect
    [M3Capture]::GetWindowRect($app.MainWindowHandle,[ref]$rect)|Out-Null
    $bitmap=New-Object Drawing.Bitmap(($rect.R-$rect.L),($rect.B-$rect.T))
    $graphics=[Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
    try { $null=[M3Capture]::PrintWindow($app.MainWindowHandle,$dc,2) } finally { $graphics.ReleaseHdc($dc);$graphics.Dispose() }
    try { $bitmap.Save((Join-Path $directory $Name)) } finally { $bitmap.Dispose() }
}
function Read-Metrics {
    for($attempt=0;$attempt -lt 40;$attempt++) {
        try {return Get-Content -Raw $metrics|ConvertFrom-Json} catch {Start-Sleep -Milliseconds 25}
    }
    throw 'Numeric metrics did not become readable'
}
function Click-Control([string]$Name) {
    $node=Find $Name;$r=$node.Current.BoundingRectangle
    $point=New-Object OrganizerClick+Point;$point.X=[int]($r.X+$r.Width/2);$point.Y=[int]($r.Y+$r.Height/2)
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$point)|Out-Null
    $lp=[IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x200,[IntPtr]::Zero,$lp)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]::Zero,$lp)|Out-Null
    Start-Sleep -Milliseconds 100
}
