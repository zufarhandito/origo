param([ValidateSet('debug','release')][string]$Profile='debug')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$directory=Join-Path $root ('.research/m3-ui-'+[guid]::NewGuid().ToString('N'))
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
    if($n.Current.NativeWindowHandle -ne 0) {
        $button=[IntPtr]$n.Current.NativeWindowHandle
        if($n.Current.ClassName -eq 'CCPushButton') {
            $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
            $dialog=$ui.FindFirst([Windows.Automation.TreeScope]::Children,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ClassNameProperty,'#32770'))
            # Pinned GPUI maps Other prompt answers to -(index+1). TaskDialog's
            # native child control IDs are unrelated to those logical button IDs.
            $answerId=if($Name -eq 'Batal'){-1}elseif($Name -in @('Lanjutkan','Simpan tanpa bookmark')){-2}else{throw "Unknown prompt answer: $Name"}
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
function Open-Pdf($Path){Picker 'Buka PDF';Fill-Picker $Path;Start-Sleep -Milliseconds 400;$null=Wait-State {param($s) -not $s.opening -and $s.pageCount -eq 1}}
try {
    $null=Wait-State {param($s) $s.pageCount -eq 4}
    Invoke 'Atur halaman'
    Picker 'Impor PDF';Fill-Picker $b
    $null=Wait-State {param($s) $s.sourceCount -eq 2 -and $s.pageCount -eq 8}
    Invoke 'Halaman 2';Invoke 'Putar kanan';Invoke 'Pindahkan';Invoke 'Lebih akhir - Alt+Down'
    Invoke 'Halaman 1';Invoke 'Hapus'
    $null=Wait-State {param($s) $s.pageCount -eq 7 -and $s.dirty}
    $saved=Join-Path $directory 'edited.pdf'
    $state=Save-Pdf 'Simpan sebagai' $saved
    if(-not $state.exportSuccess -or $state.dirty -or $state.exportPages -ne 7){throw "Save As failed: $($state|ConvertTo-Json -Compress)"}
    $qpdf=Join-Path $root 'sidecars/qpdf/bin/qpdf.exe'
    & $qpdf --check $saved | Out-Null;if($LASTEXITCODE -ne 0){throw 'Saved PDF invalid'}
    $output=& $qpdf --json=2 --json-stream-data=none --json-key=pages --json-key=qpdf $saved | ConvertFrom-Json
    if($output.pages.Count -ne 7){throw 'Wrong exported page count'}
    $turns=@();foreach($page in $output.pages){$key='obj:'+$page.object;$value=$output.qpdf[1].PSObject.Properties[$key].Value.value;$turns+=if($value.'/Rotate'){$value.'/Rotate'}else{0}}
    if(($turns -join ',') -ne '180,180,270,0,90,180,270'){throw "Wrong composed rotations: $turns"}
    Invoke 'Undo';$null=Wait-State {param($s) $s.dirty -and $s.pageCount -eq 8}
    Invoke 'Redo';$null=Wait-State {param($s) -not $s.dirty -and $s.pageCount -eq 7}
    Invoke 'Halaman 1';Invoke 'Putar kiri'
    $extract=Join-Path $directory 'extract.pdf';$state=Save-Pdf 'Ekstrak halaman' $extract
    if(-not $state.exportSuccess -or -not $state.dirty -or $state.exportPages -ne 1){throw 'Extract incorrectly updated the clean baseline'}
    $output=& $qpdf --json=2 --json-key=pages $extract|ConvertFrom-Json;if($output.pages.Count -ne 1){throw 'Wrong extract count'}
    $before=$state.exportsCompleted;Picker 'Simpan sebagai';Fill-Picker '' -Cancel
    $null=Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before -and -not $s.exportSuccess -and $s.dirty}
    $info=Join-Path $directory 'info.pdf';Copy-Item (Join-Path $root '.research/export-fixtures/info.pdf') $info
    Open-Pdf $info
    $metadataOutput=Join-Path $directory 'metadata.pdf';$before=(Get-Content -Raw $metrics|ConvertFrom-Json).exportsCompleted
    Picker 'Simpan sebagai';Fill-Picker $metadataOutput
    Invoke 'Batal'
    $null=Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before -and -not $s.exportSuccess}
    if(Test-Path $metadataOutput){throw 'Unapproved metadata output exists'}
    $before=(Get-Content -Raw $metrics|ConvertFrom-Json).exportsCompleted
    Picker 'Simpan sebagai';Fill-Picker $metadataOutput;Invoke 'Lanjutkan'
    $null=Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before -and $s.exportSuccess}
    if(-not (Test-Path $metadataOutput)){throw 'Acknowledged output missing'}
    foreach($feature in @('bookmark','bookmark-named','bookmark-info')) {
        $bookmark=Join-Path $directory "$feature.pdf";Copy-Item (Join-Path $root ".research/export-fixtures/$feature.pdf") $bookmark
        $bookmarkHash=(Get-FileHash $bookmark).Hash
        Open-Pdf $bookmark
        Invoke 'Atur halaman';Invoke 'Putar kanan'
        $withoutBookmark=Join-Path $directory "$feature-edited.pdf"
        $before=(Get-Content -Raw $metrics|ConvertFrom-Json).exportsCompleted
        Picker 'Simpan sebagai';Fill-Picker $withoutBookmark
        $null=Find 'Simpan PDF tanpa bookmark?'
        $detail='Bookmark/daftar isi navigasi tidak disertakan pada PDF hasil ekspor. '
        if($feature -eq 'bookmark-info'){$detail+='Metadata seperti judul, penulis, atau informasi XML juga dapat hilang. '}
        $detail+='Halaman diekspor sesuai susunan; file sumber tetap utuh.'
        $null=Find $detail
        Invoke 'Batal'
        $null=Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before -and -not $s.exportSuccess -and $s.dirty}
        if(Test-Path $withoutBookmark){throw 'Bookmark loss was not approved'}
        $before=(Get-Content -Raw $metrics|ConvertFrom-Json).exportsCompleted
        Picker 'Simpan sebagai';Fill-Picker $withoutBookmark;Invoke 'Simpan tanpa bookmark'
        $null=Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before -and $s.exportSuccess -and -not $s.dirty}
        & $qpdf --check $withoutBookmark|Out-Null;if($LASTEXITCODE -ne 0){throw 'Bookmark output invalid'}
        $output=& $qpdf --json=2 --json-key=outlines --json-key=pages --json-key=qpdf $withoutBookmark|ConvertFrom-Json
        if($output.outlines.Count -ne 0 -or $output.pages.Count -ne 1){throw 'Bookmark removal output mismatch'}
        $key='obj:'+$output.pages[0].object
        if($output.qpdf[1].PSObject.Properties[$key].Value.value.'/Rotate' -ne 90){throw 'Bookmark export lost rotation'}
        if((Get-FileHash $bookmark).Hash -ne $bookmarkHash){throw 'Bookmark source changed'}
    }
    $form=Join-Path $directory 'form.pdf';Copy-Item (Join-Path $root '.research/export-fixtures/form.pdf') $form
    Open-Pdf $form
    $blocked=Join-Path $directory 'blocked.pdf';$state=Save-Pdf 'Simpan sebagai' $blocked
    if($state.exportSuccess -or (Test-Path $blocked) -or $state.pageCount -ne 1){throw 'Unsupported PDF was exported or viewer invalidated'}
    if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $b).Hash -ne $hashB){throw 'Originals changed'}
    if(Get-ChildItem $directory -Directory -Filter '.pdf-studio-export-*'){throw 'Owned staging leaked'}
    $state|ConvertTo-Json -Depth 6|Set-Content (Join-Path $directory 'final.json')
    Write-Host "PASS M3 UI: two-source organizer Save As, output rotations, clean baseline/Undo/Redo, Extract keeps dirty, native cancel, metadata/bookmark/combined decline and accept, named bookmarks, form reject, source hashes, cleanup. Evidence: $directory"
} finally {
    if(-not $app.HasExited){$null=$app.CloseMainWindow();$null=$app.WaitForExit(5000)}
}
