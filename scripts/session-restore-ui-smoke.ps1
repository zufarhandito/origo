param([ValidateSet('debug','release')][string]$Profile='release',[string]$ExecutablePath)
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'session-restore-ui' -IsolatedState -ExecutablePath $ExecutablePath
Add-Type 'using System;using System.Runtime.InteropServices;public class SessionWindow{[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr h);[DllImport("user32.dll")]public static extern bool ShowWindow(IntPtr h,int n);}'
function Activate-Session {[SessionWindow]::ShowWindow($app.MainWindowHandle,9)|Out-Null;[SessionWindow]::SetForegroundWindow($app.MainWindowHandle)|Out-Null;Start-Sleep -Milliseconds 300}
$cases=New-Object Collections.Generic.List[object]
function Pass([string]$Name){$cases.Add([pscustomobject]@{Case=$Name;Status='passed'});$cases.ToArray()|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'cases.json');Write-Host "PASS session restore: $Name"}
function Key([int]$Code){[OutputDialog]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]$Code,[IntPtr]0x10001)|Out-Null;[OutputDialog]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]$Code,[IntPtr]0xC0010001)|Out-Null;Start-Sleep -Milliseconds 200}
function Page([string]$Number){$field=Find 'Nomor halaman tujuan';$field.SetFocus();$field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue($Number);Key 13}
function Stop-Session {Close-Window;if(-not $app.WaitForExit(15000)){throw 'Fixture did not close'}}
function Start-Session([switch]$WithFile) {
    Remove-Item -LiteralPath $metrics -ErrorAction SilentlyContinue
    $launchArguments=if($WithFile){'"'+$a+'" '}else{''}
    Start-Process -FilePath $exe -ArgumentList ($launchArguments+'--metrics "'+$metrics+'" --state-dir "'+$stateDirectory+'"') -WorkingDirectory (Split-Path $exe) -WindowStyle Normal|Out-Null
    $deadline=(Get-Date).AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 200
        $owned=Get-CimInstance Win32_Process -Filter "Name='pdf-studio.exe'"|Where-Object {$_.ExecutablePath -eq [IO.Path]::GetFullPath($exe) -and $_.CommandLine.Contains([IO.Path]::GetFileName($directory))}|Select-Object -First 1
        if($owned){$script:app=Get-Process -Id $owned.ProcessId;$app.Refresh()}
    }while((Get-Date) -lt $deadline -and (-not $app -or $app.HasExited -or $app.MainWindowHandle -eq 0))
    if(-not $app -or $app.HasExited -or $app.MainWindowHandle -eq 0){throw 'Restart window unavailable'}
    Activate-Session
}
try {
    $null=Wait-State {param($s) $s.tabCount -eq 1 -and $s.sessionReady}
    Menu-Action 'Tampilan' 'Sesuaikan halaman';Page '3'
    $null=Wait-State {param($s) $s.activePage -eq 3}
    Picker 'Buka PDF';Fill-Picker $b
    $null=Wait-State {param($s) $s.tabCount -eq 2}
    Menu-Action 'Tampilan' 'Sesuaikan halaman';Page '2'
    $null=Wait-State {param($s) $s.activePage -eq 2}
    Switch-Tab $a
    Menu-Action 'Alat' 'Atur halaman';Invoke 'Putar kanan'
    $c=Join-Path $directory 'C.pdf';Copy-Item (Join-Path $root 'tests/fixtures/text-selection.pdf') $c
    Menu-Action 'Alat' 'Impor PDF…';Fill-Picker $c
    $null=Wait-State {param($s) $s.sourceCount -eq 2 -and $s.pageCount -eq 8 -and $s.dirty}
    Page '3'
    $null=Wait-State {param($s) $s.activePage -eq 3}
    Close-Window
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Buang perubahan'
    if(-not $app.WaitForExit(15000)){throw 'Dirty session did not close'}
    $sessionPath=Join-Path $stateDirectory 'session.json'
    $savedMetadata=[IO.File]::ReadAllText($sessionPath)
    $snapshot=$savedMetadata|ConvertFrom-Json
    if($snapshot.documents.Count -ne 2 -or $snapshot.active -ne 0 -or $snapshot.documents[0].path -ne $a -or $snapshot.documents[0].PSObject.Properties.Name -contains 'history'){throw 'Reading-only session metadata invalid'}
    Pass 'metadata-only-dirty-and-multi-source-session'
    Start-Session
    $null=Wait-State {param($s) $s.sessionRestoreOffered -and $s.tabCount -eq 0}
    $restore=(Find 'Pulihkan Sesi').Current.BoundingRectangle;$dismiss=(Find 'Tutup tawaran sesi').Current.BoundingRectangle
    $rect=New-Object M3Capture+Rect;[M3Capture]::GetWindowRect($app.MainWindowHandle,[ref]$rect)|Out-Null
    if($restore.X -lt $rect.L+($rect.R-$rect.L)*0.65 -or $dismiss.X -le $restore.Right){throw 'Restore and close must be on the right'}
    $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
    $banner=$ui.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.Name -like 'Lanjutkan sesi baca terakhir*'}|Select-Object -First 1
    if($banner.Current.BoundingRectangle.Height -gt 80){throw 'Restore banner is not compact'}
    Capture 'compact-offer-light.png';Pass 'compact-offer-actions-right'
    Picker 'Buka PDF';Fill-Picker $a -Cancel
    $null=Wait-State {param($s) $s.sessionRestoreOffered -and $s.tabCount -eq 0};Pass 'cancel-picker-keeps-offer'
    Activate-Session
    Invoke 'Buka terakhir: A.pdf'
    $null=Wait-State {param($s) $s.tabCount -eq 1 -and -not $s.sessionRestoreOffered}
    Invoke 'Tutup tab: A.pdf';$null=Wait-State {param($s) $s.tabCount -eq 0 -and -not $s.sessionRestoreOffered}
    Start-Sleep -Milliseconds 1200
    if((Read-Metrics).sessionRestoreOffered){throw 'Restore offer reappeared after manual recent open'}
    Pass 'recent-open-dismisses-offer-without-resurrection';Stop-Session
    [IO.File]::WriteAllText($sessionPath,$savedMetadata,[Text.UTF8Encoding]::new($false))
    Start-Session;$null=Wait-State {param($s) $s.sessionRestoreOffered}
    Menu-Action 'Tampilan' 'Gelap';Capture 'compact-offer-dark.png'
    Picker 'Buka PDF';Fill-Picker $b
    $null=Wait-State {param($s) $s.tabCount -eq 1 -and -not $s.sessionRestoreOffered};Pass 'picker-open-dismisses-offer';Stop-Session
    [IO.File]::WriteAllText($sessionPath,$savedMetadata,[Text.UTF8Encoding]::new($false))
    Start-Session
    $null=Wait-State {param($s) $s.sessionRestoreOffered -and $s.tabCount -eq 0}
    Invoke 'Pulihkan Sesi'
    $null=Wait-State {param($s) $s.tabCount -eq 2 -and $s.tabId -eq 1 -and $s.activePage -eq 3 -and $s.pageCount -eq 4 -and $s.sourceCount -eq 1 -and -not $s.dirty -and $s.pagePlan[0][2] -eq 0}
    Switch-Tab $b
    $null=Wait-State {param($s) $s.activePage -eq 2 -and -not $s.dirty}
    Pass 'offer-order-active-position-and-no-edit-recovery'
    Capture 'restored-session.png'
    Stop-Session
    Remove-Item -LiteralPath $b
    [IO.File]::SetLastWriteTimeUtc($a,[DateTime]::UtcNow.AddMinutes(1))
    Start-Session
    $null=Wait-State {param($s) $s.sessionRestoreOffered}
    Invoke 'Pulihkan Sesi'
    $null=Wait-State {param($s) $s.tabCount -eq 1 -and $s.activePage -eq 1 -and -not $s.tabsOpening}
    Pass 'partial-restore-and-changed-fingerprint-reset'
    Invoke 'Tutup tab: A.pdf'
    $null=Wait-State {param($s) $s.tabCount -eq 0 -and $s.pendingPreferences -eq 0}
    Stop-Session;Start-Session
    $null=Wait-State {param($s) $s.sessionReady -and -not $s.sessionRestoreOffered -and $s.tabCount -eq 0}
    Stop-Session
    Pass 'last-tab-close-persists-empty-session'
    [IO.File]::WriteAllText($sessionPath,'{broken',[Text.UTF8Encoding]::new($false))
    Start-Session
    $null=Find 'Sesi tersimpan tidak dapat dibaca. Pemulihan dilewati; penyimpanan sesi dinonaktifkan agar metadata lama tetap aman. Hapus riwayat sesi untuk mengaktifkannya kembali.'
    if([IO.File]::ReadAllText($sessionPath) -ne '{broken'){throw 'Corrupt metadata was overwritten'}
    Menu-Action 'Berkas' 'Hapus riwayat sesi'
    $null=Wait-State {param($s) $s.sessionReady -and -not $s.sessionRecording -and $s.pendingPreferences -eq 0}
    Click-Control 'Buka PDF';Fill-Picker $a
    $null=Wait-State {param($s) $s.tabCount -eq 1 -and -not $s.sessionRecording}
    Stop-Session;Start-Session
    $null=Wait-State {param($s) $s.sessionReady -and -not $s.sessionRestoreOffered -and $s.tabCount -eq 0}
    Stop-Session
    Pass 'corrupt-session-preserved-and-explicit-clear'
    [IO.File]::WriteAllText($sessionPath,$savedMetadata,[Text.UTF8Encoding]::new($false))
    Start-Session
    $null=Wait-State {param($s) $s.sessionRestoreOffered}
    Invoke 'Tutup tawaran sesi'
    $null=Wait-State {param($s) -not $s.sessionRestoreOffered -and $s.pendingPreferences -eq 0}
    Stop-Session;Start-Session
    $null=Wait-State {param($s) $s.sessionReady -and -not $s.sessionRestoreOffered -and $s.tabCount -eq 0}
    Stop-Session
    Pass 'dismiss-restore-is-remembered'
    Start-Session -WithFile
    $null=Wait-State {param($s) $s.sessionReady -and $s.tabCount -eq 1 -and $s.pendingPreferences -eq 0}
    Remove-Item -LiteralPath $sessionPath -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Path $sessionPath|Out-Null
    Page '2'
    $null=Wait-State {param($s) $s.activePage -eq 2}
    # Observe the failed background checkpoint before closing. Otherwise the
    # one-second poll may report it first and Close is already the allowed retry.
    $null=Find 'Sesi baca gagal disimpan. PDF tidak berubah. Tutup lagi untuk keluar tanpa menyimpan sesi, atau ubah posisi baca untuk mencoba lagi.'
    $null=Wait-State {param($s) $s.tabCount -eq 1 -and -not $s.closingTabs -and $s.pendingPreferences -eq 0}
    Stop-Session
    Pass 'write-failure-is-visible-and-exit-can-be-retried'
    if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $c).Hash -ne $hashA){throw 'Source PDF bytes changed'}
    Pass 'source-bytes-unchanged'
    Write-Host "PASS session restore UI: $directory"
} catch {if($app -and -not $app.HasExited){Capture 'failure.png'};throw}
