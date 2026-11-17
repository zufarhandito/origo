# Generated-book profiling for the two visible Dock panels. Owned fixture only.
param([ValidateSet('debug','release')][string]$Profile='release',[ValidateRange(2,5)][int]$Rounds=3,[switch]$HighZoom,[switch]$SameDocument)
$profileTestName=if($SameDocument){'same-document-memory'}else{'comparison-memory'}
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName $profileTestName -Fixture '.research/book-benchmark/vector-1000.pdf' -IsolatedState
$leftPane=if($SameDocument){'A.pdf (kiri)'}else{'A.pdf'}
$rightPane=if($SameDocument){'A.pdf (kanan)'}else{'B.pdf'}
Add-Type 'using System;using System.Runtime.InteropServices;public static class TabGuiMemory{[DllImport("user32.dll")]public static extern uint GetGuiResources(IntPtr process,uint flag);}'
$samples=New-Object Collections.Generic.List[object]
$gpuAvailable=$false
function Sample([string]$Phase,[int]$Round=0) {
    $app.Refresh()
    if($app.HasExited){throw 'Owned viewer exited during profiling'}
    $s=Read-Metrics
    $gpu=@()
    try { $gpu=@(Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUProcessMemory -ErrorAction Stop | Where-Object {$_.Name.StartsWith("pid_$($app.Id)_",[StringComparison]::Ordinal)}) } catch {}
    $valid=$gpu.Count -gt 0
    if($valid){$script:gpuAvailable=$true}
    $shared=if($valid){[long](($gpu|Measure-Object SharedUsage -Sum).Sum)}else{$null}
    $dedicated=if($valid){[long](($gpu|Measure-Object DedicatedUsage -Sum).Sum)}else{$null}
    $row=[pscustomobject]@{
        Phase=$Phase;Round=$Round;Utc=[DateTime]::UtcNow.ToString('o');TabCount=$s.tabCount;VisibleTabs=$s.visibleTabIds.Count;VisibleViews=$s.visibleViewIds.Count;RetainedViews=$s.sessions.Count;SplitView=$s.splitView
        PrivateBytes=$app.PrivateMemorySize64;WorkingSetBytes=$app.WorkingSet64;PeakWorkingSetBytes=$app.PeakWorkingSet64
        Handles=$app.HandleCount;Threads=$app.Threads.Count;GdiObjects=[TabGuiMemory]::GetGuiResources($app.Handle,0);UserObjects=[TabGuiMemory]::GetGuiResources($app.Handle,1)
        GpuCountersAvailable=$valid;GpuSharedBytes=$shared;GpuDedicatedBytes=$dedicated
        PageCacheBytes=$s.totalPageCacheBytes;ThumbnailCacheBytes=$s.totalThumbnailCacheBytes;TextCacheBytes=$s.totalTextCacheBytes
        InactiveCacheBytes=$s.inactiveCacheBytes;InactiveGpuImages=$s.inactiveGpuImages;GpuImages=$s.totalGpuImages
        NativeDocuments=$s.nativeActiveDocuments;GpuReleaseErrors=$s.windowGpuReleaseErrors
    }
    $samples.Add($row)
    $samples|Export-Csv -NoTypeInformation -Encoding UTF8 (Join-Path $directory 'samples.csv')
    if($s.totalPageCacheBytes -gt 128MB -or $s.totalThumbnailCacheBytes -gt 16MB -or $s.totalTextCacheBytes -gt 16MB -or
        $s.inactiveCacheBytes -ne 0 -or $s.inactiveGpuImages -ne 0 -or $s.nativeActiveDocuments -gt 1 -or $s.windowGpuReleaseErrors -ne 0){throw "Window resource invariant failed at $Phase"}
    if($app.PrivateMemorySize64 -gt 2GB){throw 'Generated profile exceeded 2 GiB private bytes'}
    return $row
}
function Focus-Pdf([string]$Name){
    $rect=(Find ('Panel PDF: '+$Name)).Current.BoundingRectangle
    if($rect.Width -lt 300 -or $rect.Height -lt 100){throw 'PDF pane is not realized'}
    $point=New-Object OrganizerClick+Point;$point.X=[int]($rect.X+60);$point.Y=[int]($rect.Y+12)
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$point)|Out-Null
    $packed=[IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x200,[IntPtr]0,$packed)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$packed)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$packed)|Out-Null
    Start-Sleep -Milliseconds 150
}
function Page([string]$Number){
    $field=Find 'Nomor halaman tujuan';$field.SetFocus();$field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue($Number)
    [OutputDialog]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]13,[IntPtr]0x10001)|Out-Null
    [OutputDialog]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]13,[IntPtr]0xC0010001)|Out-Null
}
function Open-Comparison {
    Switch-Tab $a
    if($SameDocument){Menu-Action 'Tampilan' 'Tampilan kedua PDF ini'}else{Menu-Action 'Tampilan' 'Bandingkan dua PDF…';Invoke $b}
    $null=Wait-State {param($s) $s.splitView -and $s.visibleViewIds.Count -eq 2 -and $s.visibleTabIds.Count -eq $(if($SameDocument){1}else{2})}
}
try {
    $null=Wait-State {param($s) $s.pageCount -eq 1000 -and $s.sessionReady}
    Copy-Item (Join-Path $root '.research/book-benchmark/simulated-scan-1000.pdf') $b -Force
    $paths=@($a,$b)
    0..5|ForEach-Object {
        $path=Join-Path $directory (([char](67+$_))+'.pdf')
        $source=if($_%2 -eq 0){$a}else{$b};Copy-Item $source $path;$paths+=$path
    }
    $hashes=@{};foreach($path in $paths){$hashes[$path]=(Get-FileHash $path).Hash}
    $baseline=Sample 'baseline'
    $endpoints=@()
    for($round=1;$round -le $Rounds;$round++){
        $openPaths=if($round -eq 1){$paths[1..7]}else{$paths}
        Menu-Action 'Berkas' 'Buka PDF…  Ctrl+O'
        Fill-Picker (($openPaths|ForEach-Object {'"'+$_+'"'}) -join ' ')
        $null=Wait-State {param($s) $s.tabCount -eq 8 -and -not $s.tabsOpening}
        $null=Sample 'eight-loaded' $round
        Open-Comparison
        foreach($page in @('20','500','999')){
            Focus-Pdf $leftPane;Page $page
            $null=Wait-State {param($s) $s.activePage -eq [int]$page}
            Focus-Pdf $rightPane;Page $page
            $null=Wait-State {param($s) $s.activePage -eq [int]$page}
            $null=Sample ('split-page-'+$page) $round
        }
        if($HighZoom){
            foreach($name in @($leftPane,$rightPane)){
                Focus-Pdf $name;Invoke 'Zoom';Invoke '400%'
                $null=Wait-State {param($s) $s.zoomPercent -eq 400}
                $null=Sample ('zoom-400-'+$name) $round
            }
            $null=Sample 'both-views-400' $round
            foreach($name in @($leftPane,$rightPane)){
                Focus-Pdf $name
                Menu-Action 'Tampilan' 'Sesuaikan halaman'
                $null=Wait-State {param($s) $s.zoomPercent -ne 400}
            }
        }
        Switch-Tab $paths[7]
        $null=Wait-State {param($s) -not $s.splitView -and $s.visibleTabIds.Count -eq 1}
        $null=Sample 'hidden-pair' $round
        Open-Comparison
        $null=Sample 'split-again' $round
        foreach($path in $paths){
            $before=(Read-Metrics).tabCount
            Invoke ('Tutup tab: '+[IO.Path]::GetFileName($path))
            $null=Wait-State {param($s) $s.tabCount -eq $before-1}
        }
        Start-Sleep -Seconds 17
        $empty=Wait-State {param($s) $s.tabCount -eq 0 -and $s.nativeActiveDocuments -eq 0 -and $s.totalGpuImages -eq 0}
        $endpoints+=Sample 'empty-idle' $round
        Write-Host "PASS comparison profile round $round"
    }
    $privateGrowth=$endpoints[-1].PrivateBytes-$endpoints[0].PrivateBytes
    $handleGrowth=$endpoints[-1].Handles-$endpoints[0].Handles
    $gpuGrowth=if($endpoints[-1].GpuCountersAvailable -and $endpoints[0].GpuCountersAvailable){$endpoints[-1].GpuSharedBytes+$endpoints[-1].GpuDedicatedBytes-$endpoints[0].GpuSharedBytes-$endpoints[0].GpuDedicatedBytes}else{$null}
    if($privateGrowth -gt 64MB -or $handleGrowth -gt 32 -or ($null -ne $gpuGrowth -and $gpuGrowth -gt 64MB)){throw 'Warmed endpoint growth exceeded profiling guard'}
    foreach($path in $paths){if((Get-FileHash $path).Hash -ne $hashes[$path]){throw 'Corpus source changed'}}
    [pscustomobject]@{Rounds=$Rounds;HighZoom=[bool]$HighZoom;SameDocument=[bool]$SameDocument;Samples=$samples.Count;MaxPrivateBytes=($samples|Measure-Object PrivateBytes -Maximum).Maximum;MaxWorkingSetBytes=($samples|Measure-Object WorkingSetBytes -Maximum).Maximum;MaxGpuSharedBytes=($samples|Measure-Object GpuSharedBytes -Maximum).Maximum;PrivateGrowth=$privateGrowth;HandleGrowth=$handleGrowth;GpuCountersAvailable=$gpuAvailable;GpuGrowth=$gpuGrowth;Endpoints=$endpoints;Status='passed'}|ConvertTo-Json -Depth 5|Set-Content -Encoding UTF8 (Join-Path $directory 'summary.json')
    Close-Window
    if(-not $app.WaitForExit(15000)){throw 'Profiling fixture did not close'}
    Write-Host "PASS comparison memory: $directory"
} catch {if($app -and -not $app.HasExited){Capture 'failure.png'};throw}
