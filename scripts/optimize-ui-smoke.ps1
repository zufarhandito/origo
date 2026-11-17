param([ValidateSet('debug','release')][string]$Profile='debug')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type 'using System;using System.Runtime.InteropServices;public static class M3Capture{[StructLayout(LayoutKind.Sequential)]public struct Rect{public int L,T,R,B;}[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);[DllImport("user32.dll")]public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);}'
$directory=Join-Path $root ('.research/optimize-ui-'+[guid]::NewGuid().ToString('N'))
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
function Open-Pdf($Path,[int]$ExpectedPages=1){
    $previous=Get-Content -Raw $metrics|ConvertFrom-Json
    Invoke 'Berkas';Invoke 'Tutup tab  Ctrl+W'
    if($previous.dirty){$null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Buang perubahan'}
    Start-Sleep -Milliseconds 300
    Picker 'Buka PDF';Fill-Picker $Path
    $null=Wait-State {param($s) $s.pageCount -eq $ExpectedPages -and $s.tabId -ne $previous.tabId}
}
function Capture-Error([string]$Name='error-popup.png') {
    $rect=New-Object M3Capture+Rect;[M3Capture]::GetWindowRect($app.MainWindowHandle,[ref]$rect)|Out-Null
    $bitmap=New-Object Drawing.Bitmap(($rect.R-$rect.L),($rect.B-$rect.T));$graphics=[Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
    try{$ok=[M3Capture]::PrintWindow($app.MainWindowHandle,$dc,2)}finally{$graphics.ReleaseHdc($dc)}
    try{if(-not $ok){throw 'Popup capture failed'};$bitmap.Save((Join-Path $directory $Name))}finally{$graphics.Dispose();$bitmap.Dispose()}
}
function Wait-Transition([string]$Phase) {
    $deadline=(Get-Date).AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 100
        if($app.HasExited){throw 'Viewer exited during transition'}
        try{$s=Get-Content -Raw $metrics|ConvertFrom-Json}catch{continue}
        if($s.transition -eq $Phase){return $s}
    }while((Get-Date) -lt $deadline)
    throw "Transition did not reach $Phase"
}
function Request-Close {
    [OutputDialog]::PostMessage($app.MainWindowHandle,0x10,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
}
function Assert-SameWorkspace($Before) {
    $s=Wait-Transition 'Idle'
    if($s.workspaceInstance -ne $Before.workspaceInstance -or $s.revision -ne $Before.revision -or
        $s.activePage -ne $Before.activePage -or $s.selectedPages -ne $Before.selectedPages -or
        ($s.pagePlan|ConvertTo-Json -Compress) -ne ($Before.pagePlan|ConvertTo-Json -Compress)){
        throw 'Cancelled/failed transition changed the workspace'
    }
}
function Start-Optimize([string]$Path) {
    Invoke 'Alat';Picker 'Optimize PDF (lossless)…';Fill-Picker $Path
}
function Optimize-Pdf([string]$Path) {
    $before=(Get-Content -Raw $metrics|ConvertFrom-Json).exportsCompleted
    Start-Optimize $Path
    return Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before}
}
function Check-SizeReport($State,[string]$Path) {
    if(-not $State.exportSuccess -or -not $State.optimizing -or $State.optimizeBeforeBytes -le 0 -or
        $State.optimizeAfterBytes -ne (Get-Item -LiteralPath $Path).Length){throw 'Incorrect optimization size receipt'}
    $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
    $notice=$ui.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.Name.StartsWith('Optimize selesai:')}|Select-Object -First 1
    if(-not $notice){throw 'Accessible optimization result missing'}
    $text=$notice.Current.Name
    if(-not $text.Contains([string]$State.optimizeBeforeBytes+' B') -or -not $text.Contains([string]$State.optimizeAfterBytes+' B')){throw 'UI result does not show exact before/after bytes'}
    $change=if($State.optimizeAfterBytes -lt $State.optimizeBeforeBytes){'Ukuran berkurang'}elseif($State.optimizeAfterBytes -gt $State.optimizeBeforeBytes){'Ukuran bertambah'}else{'Ukuran tetap sama'}
    if(-not $text.Contains($change)){throw 'Misleading size-change notice'}
}
try {
    $initial=Wait-State {param($s) $s.pageCount -eq 4 -and $s.transition -eq 'Idle'}
    $before=$initial.exportsCompleted
    Invoke 'Alat';Picker 'Optimize PDF (lossless)…';Fill-Picker '' -Cancel
    $null=Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before -and -not $s.exportSuccess}
    Assert-SameWorkspace $initial
    Invoke 'Panel alat';Picker 'Impor PDF';Fill-Picker $b
    $null=Wait-State {param($s) $s.pageCount -eq 8}
    Invoke 'Halaman 2';Invoke 'Putar kanan';Invoke 'Pindahkan';Invoke 'Lebih akhir - Alt+Down'
    Invoke 'Halaman 1';Invoke 'Hapus'
    $edited=Wait-State {param($s) $s.pageCount -eq 7 -and $s.dirty}
    $output=Join-Path $directory 'optimized.pdf'
    $state=Optimize-Pdf $output
    Check-SizeReport $state $output
    if(-not $state.dirty){throw 'Optimize incorrectly marked workspace saved'}
    Assert-SameWorkspace $edited
    Start-Sleep -Milliseconds 400;Capture-Error 'optimize-result.png'
    $qpdf=Join-Path $root 'sidecars/qpdf/bin/qpdf.exe'
    & $qpdf --check $output | Out-Null;if($LASTEXITCODE -ne 0){throw 'Optimized output failed QPDF check'}
    $json=& $qpdf --json=2 --json-key=pages --json-key=qpdf $output|ConvertFrom-Json
    if($json.pages.Count -ne 7){throw 'Optimize did not capture current page plan'}
    $turns=@();foreach($page in $json.pages){$value=$json.qpdf[1].PSObject.Properties['obj:'+$page.object].Value.value;$turns+=if($value.'/Rotate'){$value.'/Rotate'}else{0}}
    $expected=@($edited.pagePlan|ForEach-Object {(($_[1]+$_[2])*90)%360})
    if(($turns -join ',') -ne ($expected -join ',')){throw 'Composed rotations changed'}
    Invoke 'Undo';$null=Wait-State {param($s) $s.pageCount -eq 8 -and $s.dirty}
    Invoke 'Redo';$null=Wait-State {param($s) $s.pageCount -eq 7 -and $s.dirty}
    Assert-SameWorkspace $edited
    $info=Join-Path $directory 'info.pdf';Copy-Item (Join-Path $root '.research/export-fixtures/info.pdf') $info
    Open-Pdf $info
    $metadataOutput=Join-Path $directory 'metadata-optimized.pdf'
    Start-Optimize $metadataOutput
    $null=Find 'Metadata dokumen dapat tidak terbawa';Invoke 'Batal'
    $null=Wait-State {param($s) -not $s.exporting -and -not $s.exportSuccess}
    if(Test-Path $metadataOutput){throw 'Unapproved metadata output exists'}
    Start-Optimize $metadataOutput;Invoke 'Lanjutkan'
    $state=Wait-State {param($s) -not $s.exporting -and $s.exportSuccess}
    Check-SizeReport $state $metadataOutput
    if($state.dirty){throw 'Clean workspace changed after Optimize'}
    # A completed Optimize cannot authorize closing a dirty workspace.
    Invoke 'Panel alat';Invoke 'Putar kanan'
    $dirty=Wait-State {param($s) $s.dirty}
    $waiting=Join-Path $directory 'close-wait-optimized.pdf'
    Start-Optimize $waiting;$null=Find 'Metadata dokumen dapat tidak terbawa'
    Request-Close;$null=Wait-Transition 'WaitingExport';Invoke 'Lanjutkan'
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Batal'
    $state=Wait-State {param($s) -not $s.exporting -and $s.exportSuccess -and $s.dirty}
    Check-SizeReport $state $waiting
    Assert-SameWorkspace $dirty
    foreach($feature in @('form','tagged','signature')){
        $source=Join-Path $directory ($feature+'.pdf');Copy-Item (Join-Path $root ('.research/export-fixtures/'+$feature+'.pdf')) $source
        Open-Pdf $source
        $blocked=Join-Path $directory ($feature+'-blocked.pdf')
        $state=Optimize-Pdf $blocked
        if($state.exportSuccess -or (Test-Path $blocked)){throw 'Conservative Optimize preflight bypassed'}
        $null=Find 'PDF belum dapat diekspor';Invoke 'Tutup'
    }
    $blank=Join-Path $directory 'blank.pdf';Copy-Item (Join-Path $root '.research/export-fixtures/optimize-blank.pdf') $blank
    Open-Pdf $blank
    $blankOutput=Join-Path $directory 'blank-optimized.pdf';$state=Optimize-Pdf $blankOutput
    Check-SizeReport $state $blankOutput
    $state|ConvertTo-Json -Depth 5|Set-Content (Join-Path $directory 'blank-size.json')
    $jpeg=Join-Path $directory 'jpeg.pdf';Copy-Item (Join-Path $root '.research/export-fixtures/optimize-jpeg.pdf') $jpeg
    $jpegHash=(Get-FileHash $jpeg).Hash
    Open-Pdf $jpeg
    $jpegOutput=Join-Path $directory 'jpeg-optimized.pdf';$state=Optimize-Pdf $jpegOutput
    Check-SizeReport $state $jpegOutput
    if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $b).Hash -ne $hashB -or (Get-FileHash $jpeg).Hash -ne $jpegHash){throw 'Source bytes changed'}
    if(Get-ChildItem $directory -Directory -Filter '.pdf-studio-export-*'){throw 'Owned optimization staging leaked'}
    $state|ConvertTo-Json -Depth 5|Set-Content (Join-Path $directory 'final.json')
    Write-Host "PASS Optimize UI: picker cancel, multi-source edited plan/rotations, exact before/after report, unchanged dirty/history, loss consent, close-wait guard, blockers, JPEG, hashes and cleanup. Evidence: $directory"
} finally {
    for($attempt=0;$attempt -lt 3 -and -not $app.HasExited;$attempt++){
        $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
        $errorClose=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Tutup'))
        if($errorClose){Invoke 'Tutup'}
        $null=$app.CloseMainWindow();Start-Sleep -Milliseconds 300
        if(-not $app.HasExited){
            $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
            $discard=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Buang perubahan'))
            if($discard){Invoke 'Buang perubahan'}
        }
        $null=$app.WaitForExit(1500)
    }
}
