param([ValidateSet('debug','release')][string]$Profile='debug')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type 'using System;using System.Runtime.InteropServices;public static class M3Capture{[StructLayout(LayoutKind.Sequential)]public struct Rect{public int L,T,R,B;}[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);[DllImport("user32.dll")]public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);}'
$directory=Join-Path $root ('.research/split-ui-'+[guid]::NewGuid().ToString('N'))
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
        if(($id -eq 1148 -or $id -eq 1001 -or $id -eq 1152) -and $cls.ToString() -eq 'Edit'){$script:filename=$h}
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
function Set-Input([string]$Name,[string]$Value) {
    $input=Find $Name
    $input.SetFocus()
    $input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue($Value)
    Start-Sleep -Milliseconds 200
}
function Configure-Split([string]$Spec,[string]$Folder,[switch]$Manual) {
    Invoke 'Alat';Invoke 'Split PDF…';$null=Find 'Split PDF'
    if($Manual){Invoke 'Rentang manual'}
    Set-Input 'Aturan pembagian' $Spec
    Set-Input 'Nama folder Split' $Folder
}
function Run-Split([string]$Spec,[string]$Folder,[switch]$Manual) {
    $before=(Get-Content -Raw $metrics|ConvertFrom-Json).exportsCompleted
    Configure-Split $Spec $Folder -Manual:$Manual
    Picker 'Pilih folder induk';Fill-Picker $directory
    return Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before}
}
try {
    $initial=Wait-State {param($s) $s.pageCount -eq 4 -and $s.transition -eq 'Idle'}
    Configure-Split '0' 'hasil-valid'
    Invoke 'Pilih folder induk'
    $null=Find 'Masukkan jumlah halaman per file antara 1 dan jumlah halaman workspace.'
    Invoke 'Rentang manual';Set-Input 'Aturan pembagian' '1-2;2-3';Invoke 'Pilih folder induk'
    $null=Find 'Gunakan rentang naik seperti 1-3; 4-8; 9-12. Koma menggabungkan halaman dalam satu file. Halaman tidak boleh berulang atau melewati batas workspace.'
    Set-Input 'Aturan pembagian' '1-2;3-4';Set-Input 'Nama folder Split' '../bad';Invoke 'Pilih folder induk'
    $null=Find 'Gunakan nama folder baru, maksimal 100 karakter, tanpa tanda path atau karakter khusus Windows.'
    Set-Input 'Nama folder Split' 'hasil-valid'
    $null=Find '2 file PDF · 4 dari 4 halaman workspace. Halaman yang tidak dicantumkan tidak diekspor.'
    Start-Sleep -Milliseconds 500;Capture-Error 'split-dialog.png'
    Invoke 'Batal';Assert-SameWorkspace $initial
    # Capture the transformed multi-source plan, then verify PDFs and clean baseline.
    Invoke 'Panel alat';Invoke 'Putar kanan'
    Picker 'Impor PDF';Fill-Picker $b
    $before=Wait-State {param($s) $s.pageCount -eq 8 -and $s.dirty}
    $state=Run-Split '3' 'per-tiga'
    if(-not $state.exportSuccess -or $state.splitFiles -ne 3 -or $state.splitDone -ne 3 -or -not $state.dirty){throw 'Split per-N failed or marked workspace saved'}
    Assert-SameWorkspace $before
    $qpdf=Join-Path $root 'sidecars/qpdf/bin/qpdf.exe'
    $files=@(Get-ChildItem (Join-Path $directory 'per-tiga') -File|Sort-Object Name)
    if(($files.Name -join ',') -ne 'bagian-0001.pdf,bagian-0002.pdf,bagian-0003.pdf'){throw 'Wrong deterministic split names'}
    $allRotations=@();$counts=@()
    foreach($file in $files){
        & $qpdf --check $file.FullName | Out-Null;if($LASTEXITCODE -ne 0){throw 'Split output failed QPDF check'}
        $json=& $qpdf --json=2 --json-key=pages --json-key=qpdf $file.FullName|ConvertFrom-Json
        $counts+=$json.pages.Count
        foreach($page in $json.pages){$obj=$json.qpdf[1].PSObject.Properties['obj:'+$page.object].Value.value;$allRotations+=if($obj.'/Rotate'){$obj.'/Rotate'}else{0}}
    }
    if(($counts -join ',') -ne '3,3,2'){throw 'Wrong per-N page groups'}
    $expected=@($before.pagePlan|ForEach-Object {(($_[1]+$_[2])*90)%360})
    if(($allRotations -join ',') -ne ($expected -join ',')){throw 'Split did not preserve composed rotations'}
    $hashes=@($files|ForEach-Object {(Get-FileHash $_.FullName).Hash})
    $state=Run-Split '3' 'per-tiga'
    if($state.exportSuccess){throw 'Existing output directory accepted'}
    $null=Find 'PDF belum dapat dibagi';Invoke 'Tutup'
    if((@(Get-ChildItem (Join-Path $directory 'per-tiga') -File|Sort-Object Name|ForEach-Object {(Get-FileHash $_.FullName).Hash}) -join ',') -ne ($hashes -join ',')){throw 'Existing outputs modified'}
    $state=Run-Split '1-2,4;6-8' 'manual' -Manual
    if(-not $state.exportSuccess -or $state.splitFiles -ne 2 -or $state.exportPages -ne 6 -or -not $state.dirty){throw 'Manual Split failed'}
    Assert-SameWorkspace $before
    Configure-Split '2' 'picker-cancel'
    Picker 'Pilih folder induk';Fill-Picker '' -Cancel
    $null=Wait-State {param($s) -not $s.exporting -and -not $s.exportSuccess}
    if(Test-Path (Join-Path $directory 'picker-cancel')){throw 'Cancelled folder picker published output'}
    Assert-SameWorkspace $before

    # Consent covers every staged part; Close waits and still protects dirty edits.
    $tagged=Join-Path $directory 'tagged.pdf';Copy-Item (Join-Path $root '.research/export-fixtures/tagged-ink.pdf') $tagged
    $taggedHash=(Get-FileHash $tagged).Hash
    Open-Pdf $tagged 3
    Invoke 'Panel alat';Invoke 'Putar kanan'
    $taggedBefore=Wait-State {param($s) $s.dirty}
    Configure-Split '1' 'consent-refused'
    Picker 'Pilih folder induk';Fill-Picker $directory
    $null=Find 'Simpan PDF tanpa tag aksesibilitas?';Invoke 'Batal'
    $null=Wait-State {param($s) -not $s.exporting -and -not $s.exportSuccess}
    if(Test-Path (Join-Path $directory 'consent-refused')){throw 'Refused consent published folder'}
    Configure-Split '1' 'consent-accepted'
    Picker 'Pilih folder induk';Fill-Picker $directory
    $null=Find 'Simpan PDF tanpa tag aksesibilitas?'
    Request-Close;$null=Wait-Transition 'WaitingExport'
    Invoke 'Simpan tanpa tag'
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Batal'
    $final=Wait-State {param($s) -not $s.exporting -and $s.exportSuccess -and $s.splitDone -eq 3 -and $s.dirty}
    Assert-SameWorkspace $taggedBefore
    $inkOutput=Join-Path $directory 'consent-accepted/bagian-0003.pdf'
    $json=& $qpdf --json=2 --json-key=pages --json-key=qpdf $inkOutput|ConvertFrom-Json
    $objects=$json.qpdf[1];$page=$objects.PSObject.Properties['obj:'+$json.pages[0].object].Value.value
    $annots=$page.'/Annots';if($annots -is [string]){$annots=$objects.PSObject.Properties['obj:'+$annots].Value.value}
    $ink=$objects.PSObject.Properties['obj:'+$annots[0]].Value.value
    if($ink.'/Subtype' -ne '/Ink'){throw 'Split lost passive Ink annotation'}
    if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $b).Hash -ne $hashB -or (Get-FileHash $tagged).Hash -ne $taggedHash){throw 'Source PDF changed'}
    if(Get-ChildItem $directory -Directory -Filter '.pdf-studio-split-*'){throw 'Owned batch staging leaked'}
    $final|ConvertTo-Json -Depth 5|Set-Content (Join-Path $directory 'final.json')
    Write-Host "PASS Split UI: validation/preview, per-N/manual, native folder picker, order/rotation, dirty baseline, collisions/cancel, batch tag consent/Ink, close-wait, hashes/cleanup. Evidence: $directory"
} finally {
    for($attempt=0;$attempt -lt 3 -and -not $app.HasExited;$attempt++){
        $condition=[Windows.Automation.AndCondition]::new(
            [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ProcessIdProperty,[int]$app.Id),
            [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ClassNameProperty,'#32770'))
        $native=[Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)
        if($native){
            $cancelCondition=[Windows.Automation.AndCondition]::new(
                [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::AutomationIdProperty,'2'),
                [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ClassNameProperty,'Button'))
            $nativeCancel=$native.FindFirst([Windows.Automation.TreeScope]::Descendants,$cancelCondition)
            $pattern=$null
            if($nativeCancel -and $nativeCancel.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)){$pattern.Invoke();Start-Sleep -Milliseconds 400}
            elseif($nativeCancel -and $nativeCancel.Current.NativeWindowHandle -ne 0){[OutputDialog]::PostMessage([IntPtr]$nativeCancel.Current.NativeWindowHandle,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null;Start-Sleep -Milliseconds 400}
        }
        $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
        $cancel=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Batal'))
        if($cancel){Invoke 'Batal';Start-Sleep -Milliseconds 300}
        $null=$app.CloseMainWindow();Start-Sleep -Milliseconds 400
        if(-not $app.HasExited){
            $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
            $discard=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Buang perubahan'))
            if($discard){Invoke 'Buang perubahan'}
        }
        $null=$app.WaitForExit(1500)
    }
}
