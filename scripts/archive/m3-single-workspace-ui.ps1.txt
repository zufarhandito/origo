param([ValidateSet('debug','release')][string]$Profile='debug')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type 'using System;using System.Runtime.InteropServices;public static class M3Capture{[StructLayout(LayoutKind.Sequential)]public struct Rect{public int L,T,R,B;}[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);[DllImport("user32.dll")]public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);}'
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
    $beforeGeneration=$previous.generation
    $beforeSource=if($previous.pageCount -gt 0){$previous.pagePlan[0][0]}else{-1}
    Picker 'Buka PDF';Fill-Picker $Path
    if($previous.dirty){$null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Buang perubahan'}
    $null=Wait-State {param($s) -not $s.opening -and $s.pageCount -eq $ExpectedPages -and $s.generation -gt $beforeGeneration -and $s.pagePlan[0][0] -ne $beforeSource}
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
$clipboardSaved=[Windows.Forms.Clipboard]::GetDataObject();$clipboardBackup=New-Object Windows.Forms.DataObject
if($clipboardSaved){foreach($format in $clipboardSaved.GetFormats($false)){$data=$clipboardSaved.GetData($format,$false);if($null -ne $data){$clipboardBackup.SetData($format,$data)}}}
try {
    $null=Wait-State {param($s) $s.pageCount -eq 4}
    # Open keeps the old plan/history until a validated replacement is accepted.
    Invoke 'Atur halaman';Invoke 'Putar kanan'
    $original=Wait-State {param($s) $s.dirty}
    Picker 'Buka PDF';Fill-Picker '' -Cancel
    Assert-SameWorkspace $original
    $invalid=Join-Path $directory 'invalid-open.pdf';[IO.File]::WriteAllText($invalid,'Not a PDF')
    Picker 'Buka PDF';Fill-Picker $invalid
    $null=Find 'PDF belum dapat dibuka';Invoke 'Tutup';Assert-SameWorkspace $original
    Picker 'Buka PDF';Fill-Picker $b
    $null=Find 'Simpan perubahan sebelum melanjutkan?'
    Start-Sleep -Milliseconds 400
    Capture-Error 'unsaved-prompt.png'
    if(-not (Find 'Batal').Current.HasKeyboardFocus){throw 'Unsaved dialog did not focus Batal'}
    if((Find 'Putar kanan').Current.IsEnabled -or (Find 'Undo').Current.IsEnabled){throw 'Edits enabled during unsaved prompt'}
    Request-Close # repeated intent must not replace Open or stack another dialog
    Invoke 'Batal';Assert-SameWorkspace $original
    Picker 'Buka PDF';Fill-Picker $b
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Start-Sleep -Milliseconds 300
    [OutputDialog]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]27,[IntPtr]0x10001)|Out-Null
    [OutputDialog]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]27,[IntPtr]0xC0010001)|Out-Null
    Assert-SameWorkspace $original
    # Candidate changes after inspection must not replace the current plan.
    Picker 'Buka PDF';Fill-Picker $b
    $null=Find 'Simpan perubahan sebelum melanjutkan?'
    [IO.File]::AppendAllText($b,"`n% changed candidate`n")
    Invoke 'Buang perubahan';$null=Find 'PDF belum dapat dibuka';Invoke 'Tutup'
    Assert-SameWorkspace $original
    Copy-Item $a $b -Force
    # Cancelling Save As cancels the pending Open as well.
    Picker 'Buka PDF';Fill-Picker $b
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Simpan perubahan sebagai PDF';Fill-Picker '' -Cancel
    $null=Wait-State {param($s) -not $s.exporting -and $s.transition -eq 'Idle'}
    Assert-SameWorkspace $original
    # Empty edited plans still require a decision, but cannot be saved.
    Invoke 'Seleksi';Invoke 'Pilih semua'
    $allSelected=Wait-State {param($s) $s.selectedPages -eq 4}
    Invoke 'Hapus'
    $null=Wait-State {param($s) $s.pageCount -eq 0 -and $s.dirty}
    Request-Close;$null=Find 'Simpan perubahan sebelum melanjutkan?'
    if((Find 'Simpan perubahan sebagai PDF').Current.IsEnabled){throw 'Empty workspace save enabled'}
    Invoke 'Batal';Invoke 'Undo';Assert-SameWorkspace $allSelected
    # Only successful full Save As resumes Open, exactly once.
    $guardSaved=Join-Path $directory 'before-open.pdf'
    Picker 'Buka PDF';Fill-Picker $b
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Simpan perubahan sebagai PDF';Fill-Picker $guardSaved
    $null=Wait-State {param($s) $s.transition -eq 'Idle' -and $s.workspaceInstance -gt $original.workspaceInstance -and -not $s.dirty}
    if(-not (Test-Path $guardSaved)){throw 'Save-before-open did not publish output'}
    # Explicit discard closes a dirty viewer; restart only our fixture process.
    Invoke 'Atur halaman';Invoke 'Putar kanan'
    $null=Wait-State {param($s) $s.dirty}
    Request-Close;$null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Buang perubahan'
    if(-not $app.WaitForExit(10000)){throw 'Discard did not close viewer'}
    [IO.File]::Delete($metrics)
    $oldIds=@(Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | ForEach-Object Id)
    $desktop.ShellExecute($exe,('"'+$a+'" --metrics "'+$metrics+'"'),$root,'open',1)
    $deadline=(Get-Date).AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 200
        $app=Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | Where-Object Id -NotIn $oldIds | Select-Object -First 1
        if($app){$app.Refresh()}
    }while((Get-Date) -lt $deadline -and (-not $app -or $app.MainWindowHandle -eq 0))
    if(-not $app -or $app.MainWindowHandle -eq 0){throw 'Restarted fixture window unavailable'}
    $null=Wait-State {param($s) $s.transition -eq 'Idle' -and $s.pageCount -eq 4}
    Open-Pdf $a 4
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
    # Closing during a native consent dialog waits without cancelling the export.
    $null=Find 'Metadata dokumen dapat tidak terbawa'
    Request-Close;$null=Wait-Transition 'WaitingExport'
    Invoke 'Tetap di aplikasi'
    $null=Wait-Transition 'Idle'
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
    $copySource=Join-Path $directory 'empty-form-tagged.pdf';Copy-Item (Join-Path $root '.research/export-fixtures/empty-form-tagged.pdf') $copySource
    $copyHash=(Get-FileHash $copySource).Hash
    Open-Pdf $copySource;Invoke 'Atur halaman'
    $copyOutput=Join-Path $directory 'whole-source-extract.pdf';$state=Save-Pdf 'Ekstrak halaman' $copyOutput
    if(-not $state.exportSuccess -or $state.exportPages -ne 1){throw 'Whole-source extract failed'}
    if((Get-FileHash $copyOutput).Hash -ne $copyHash -or (Get-FileHash $copySource).Hash -ne $copyHash){throw 'Whole-source extraction changed PDF structure or source'}
    $taggedSource=Join-Path $directory 'tagged-ink.pdf';Copy-Item (Join-Path $root '.research/export-fixtures/tagged-ink.pdf') $taggedSource
    $taggedHash=(Get-FileHash $taggedSource).Hash
    Open-Pdf $taggedSource 3;Invoke 'Atur halaman'
    $taggedOutput=Join-Path $directory 'tagged-page-1.pdf';$before=(Get-Content -Raw $metrics|ConvertFrom-Json).exportsCompleted
    Picker 'Ekstrak halaman';Fill-Picker $taggedOutput
    $null=Find 'Simpan PDF tanpa tag aksesibilitas?'
    Invoke 'Batal';$null=Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before -and -not $s.exportSuccess -and $s.pageCount -eq 3}
    if(Test-Path $taggedOutput){throw 'Unapproved tag removal produced output'}
    $before=(Get-Content -Raw $metrics|ConvertFrom-Json).exportsCompleted
    Picker 'Ekstrak halaman';Fill-Picker $taggedOutput;Invoke 'Simpan tanpa tag'
    $null=Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before -and $s.exportSuccess -and $s.exportPages -eq 1}
    $output=& $qpdf --json=2 --json-key=pages --json-key=qpdf $taggedOutput|ConvertFrom-Json
    $objects=$output.qpdf[1];$rootRef=$objects.trailer.value.'/Root';$catalog=$objects.PSObject.Properties['obj:'+$rootRef].Value.value
    $page=$objects.PSObject.Properties['obj:'+$output.pages[0].object].Value.value
    if($catalog.'/StructTreeRoot' -or $catalog.'/MarkInfo' -or $null -ne $page.'/StructParents' -or $page.'/Tabs' -eq '/S'){throw 'Stale structure survived extraction'}
    Invoke 'Halaman 3';$before=(Get-Content -Raw $metrics|ConvertFrom-Json).exportsCompleted
    $inkOutput=Join-Path $directory 'tagged-page-3.pdf'
    Picker 'Ekstrak halaman';Fill-Picker $inkOutput;Invoke 'Simpan tanpa tag'
    $null=Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before -and $s.exportSuccess}
    $output=& $qpdf --json=2 --json-key=pages --json-key=qpdf $inkOutput|ConvertFrom-Json
    $objects=$output.qpdf[1];$page=$objects.PSObject.Properties['obj:'+$output.pages[0].object].Value.value
    $annotations=$page.'/Annots';if($annotations -is [string]){$annotations=$objects.PSObject.Properties['obj:'+$annotations].Value.value}
    if($annotations.Count -ne 1){throw 'Ink annotation was lost'}
    $ink=$objects.PSObject.Properties['obj:'+$annotations[0]].Value.value
    if($ink.'/Subtype' -ne '/Ink' -or $ink.'/InkList'.Count -ne 1){throw 'Ink was flattened or changed'}
    if((Get-FileHash $taggedSource).Hash -ne $taggedHash){throw 'Tagged source changed'}
    # A successful pending Extract does not authorize discarding unsaved edits.
    Invoke 'Putar kanan';$pendingExtract=Wait-State {param($s) $s.dirty}
    $waitExtract=Join-Path $directory 'wait-extract.pdf'
    Picker 'Ekstrak halaman';Fill-Picker $waitExtract
    $null=Find 'Simpan PDF tanpa tag aksesibilitas?'
    Request-Close;$null=Wait-Transition 'WaitingExport'
    Invoke 'Simpan tanpa tag'
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Batal'
    Assert-SameWorkspace $pendingExtract
    if(-not (Test-Path $waitExtract)){throw 'Waiting cancelled the running Extract'}
    $form=Join-Path $directory 'form.pdf';Copy-Item (Join-Path $root '.research/export-fixtures/form.pdf') $form
    Open-Pdf $form
    $blocked=Join-Path $directory 'blocked.pdf';$state=Save-Pdf 'Simpan sebagai' $blocked
    if($state.exportSuccess -or (Test-Path $blocked) -or $state.pageCount -ne 1){throw 'Unsupported PDF was exported or viewer invalidated'}
    $null=Find 'PDF belum dapat diekspor'
    # Wait for the modal entrance animation before using its text coordinates.
    Start-Sleep -Milliseconds 600
    $reason='Ekspor diblokir: AcroForm/XFA belum didukung.'
    $messageNode=Find $reason;$bounds=$messageNode.Current.BoundingRectangle
    $start=New-Object OrganizerClick+Point;$start.X=[int]$bounds.Left+1;$start.Y=[int]($bounds.Top+$bounds.Height/2)
    $end=New-Object OrganizerClick+Point;$end.X=[int]$bounds.Right-1;$end.Y=$start.Y
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$start)|Out-Null
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$end)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x200,[IntPtr]0,[IntPtr](($start.Y -shl 16) -bor ($start.X -band 0xffff)))|Out-Null
    Start-Sleep -Milliseconds 150
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,[IntPtr](($start.Y -shl 16) -bor ($start.X -band 0xffff)))|Out-Null
    for($step=1;$step -le 12;$step++){$x=[int]($start.X+($end.X-$start.X)*$step/12);[OrganizerClick]::SendMessage($app.MainWindowHandle,0x200,[IntPtr]1,[IntPtr](($end.Y -shl 16) -bor ($x -band 0xffff)))|Out-Null;Start-Sleep -Milliseconds 15}
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,[IntPtr](($end.Y -shl 16) -bor ($end.X -band 0xffff)))|Out-Null
    Start-Sleep -Milliseconds 200
    Capture-Error
    [Windows.Forms.Clipboard]::SetText('NO_ERROR_MESSAGE_COPY')
    Invoke 'Salin pesan'
    if([Windows.Forms.Clipboard]::GetText() -ne $reason){throw 'Error message copy mismatch'}
    $null=Find 'PDF belum dapat diekspor'
    Invoke 'Tutup'
    foreach($case in @(
        @{Feature='tagged';Reason='Ekspor diblokir: PDF memiliki tag/struktur aksesibilitas (/StructTreeRoot); penyimpanan struktur ini belum didukung.'},
        @{Feature='labels';Reason='Ekspor diblokir: PDF memiliki label penomoran halaman (/PageLabels); penyusunan ulang label belum didukung.'},
        @{Feature='unknown';Reason='Ekspor diblokir: atribut catalog /FutureFeature belum didukung. Dokumen perlu diperiksa sebelum dapat diekspor.'}
    )) {
        $path=Join-Path $directory ($case.Feature+'.pdf');Copy-Item (Join-Path $root ('.research/export-fixtures/'+$case.Feature+'.pdf')) $path
        Open-Pdf $path
        $dest=Join-Path $directory ($case.Feature+'-blocked.pdf');$state=Save-Pdf 'Simpan sebagai' $dest
        if($state.exportSuccess -or (Test-Path $dest)){throw 'Catalog blocker was bypassed'}
        $null=Find 'PDF belum dapat diekspor';$null=Find $case.Reason;Invoke 'Tutup'
    }
    $corrupt=Join-Path $directory 'corrupt.pdf';[IO.File]::WriteAllText($corrupt,'Not a PDF')
    Invoke 'Atur halaman';Picker 'Impor PDF';Fill-Picker $corrupt
    $null=Find 'PDF belum dapat diimpor';Invoke 'Tutup'
    $null=Wait-State {param($s) -not $s.opening -and $s.pageCount -eq 1}
    Picker 'Buka PDF';Fill-Picker $corrupt
    $null=Find 'PDF belum dapat dibuka';Invoke 'Tutup'
    $null=Wait-State {param($s) -not $s.opening -and $s.pageCount -eq 1 -and $s.transition -eq 'Idle'}
    Open-Pdf $info
    # Failed Save As must not execute the pending Open.
    Open-Pdf $form
    Invoke 'Atur halaman';Invoke 'Putar kanan'
    $unsavable=Wait-State {param($s) $s.dirty}
    Picker 'Buka PDF';Fill-Picker $a
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Simpan perubahan sebagai PDF'
    $failedSave=Join-Path $directory 'before-open-blocked.pdf';Fill-Picker $failedSave
    $null=Find 'PDF belum dapat diekspor';Invoke 'Tutup'
    Assert-SameWorkspace $unsavable
    if(Test-Path $failedSave){throw 'Blocked pending save created output'}
    Open-Pdf $info
    if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $b).Hash -ne $hashB){throw 'Originals changed'}
    if(Get-ChildItem $directory -Directory -Filter '.pdf-studio-export-*'){throw 'Owned staging leaked'}
    $state|ConvertTo-Json -Depth 6|Set-Content (Join-Path $directory 'final.json')
    # Close -> Save As must stay open after refusal, then close only after commit.
    Invoke 'Atur halaman';Invoke 'Putar kanan'
    $closing=Wait-State {param($s) $s.dirty}
    Request-Close;$null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Simpan perubahan sebagai PDF'
    $closeOutput=Join-Path $directory 'before-close.pdf';Fill-Picker $closeOutput
    Invoke 'Batal'
    $null=Wait-State {param($s) -not $s.exporting -and $s.transition -eq 'Idle' -and $s.dirty}
    Assert-SameWorkspace $closing
    Request-Close;$null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Simpan perubahan sebagai PDF'
    Fill-Picker $closeOutput;Invoke 'Lanjutkan'
    if(-not $app.WaitForExit(20000)){throw 'Save-before-close did not close after commit'}
    if(-not (Test-Path $closeOutput)){throw 'Save-before-close output missing'}
    & $qpdf --check $closeOutput | Out-Null;if($LASTEXITCODE -ne 0){throw 'Close output invalid'}
    if(Get-ChildItem $directory -Directory -Filter '.pdf-studio-export-*'){throw 'Close leaked staging'}
    Write-Host "PASS M3 UI + unsaved guards: cancellation, candidate errors/change, save-before-open/close, modal focus/locks, empty plan, wait-export, organizer/export regressions, source hashes, cleanup. Evidence: $directory"
} finally {
    for($cleanupAttempt=0;$cleanupAttempt -lt 3 -and -not $app.HasExited;$cleanupAttempt++){
        $null=$app.CloseMainWindow();Start-Sleep -Milliseconds 300
        if(-not $app.HasExited){
            $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
            $discard=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Buang perubahan'))
            if($discard){Invoke 'Buang perubahan'}
        }
        $null=$app.WaitForExit(1500)
    }
    if($clipboardSaved){[Windows.Forms.Clipboard]::SetDataObject($clipboardBackup,$true)}else{[Windows.Forms.Clipboard]::Clear()}
}
