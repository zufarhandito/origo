# Only fixture windows are driven by UI Automation and Win32 messages.
param(
    [string]$ExecutablePath='target/tab-background-check/release/pdf-studio.exe',
    [string]$Fixture='.research/ocr-fixtures/batch.pdf',
    [switch]$Complete
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -ExecutablePath $ExecutablePath -TestName 'ocr-batch-ui' -Fixture $Fixture

Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class OcrTrayProbe {
    [StructLayout(LayoutKind.Sequential)] private struct MenuBarInfo {
        public uint size;
        public int left, top, right, bottom;
        public IntPtr menu, menuWindow;
        public uint flags;
    }
    private delegate bool EnumProc(IntPtr hwnd, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr hwnd, StringBuilder name, int max);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr hwnd, int command);
    [DllImport("user32.dll")] private static extern bool GetMenuBarInfo(IntPtr hwnd, int kind, int item, ref MenuBarInfo info);
    [DllImport("user32.dll")] private static extern int GetMenuItemCount(IntPtr menu);
    [DllImport("user32.dll")] private static extern int GetMenuItemID(IntPtr menu, int position);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetMenuString(IntPtr menu, uint position, StringBuilder name, int max, uint flags);
    public static IntPtr OwnedWindow(uint pid, string kind) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((hwnd, _) => {
            uint owner;
            GetWindowThreadProcessId(hwnd, out owner);
            if (owner == pid) {
                var name = new StringBuilder(256);
                GetClassName(hwnd, name, 256);
                if (name.ToString() == kind && (kind != "#32768" || IsWindowVisible(hwnd))) {
                    found = hwnd;
                    return false;
                }
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    public static int MenuCommand(IntPtr popup, string label) {
        var info = new MenuBarInfo { size = (uint)Marshal.SizeOf(typeof(MenuBarInfo)) };
        if (!GetMenuBarInfo(popup, -4, 0, ref info)) return -1; // OBJID_CLIENT
        for (int i = 0; i < GetMenuItemCount(info.menu); ++i) {
            var name = new StringBuilder(256);
            GetMenuString(info.menu, (uint)i, name, name.Capacity, 0x400); // MF_BYPOSITION
            if (name.ToString() == label) return GetMenuItemID(info.menu, i);
        }
        return -1;
    }
}
'@

function Invoke-FixtureTray([string]$Label) {
    $tray=[OcrTrayProbe]::OwnedWindow($app.Id,'tray_icon_app')
    if($tray -eq [IntPtr]::Zero){throw 'Fixture tray icon window missing'}
    # tray-icon 0.26.1 delivers WM_RBUTTONUP through its tray callback.
    [OutputDialog]::PostMessage($tray,6002,[IntPtr]::Zero,[IntPtr]0x205)|Out-Null
    $timeout=(Get-Date).AddSeconds(5)
    $menu=[IntPtr]::Zero
    do {
        Start-Sleep -Milliseconds 100
        $menu=[OcrTrayProbe]::OwnedWindow($app.Id,'#32768')
    } while($menu -eq [IntPtr]::Zero -and (Get-Date) -lt $timeout)
    if($menu -eq [IntPtr]::Zero){throw 'Native OCR tray menu did not open'}
    $command=[OcrTrayProbe]::MenuCommand($menu,$Label)
    [OutputDialog]::PostMessage($tray,0x1F,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null # WM_CANCELMODE
    if($command -le 0){throw "Missing tray menu item $Label"}
    [OutputDialog]::PostMessage($tray,0x111,[IntPtr]$command,[IntPtr]::Zero)|Out-Null # WM_COMMAND
}

try {
    $null=Wait-State {param($s) $s.pageCount -ge 100}
    $hwnd=$app.MainWindowHandle
    Menu-Action 'Berkas' 'OCR seluruh dokumen'
    $started=Wait-State {param($s) $s.ocrBatch.ready -gt 0 -and $s.ocrBatch.remaining -gt 0}
    [OcrTrayProbe]::ShowWindowAsync($hwnd,6)|Out-Null # SW_MINIMIZE
    $minimized=Wait-State {param($s) [OcrTrayProbe]::IsIconic($hwnd) -and $s.ocrBatch.ready -gt $started.ocrBatch.ready}
    if($minimized.ocrInTray){throw 'Minimize unexpectedly sent the viewer to tray'}
    [OcrTrayProbe]::ShowWindowAsync($hwnd,9)|Out-Null # SW_RESTORE
    $null=Wait-State {param($s) -not [OcrTrayProbe]::IsIconic($hwnd)}
    Write-Host 'PASS OCR batch UI: ordinary minimize keeps the batch running'
    Invoke 'Jeda OCR'
    $paused=Wait-State {param($s) $s.ocrBatch.paused -and -not $s.ocrBatch.working}
    Start-Sleep -Milliseconds 500
    if((Read-Metrics).ocrBatch.ready -ne $paused.ocrBatch.ready){throw 'Paused batch progressed'}
    Menu-Action 'Tampilan' 'Halaman terakhir'
    $null=Find 'Butuh OCR'
    Invoke 'Butuh OCR'
    $priority=Wait-State {param($s) $s.ocrPageCached -and $s.ocrBatch.paused -and -not $s.ocrBatch.working}
    if($priority.ocrBatch.ready -ne $paused.ocrBatch.ready+1){throw 'Manual page did not receive priority'}
    Write-Host 'PASS OCR batch UI: pause and prioritize one page during pause'
    Invoke 'Lanjutkan OCR'
    Menu-Action 'Berkas' 'Sembunyikan ke tray'
    $hidden=Wait-State {param($s) $s.ocrInTray}
    if([OcrTrayProbe]::IsWindowVisible($hwnd)){throw 'Main window still visible after tray action'}
    $null=Wait-State {param($s) $s.ocrInTray -and $s.ocrBatch.ready -gt $hidden.ocrBatch.ready}
    Write-Host 'PASS OCR batch UI: native tray hides the same HWND and processing continues'
    Invoke-FixtureTray 'Jeda OCR'
    $null=Wait-State {param($s) $s.ocrInTray -and $s.ocrBatch.paused -and -not $s.ocrBatch.working}
    Invoke-FixtureTray 'Lanjutkan OCR'
    $null=Wait-State {param($s) $s.ocrInTray -and -not $s.ocrBatch.paused}
    Invoke-FixtureTray 'Jeda OCR'
    $null=Wait-State {param($s) $s.ocrInTray -and $s.ocrBatch.paused -and -not $s.ocrBatch.working}
    Invoke-FixtureTray 'Buka Origo'
    $restored=Wait-State {param($s) -not $s.ocrInTray -and $s.ocrBatch.paused}
    if(-not [OcrTrayProbe]::IsWindowVisible($hwnd)){throw 'Tray failed to restore the same HWND'}
    if([OcrTrayProbe]::OwnedWindow($app.Id,'tray_icon_app') -ne [IntPtr]::Zero){throw 'Orphan tray icon window'}
    Write-Host 'PASS OCR batch UI: tray pause, reopen same window, and remove tray icon'

    Invoke 'Lanjutkan OCR'
    $closing=Wait-State {param($s) $s.ocrBatch.working}
    Close-Window
    if(-not $app.WaitForExit(6000)){throw 'Fixture window did not close with an active job'}
    if(Get-CimInstance Win32_Process -Filter "Name='tesseract.exe' AND ParentProcessId=$($app.Id)"){throw 'OCR worker remains after close'}
    $metrics=Join-Path $directory 'metrics-restart.json'
    $argsForFixture='"'+$a+'" --metrics "'+$metrics+'" --state-dir "'+$stateDirectory+'"'
    $app=Start-Process -FilePath $exe -ArgumentList $argsForFixture -WindowStyle Normal -PassThru -WorkingDirectory (Split-Path $exe)
    $end=(Get-Date).AddSeconds(15)
    do {Start-Sleep -Milliseconds 100;$app.Refresh()} while($app.MainWindowHandle -eq 0 -and (Get-Date) -lt $end)
    $hwnd=$app.MainWindowHandle
    $null=Wait-State {param($s) $s.activePage -ge 1 -and $s.pageCount -ge 100}
    Invoke 'Cari teks'
    Invoke 'Lanjutkan OCR dokumen'
    $resumed=Wait-State {param($s) $s.ocrBatch.ready -ge $closing.ocrBatch.ready -and $s.ocrBatch.remaining -gt 0}
    Write-Host 'PASS OCR batch UI: exit cancels worker and restart offers the saved checkpoint'

    if($Complete) {
        (Find 'Cari dalam PDF').GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('Buku ini')
        $during=Wait-State {param($s) $s.searchMatches -gt 0 -and $s.ocrBatch.remaining -gt 0}
        Invoke 'Perbesar'
        $null=Wait-State {param($s) $s.zoomPercent -ne $during.zoomPercent}
        Menu-Action 'Tampilan' 'Halaman terakhir'
        $null=Wait-State {param($s) $s.activePage -eq $s.pageCount -and $s.ocrBatch.remaining -gt 0}
        Menu-Action 'Berkas' 'Sembunyikan ke tray'
        $null=Wait-State {param($s) $s.ocrInTray}
        $watch=[Diagnostics.Stopwatch]::StartNew(); $next=100; $hostPeak=0; $workerPeak=0; $combinedPeak=0
        do {
            $progress=Read-Metrics
            $app.Refresh()
            $hostRam=$app.WorkingSet64
            $worker=Get-CimInstance Win32_Process -Filter "Name='tesseract.exe' AND ParentProcessId=$($app.Id)" | Select-Object -First 1
            $workerRam=if($worker){[long]$worker.WorkingSetSize}else{0}
            $hostPeak=[Math]::Max($hostPeak,$hostRam)
            $workerPeak=[Math]::Max($workerPeak,$workerRam)
            $combinedPeak=[Math]::Max($combinedPeak,$hostRam+$workerRam)
            if($progress.ocrBatch.ready -ge $next) {
                Write-Host ('OCR desktop {0}/{1}, elapsed after restart {2:F1} s' -f $progress.ocrBatch.ready,$progress.ocrBatch.total,$watch.Elapsed.TotalSeconds)
                $next+=100
            }
            if($progress.ocrBatch.paused -or $progress.ocrBatch.failed -gt 0){throw 'Long OCR batch reached an unexpected error'}
            Start-Sleep -Milliseconds 350
        } while($progress.ocrBatch.remaining -gt 0 -and $watch.Elapsed.TotalMinutes -lt 30)
        if($progress.ocrBatch.ready -ne $progress.ocrBatch.total){throw 'Long OCR batch did not complete'}
        $cache=(Get-Item -LiteralPath (Join-Path $stateDirectory 'ocr-v1.sqlite3')).Length
        Write-Host ('PASS long OCR desktop: {0} pages, {1:F1} s after resuming with {2} pages ready, {3} bytes SQLite' -f $progress.ocrBatch.total,$watch.Elapsed.TotalSeconds,$resumed.ocrBatch.ready,$cache)
        Write-Host ('Desktop working set samples: viewer {0:F1}, worker {1:F1}, combined {2:F1} MiB' -f ($hostPeak/1MB),($workerPeak/1MB),($combinedPeak/1MB))
        Invoke-FixtureTray 'Buka Origo'
        $null=Wait-State {param($s) -not $s.ocrInTray -and $s.ocrBatch.remaining -eq 0}
    } else {
        Invoke 'Panel alat'
        Invoke 'Putar kanan'
        $null=Wait-State {param($s) $s.dirty}
        Menu-Action 'Berkas' 'Sembunyikan ke tray'
        $null=Wait-State {param($s) $s.ocrInTray}
        Invoke-FixtureTray 'Keluar'
        $null=Wait-State {param($s) -not $s.ocrInTray}
        $null=Find 'Simpan perubahan sebelum melanjutkan?'
        Invoke 'Batal'
        $null=Wait-State {param($s) $s.dirty -and -not $s.ocrInTray}
        Write-Host 'PASS OCR batch UI: tray exit asks before discarding unsaved PDF changes'
    }
    Close-Window
    if(!$Complete) {
        $null=Find 'Simpan perubahan sebelum melanjutkan?'
        Invoke 'Buang perubahan'
    }
    if(-not $app.WaitForExit(6000)){throw 'Restarted fixture did not close'}
    if((Get-FileHash -LiteralPath $a).Hash -ne $hashA){throw 'Source PDF changed'}
    Write-Host "OCR batch UI evidence: $directory"
} catch {
    if($app.MainWindowHandle -ne 0 -and [OcrTrayProbe]::IsWindowVisible($app.MainWindowHandle)){
        Capture 'failure.png'
    }
    throw
}
