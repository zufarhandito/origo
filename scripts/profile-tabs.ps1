# Repeatable generated-book process/GPU profiling. Targets only its own PID/HWND.
param([ValidateSet('debug','release')][string]$Profile='release',[ValidateRange(2,5)][int]$Rounds=3,[switch]$HighZoom)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'tabs-memory' -Fixture '.research/book-benchmark/vector-1000.pdf'
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
        Phase=$Phase;Round=$Round;Utc=[DateTime]::UtcNow.ToString('o');TabCount=$s.tabCount
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
function Close-Tabs {
    do {
        $s=Read-Metrics
        if($s.tabCount -eq 0){break}
        $before=$s.tabCount
        Menu-Action 'Berkas' 'Tutup tab  Ctrl+W'
        $null=Wait-State {param($s) $s.tabCount -eq $before-1}
    } while($true)
}
function Wait-NativeIdle {
    $deadline=(Get-Date).AddSeconds(24)
    do { Start-Sleep -Milliseconds 500; $s=Read-Metrics } while($s.nativeActiveDocuments -ne 0 -and (Get-Date) -lt $deadline)
    if($s.nativeActiveDocuments -ne 0){throw 'PDFium retained a document beyond its idle timeout'}
    Start-Sleep -Seconds 1
}
try {
    $null=Wait-State {param($s) $s.pageCount -eq 1000}
    $files=@();$hashes=@{}
    1..8|ForEach-Object {
        $file=Join-Path $directory "Book-$_.pdf"
        $kind=if($_ % 2 -eq 0){'simulated-scan-1000.pdf'}else{'vector-1000.pdf'}
        Copy-Item -LiteralPath (Join-Path $root ".research/book-benchmark/$kind") -Destination $file
        $files+=$file;$hashes[$file]=(Get-FileHash -LiteralPath $file).Hash
    }
    Close-Tabs;Wait-NativeIdle
    $baseline=Sample 'baseline-empty'
    $ends=@()
    for($round=1;$round -le $Rounds;$round++) {
        Picker 'Buka PDF';Fill-Picker (($files|ForEach-Object {'"'+$_+'"'}) -join ' ')
        $null=Wait-State {param($s) $s.tabCount -eq 8 -and -not $s.tabsOpening -and $s.pageCount -eq 1000}
        $null=Sample 'eight-loaded' $round
        foreach($sweep in 1..2) {
            foreach($file in $files) {
                Switch-Tab $file
                $null=Wait-State {param($s) $s.pageCount -eq 1000 -and -not $s.textLoading}
                $null=Sample "sweep-$sweep" $round
            }
        }
        if($HighZoom) {
            # Exercise the larger raster/texture allocations too, once for each
            # generated source kind, then return to fit before closing.
            foreach($file in $files[0..1]) {
                Switch-Tab $file
                Invoke 'Zoom';Invoke '400%'
                $null=Wait-State {param($s) $s.zoomPercent -eq 400 -and -not $s.textLoading}
                $null=Sample 'high-zoom-400' $round
                Invoke 'Zoom';Invoke 'Sesuaikan halaman'
                $null=Wait-State {param($s) -not $s.textLoading}
            }
        }
        Close-Tabs;Wait-NativeIdle
        $ends+=Sample 'round-empty-idle' $round
        Write-Host "Profile round $round/${Rounds}: private $([math]::Round($ends[-1].PrivateBytes/1MB,1)) MiB, GPU shared $([math]::Round($ends[-1].GpuSharedBytes/1MB,1)) MiB"
    }
    # Allocator/driver pools may grow during warm-up. Compare later identical
    # rounds with the first warmed empty state, rather than demanding zero memory.
    $privateGrowth=$ends[-1].PrivateBytes-$ends[0].PrivateBytes
    $gpuGrowth=if($ends[-1].GpuCountersAvailable -and $ends[0].GpuCountersAvailable){($ends[-1].GpuSharedBytes+$ends[-1].GpuDedicatedBytes)-($ends[0].GpuSharedBytes+$ends[0].GpuDedicatedBytes)}else{$null}
    if($privateGrowth -gt 64MB -or ($null -ne $gpuGrowth -and $gpuGrowth -gt 64MB) -or $ends[-1].Handles-$ends[0].Handles -gt 32){throw 'Repeated identical open/switch/close rounds did not plateau within profiling tolerances'}
    foreach($file in $files){if((Get-FileHash -LiteralPath $file).Hash -ne $hashes[$file]){throw 'Generated source was modified'}}
    $summary=[pscustomobject]@{
        Profile=$Profile;Rounds=$Rounds;HighZoom=[bool]$HighZoom;Tabs=8;PagesPerSource=1000;VectorSources=4;SimulatedScanSources=4
        PrivateGrowthAfterWarmupBytes=$privateGrowth;GpuGrowthAfterWarmupBytes=$gpuGrowth
        PeakWorkingSetBytes=($samples|Measure-Object PeakWorkingSetBytes -Maximum).Maximum
        MaxPrivateBytes=($samples|Measure-Object PrivateBytes -Maximum).Maximum
        MaxGpuSharedBytes=($samples|Measure-Object GpuSharedBytes -Maximum).Maximum
        MaxGpuDedicatedBytes=($samples|Measure-Object GpuDedicatedBytes -Maximum).Maximum
        GpuCountersAvailable=$gpuAvailable;Baseline=$baseline;RoundIdle=$ends
        ToleranceBytes=64MB;Notes='Private bytes include Rust/native/framework/allocator memory; they are not a PDFium-only measurement. Generated corpus, one machine, no universal leak/fidelity claim.'
    }
    $summary|ConvertTo-Json -Depth 6|Set-Content -Encoding UTF8 (Join-Path $directory 'summary.json')
    Write-Host "PASS 8-tab memory profile: $directory"
} finally {
    if($app -and -not $app.HasExited){Close-Window;$null=$app.WaitForExit(10000)}
}
