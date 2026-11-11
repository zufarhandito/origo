# Interactive Windows GPU acceptance test. Targets only its own synthetic viewer window.
# Uses UIAutomation Invoke/ValuePattern and HWND messages, never global keyboard input.
param([switch]$Stress, [ValidateSet('debug','release')][string]$Profile='debug', [string]$Fixture='', [ValidateRange(2,1024)][int]$ExpectedPages=100, [ValidateRange(2,1024)][int]$StressSamples=100)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$exe=Join-Path $root "target/$Profile/pdf-studio.exe"
$fixture=if($Fixture){(Resolve-Path -LiteralPath $Fixture).Path}else{Join-Path $root 'tests/fixtures/hundred-pages.pdf'}
$directory=Join-Path $root ('.research/m1-ui-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $directory | Out-Null
$metricsPath=Join-Path $directory 'metrics.json'
$beforeHash=(Get-FileHash -LiteralPath $fixture).Hash
$sourceFixture=$fixture
$fixture=Join-Path $directory 'source.pdf'
Copy-Item -LiteralPath $sourceFixture -Destination $fixture
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
if(-not ('M1Window' -as [type])) {
    Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public static class M1Window {[DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l); [StructLayout(LayoutKind.Sequential)]public struct Point{public int X,Y;}[DllImport("user32.dll")]public static extern bool ScreenToClient(IntPtr h,ref Point p); [StructLayout(LayoutKind.Sequential)]public struct Rect {public int L,T,R,B;} [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);[DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);[DllImport("user32.dll")]public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int height,bool repaint);[DllImport("user32.dll")]public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint f);}'
}
$oldIds=@(Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | ForEach-Object Id)
$desktop=New-Object -ComObject Shell.Application
# Cover the no-argument path used by run.cmd; document-only tests missed a
# debug-build main-thread stack overflow in the empty-state view.
$desktop.ShellExecute($exe,'',$root,'open',1)
$startupDeadline=(Get-Date).AddSeconds(15)
$emptyViewer=$null
do {
    Start-Sleep -Milliseconds 200
    $emptyViewer=Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | Where-Object Id -NotIn $oldIds | Select-Object -First 1
    if($emptyViewer){$emptyViewer.Refresh()}
}while((Get-Date) -lt $startupDeadline -and (-not $emptyViewer -or $emptyViewer.MainWindowHandle -eq 0))
if(-not $emptyViewer -or $emptyViewer.MainWindowHandle -eq 0){throw 'Empty startup failed: no viewer window'}
Start-Sleep -Seconds 1
$emptyViewer.Refresh()
if($emptyViewer.HasExited -or -not $emptyViewer.Responding){throw 'Empty viewer exited or stopped responding'}
$null=$emptyViewer.CloseMainWindow()
if(-not $emptyViewer.WaitForExit(10000)){throw 'Empty viewer did not close'}
Write-Host 'PASS empty startup without PDF argument'
$arguments='"'+$fixture+'" --metrics "'+$metricsPath+'"'
$desktop.ShellExecute($exe,$arguments,$root,'open',1)
Start-Sleep -Seconds 2
$app=Get-Process -Name pdf-studio | Where-Object Id -NotIn $oldIds | Sort-Object StartTime -Descending | Select-Object -First 1
if(-not $app -or $app.Path -ne $exe){throw 'New project viewer not found'}
$app.Id | Set-Content (Join-Path $directory 'pid.txt')
$ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
function Wait-State([scriptblock]$Predicate){
    $deadline=(Get-Date).AddSeconds(30)
    $stable=0;$lastGeneration=-1
    do {
        Start-Sleep -Milliseconds 100
        try{$state=Get-Content -Raw -LiteralPath $metricsPath | ConvertFrom-Json}catch{continue}
        if(-not $state.opening -and -not $state.rendering -and $state.queue -eq 0 -and (& $Predicate $state)){
            if($lastGeneration -eq $state.generation){$stable++}else{$stable=0}
            $lastGeneration=$state.generation
            if($stable -ge 3){return $state}
        }else{$stable=0}
    }while((Get-Date) -lt $deadline)
    throw "Viewer timed out: $($state | ConvertTo-Json -Compress)"
}
function Find-Control([string]$Name,[Windows.Automation.ControlType]$Type){
    $nameCondition=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Name)
    $typeCondition=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty,$Type)
    $condition=[Windows.Automation.AndCondition]::new($nameCondition,$typeCondition)
    $node=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)
    if(-not $node){throw "Control not found: $Name"}
    return $node
}
function Invoke-Control([string]$Name,[int]$Delay=100){
    $node=Find-Control $Name ([Windows.Automation.ControlType]::Button)
    $deadline=(Get-Date).AddSeconds(1)
    while(-not $node.Current.IsEnabled -and (Get-Date) -lt $deadline){Start-Sleep -Milliseconds 100;$node=Find-Control $Name ([Windows.Automation.ControlType]::Button)}
    if(-not $node.Current.IsEnabled){return $false}

    $pattern=$null
    if($node.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)) { $pattern.Invoke() } else {
        $r=$node.Current.BoundingRectangle
        $pt=New-Object M1Window+Point
        $pt.X=[int]($r.X+$r.Width/2);$pt.Y=[int]($r.Y+$r.Height/2)
        [M1Window]::ScreenToClient($app.MainWindowHandle,[ref]$pt)|Out-Null
        $lp=[IntPtr](($pt.Y -shl 16) -bor ($pt.X -band 0xffff))
        [M1Window]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
        [M1Window]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$lp)|Out-Null
    }

    if($Delay -gt 0){Start-Sleep -Milliseconds $Delay}
    return $true
}
function Set-Fit([string]$Name){
    $null=Invoke-Control 'Zoom'
    Start-Sleep -Milliseconds 150
    $node=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Name))
    if(-not $node){throw "Missing zoom menu item: $Name"}

    $pattern=$null
    if($node.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)) { $pattern.Invoke() } else {
        $r=$node.Current.BoundingRectangle
        $pt=New-Object M1Window+Point
        $pt.X=[int]($r.X+$r.Width/2);$pt.Y=[int]($r.Y+$r.Height/2)
        [M1Window]::ScreenToClient($app.MainWindowHandle,[ref]$pt)|Out-Null
        $lp=[IntPtr](($pt.Y -shl 16) -bor ($pt.X -band 0xffff))
        [M1Window]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
        [M1Window]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$lp)|Out-Null
    }

}
function Jump([int]$Page){
    $input=Find-Control 'Nomor halaman tujuan' ([Windows.Automation.ControlType]::Edit)
    $value=$input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
    $input.SetFocus()
    $value.SetValue([string]$Page)
    [M1Window]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]13,[IntPtr]0x1C0001)|Out-Null
    [M1Window]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]13,[IntPtr]0xC01C0001)|Out-Null
}
function Capture([string]$Name){
    $rect=New-Object M1Window+Rect
    [M1Window]::GetWindowRect($app.MainWindowHandle,[ref]$rect)|Out-Null
    $bitmap=New-Object Drawing.Bitmap(($rect.R-$rect.L),($rect.B-$rect.T))
    $graphics=[Drawing.Graphics]::FromImage($bitmap)
    $dc=$graphics.GetHdc()
    try{$ok=[M1Window]::PrintWindow($app.MainWindowHandle,$dc,2)}finally{$graphics.ReleaseHdc($dc)}
    try{if(-not $ok){throw 'Capture failed'};$bitmap.Save((Join-Path $directory "$Name.png"))}
    finally{$graphics.Dispose();$bitmap.Dispose()}
}
$initial=Wait-State {param($s) $s.pageCount -eq $ExpectedPages}
if($initial.fullRenders -gt 12 -or $initial.thumbnailRenders -gt 12){throw 'Eager rendering detected'}
$initial|ConvertTo-Json|Set-Content (Join-Path $directory 'initial.json')
Write-Host "PASS lazy startup: $($initial.fullRenders) page attempts, $($initial.thumbnailRenders) thumbnail attempts"
Capture 'initial'
Set-Fit 'Sesuaikan halaman'
$fit=Wait-State {param($s) $s.zoomPercent -lt $initial.zoomPercent -and $s.pageCount -eq $ExpectedPages}
Capture 'fit-page'
Jump $ExpectedPages
$null=Wait-State {param($s) $s.activePage -eq $ExpectedPages}
$next=Find-Control 'Halaman berikutnya' ([Windows.Automation.ControlType]::Button)
if($next.Current.IsEnabled){throw 'Next must be disabled on last page'}
Capture 'last-page'
Write-Host "PASS jump to page $ExpectedPages and document boundary"
$null=Invoke-Control 'Halaman sebelumnya'
$null=Wait-State {param($s) $s.activePage -eq ($ExpectedPages-1)}
$null=Invoke-Control "Halaman $ExpectedPages"
$null=Wait-State {param($s) $s.activePage -eq $ExpectedPages}
Write-Host 'PASS accessible thumbnail activation'
$null=Invoke-Control 'Halaman pertama'
$null=Wait-State {param($s) $s.activePage -eq 1}
$rect=New-Object M1Window+Rect
[M1Window]::GetWindowRect($app.MainWindowHandle,[ref]$rect)|Out-Null
$position=[IntPtr]((($rect.T+450) -shl 16) -bor ($rect.L+700))
for($i=0;$i -lt 60;$i++){
    [M1Window]::SendMessage($app.MainWindowHandle,0x20A,[IntPtr](-120 -shl 16),$position)|Out-Null
    Start-Sleep -Milliseconds 10
}
$wheel=Wait-State {param($s) $s.activePage -gt 1 -and $s.pageCount -eq $ExpectedPages}
Capture 'continuous-scroll'
Write-Host "PASS continuous scroll reached page $($wheel.activePage)"
for($i=0;$i -lt 18;$i++){if(-not (Invoke-Control 'Perbesar' 0)){break}}
$null=Wait-State {param($s) $s.zoomPercent -eq 400}
Capture 'zoom-400'
for($i=0;$i -lt 18;$i++){if(-not (Invoke-Control 'Perkecil' 0)){break}}
$small=Wait-State {param($s) $s.zoomPercent -eq 50}
Capture 'zoom-50'
if($small.pageCacheBytes -gt 128MB -or $small.thumbnailCacheBytes -gt 16MB -or $small.maxBitmapBytes -gt 64MB){throw 'Memory budget exceeded'}
Write-Host "PASS rapid 50-400% zoom: $($small.staleResults) obsolete results ignored"
Set-Fit 'Sesuaikan halaman'
$null=Wait-State {param($s) $s.pageCount -eq $ExpectedPages}
$generation=(Get-Content -Raw $metricsPath|ConvertFrom-Json).generation
$resizeRect=New-Object M1Window+Rect
[M1Window]::GetWindowRect($app.MainWindowHandle,[ref]$resizeRect)|Out-Null
$resizeWidth=if(($resizeRect.R-$resizeRect.L) -gt 1050){1000}else{1200}
[M1Window]::MoveWindow($app.MainWindowHandle,70,40,$resizeWidth,750,$true)|Out-Null
$resized=Wait-State {param($s) $s.generation -gt $generation -and $s.pageCount -eq $ExpectedPages}
Capture 'resized'
Write-Host 'PASS fit after window resize'
Set-Fit 'Sesuaikan lebar'
$null=Wait-State {param($s) $s.pageCount -eq $ExpectedPages}
Set-Fit 'Sesuaikan halaman'
Jump 1
$final=Wait-State {param($s) $s.activePage -eq 1}
if($Stress){
    Set-Fit 'Sesuaikan lebar'
    $null=Wait-State {param($s) $s.pageCount -eq $ExpectedPages}
    for($pass=1;$pass -le 2;$pass++){
        $sampleCount=[math]::Min($ExpectedPages,$StressSamples)
        $destinations=0..($sampleCount-1) | ForEach-Object { 1+[int][math]::Round($_*($ExpectedPages-1)/($sampleCount-1)) }
        foreach($page in $destinations){
            Jump $page
            $state=Wait-State {param($s) $s.activePage -eq $page}
            if($state.pageCacheBytes -gt 128MB -or $state.thumbnailCacheBytes -gt 16MB){throw 'Stress cache limit exceeded'}
        }
        $app.Refresh()
        [pscustomobject]@{Pass=$pass;WorkingSetMiB=[math]::Round($app.WorkingSet64/1MB,1);PageCacheBytes=$state.pageCacheBytes;ThumbnailCacheBytes=$state.thumbnailCacheBytes;FullRenders=$state.fullRenders} |
            ConvertTo-Json | Set-Content (Join-Path $directory "stress-$pass.json")
        Write-Host "PASS cache stress pass $pass ($sampleCount destinations across $ExpectedPages pages, bounded LRU)"
    }
    Set-Fit 'Sesuaikan halaman'
    Jump 1
    $final=Wait-State {param($s) $s.activePage -eq 1}
}
$final|ConvertTo-Json|Set-Content (Join-Path $directory 'final.json')
if((Get-FileHash -LiteralPath $fixture).Hash -ne $beforeHash){throw 'Source fixture changed'}
if((Get-FileHash -LiteralPath $sourceFixture).Hash -ne $beforeHash){throw 'Original fixture changed'}
$app.Refresh()
[pscustomobject]@{PID=$app.Id;Responding=$app.Responding;WorkingSetMiB=[math]::Round($app.WorkingSet64/1MB,1);PeakWorkingSetMiB=[math]::Round($app.PeakWorkingSet64/1MB,1)}|ConvertTo-Json|Set-Content (Join-Path $directory 'process.json')
Write-Host "PASS M1 UI smoke. Viewer left open. Evidence: $directory"

