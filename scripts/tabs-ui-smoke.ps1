param([ValidateSet('debug','release')][string]$Profile='debug')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type 'using System;using System.Runtime.InteropServices;public static class M3Capture{[StructLayout(LayoutKind.Sequential)]public struct Rect{public int L,T,R,B;}[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);[DllImport("user32.dll")]public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);}'
$directory=Join-Path $root ('.research/tabs-ui-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $directory | Out-Null
$a=Join-Path $directory 'A.pdf'; $b=Join-Path $directory 'B.pdf'
Copy-Item (Join-Path $root 'tests/fixtures/text-selection.pdf') $a
Copy-Item (Join-Path $root 'tests/fixtures/text-selection.pdf') $b
$hashA=(Get-FileHash $a).Hash; $hashB=(Get-FileHash $b).Hash
$metrics=Join-Path $directory 'metrics.json'
$exe=Join-Path $root "target/$Profile/pdf-studio.exe"
$oldIds=@(Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | ForEach-Object Id)
$desktop=New-Object -ComObject Shell.Application
$desktop.ShellExecute($exe,('"'+$a+'" --metrics "'+$metrics+'" --state-dir "'+(Join-Path $directory 'state')+'"'),$root,'open',1)
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
try {
    $first=Wait-State {param($s) $s.pageCount -eq 4 -and $s.tabCount -eq 1}
    $oldId=$first.tabId
    Menu-Action 'Berkas' 'Tutup tab  Ctrl+W'
    $emptyDeadline=(Get-Date).AddSeconds(8)
    do {
        Start-Sleep -Milliseconds 150
        $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
        $zoom=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Zoom'))
    } while($zoom -and (Get-Date) -lt $emptyDeadline)
    if($zoom -or $app.HasExited){throw 'Closing last tab did not preserve empty window'}
    Picker 'Buka PDF';Fill-Picker $a
    $first=Wait-State {param($s) $s.tabCount -eq 1 -and $s.tabId -ne $oldId}
    $idA=$first.tabId
    Invoke 'Panel alat';Invoke 'Putar kanan'
    $editedA=Wait-State {param($s) $s.dirty}
    Invoke 'Halaman berikutnya';Invoke 'Perbesar'
    $readingA=Wait-State {param($s) $s.activePage -eq 2}
    Picker 'Buka PDF';Fill-Picker $b
    $second=Wait-State {param($s) $s.tabCount -eq 2 -and $s.tabId -ne $idA -and -not $s.dirty}
    $idB=$second.tabId
    Switch-Tab $a
    $restored=Wait-State {param($s) $s.tabId -eq $idA -and $s.activePage -eq 2 -and $s.dirty}
    if($restored.zoomPercent -ne $readingA.zoomPercent -or ($restored.pagePlan|ConvertTo-Json -Compress) -ne ($editedA.pagePlan|ConvertTo-Json -Compress)){throw 'Switch lost document state'}
    Invoke 'Undo';$null=Wait-State {param($s) -not $s.dirty}
    Invoke 'Redo';$null=Wait-State {param($s) $s.dirty}
    Switch-Tab $b
    $null=Wait-State {param($s) $s.tabId -eq $idB -and -not $s.dirty -and $s.activePage -eq 1}
    Invoke 'Panel alat';Invoke 'Putar kiri'
    $null=Wait-State {param($s) $s.dirty}
    Close-Window
    $null=Find 'Simpan perubahan sebelum melanjutkan?'
    Invoke 'Simpan perubahan sebagai PDF';Fill-Picker '' -Cancel
    $null=Wait-State {param($s) $s.tabCount -eq 2 -and $s.transition -eq 'Idle' -and $s.dirty}
    Close-Window
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Buang perubahan'
    $null=Wait-State {param($s) $s.tabId -eq $idB -and $s.transition -eq 'Prompt'}
    Invoke 'Batal'
    $null=Wait-State {param($s) $s.tabCount -eq 2 -and $s.transition -eq 'Idle'}
    Switch-Tab $a
    $null=Wait-State {param($s) $s.tabId -eq $idA -and $s.dirty}
    $saved=Save-Pdf 'Simpan sebagai' (Join-Path $directory 'saved-A.pdf')
    if(-not $saved.exportSuccess -or $saved.dirty){throw 'Save As did not clean originating tab'}
    Switch-Tab $b
    $null=Wait-State {param($s) $s.tabId -eq $idB -and $s.dirty}
    $count=(Get-Content -Raw $metrics|ConvertFrom-Json).exportsCompleted
    Menu-Action 'Alat' 'Optimize PDF (lossless)…'
    Fill-Picker (Join-Path $directory 'optimized-B.pdf')
    $null=Wait-State {param($s) $s.exportsCompleted -gt $count -and $s.exportSuccess -and $s.dirty}
    # Duplicate open must preserve history and use its existing slot.
    Picker 'Buka PDF';Fill-Picker $a
    $null=Wait-State {param($s) $s.tabId -eq $idA -and $s.tabCount -eq 2 -and -not $s.dirty}
    $bad=Join-Path $directory 'bad.pdf';[IO.File]::WriteAllText($bad,'not pdf')
    $c=Join-Path $directory 'C.pdf';Copy-Item $a $c
    Picker 'Buka PDF';Fill-Picker ('"'+$bad+'" "'+$c+'"')
    $null=Wait-State {param($s) $s.tabCount -eq 3 -and $s.tabId -ne $idA -and $s.tabId -ne $idB}
    Invoke 'Tutup'
    $extra=@()
    1..6|ForEach-Object { $p=Join-Path $directory "extra-$_.pdf";Copy-Item $a $p;$extra+=$p }
    Picker 'Buka PDF';Fill-Picker (($extra|ForEach-Object {'"'+$_+'"'}) -join ' ')
    $null=Find 'Maksimal 8 tab. Tersisa 5 slot; tutup tab atau pilih lebih sedikit PDF.'
    Invoke 'Tutup'
    Picker 'Buka PDF';Fill-Picker (($extra[0..4]|ForEach-Object {'"'+$_+'"'}) -join ' ')
    $null=Wait-State {param($s) $s.tabCount -eq 8}
    1..5|ForEach-Object { Switch-Tab $a;Switch-Tab $b }
    $state=Wait-State {param($s) $s.tabId -eq $idB}
    if($state.pageCacheBytes -gt 128MB -or $state.thumbnailCacheBytes -gt 16MB -or $state.textCacheBytes -gt 16MB -or $state.gpuReleaseErrors -ne 0){throw 'Cache/GPU limits failed'}
    Capture 'desktop-tabs.png'
    # Save earlier tab during close, cancel later tab: earlier receipt stays clean.
    Switch-Tab $a;Menu-Action 'Alat' 'Atur halaman';Invoke 'Putar kanan'
    Close-Window
    $null=Find 'Simpan perubahan sebelum melanjutkan?'
    Invoke 'Simpan perubahan sebagai PDF';Fill-Picker (Join-Path $directory 'close-save-A.pdf')
    $null=Wait-State {param($s) $s.tabId -eq $idB -and $s.transition -eq 'Prompt'}
    Invoke 'Batal'
    Switch-Tab $a
    $null=Wait-State {param($s) $s.tabId -eq $idA -and -not $s.dirty -and $s.tabCount -eq 8}
    Close-Window
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Buang perubahan'
    if(-not $app.WaitForExit(15000)){throw 'Approved close did not finish'}
    if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $b).Hash -ne $hashB){throw 'Source changed'}
    'PASS tab isolation, viewport/history, duplicate/multi/partial/capacity Open, Save As, Optimize, close-batch cancel/save/discard, cache limits and source hashes' | Set-Content (Join-Path $directory 'result.txt')
    Write-Host "PASS tabs UI: $directory"
} catch {
    if($app -and -not $app.HasExited){Capture 'failure.png'}
    throw
}
