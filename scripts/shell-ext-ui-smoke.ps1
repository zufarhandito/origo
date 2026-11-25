param([ValidateSet('debug','release')][string]$Profile='debug',[string]$ExecutablePath)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$exe=if($ExecutablePath){(Resolve-Path -LiteralPath $ExecutablePath).Path}else{Join-Path $root "target\$Profile\pdf-studio.exe"}
if(-not (Test-Path -LiteralPath $exe)){throw "Executable not found: $exe"}

# 1. Verify Sparse MSIX packaging & signing (no elevation needed for -PackageOnly)
& (Join-Path $PSScriptRoot 'register-shell-ext-dev.ps1') -Profile $Profile -SkipBuild -PackageOnly | Out-Host
$msixPath=Join-Path $env:LOCALAPPDATA 'Origo\ShellExtDev\OrigoShellExt.msix'
if(-not (Test-Path -LiteralPath $msixPath)){throw 'Sparse MSIX was not produced'}
$sig=Get-AuthenticodeSignature -FilePath $msixPath
if(-not $sig.SignerCertificate -or $sig.SignerCertificate.Subject -ne 'CN=Origo Dev'){throw "Unexpected MSIX signer: $($sig.SignerCertificate.Subject)"}
Write-Host 'PASS shell-ext: sparse MSIX packed and signed (CN=Origo Dev)'

# Prepare isolated test workspace and synthetic images inside a named folder
$directory=(New-Item -ItemType Directory -Path (Join-Path $root ('.research/shell-ext-ui-' + [guid]::NewGuid().ToString('N')))).FullName
$stateDir=(New-Item -ItemType Directory -Path (Join-Path $directory 'state')).FullName
$photoDir=(New-Item -ItemType Directory -Path (Join-Path $directory 'Foto Proyek')).FullName
$metrics=Join-Path $directory 'metrics.json'

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
function New-TestImage([string]$Path,[int]$Width,[int]$Height,[Drawing.Color]$Color,[Drawing.Imaging.ImageFormat]$Format) {
    $bmp=New-Object Drawing.Bitmap($Width,$Height)
    try {
        $g=[Drawing.Graphics]::FromImage($bmp)
        try { $g.Clear($Color) } finally { $g.Dispose() }
        $bmp.Save($Path,$Format)
    } finally { $bmp.Dispose() }
}
$img1=Join-Path $photoDir '01-portrait.png'
$img2=Join-Path $photoDir '02-landscape.jpg'
New-TestImage $img1 320 480 ([Drawing.Color]::FromArgb(255,35,95,215)) ([Drawing.Imaging.ImageFormat]::Png)
New-TestImage $img2 480 320 ([Drawing.Color]::FromArgb(255,215,55,45)) ([Drawing.Imaging.ImageFormat]::Jpeg)
$hash1=(Get-FileHash $img1).Hash
$hash2=(Get-FileHash $img2).Hash

$manifest1=Join-Path $directory 'cold-dialog-list.txt'
[IO.File]::WriteAllLines($manifest1, @($img1, $img2), (New-Object Text.UTF8Encoding($false)))

if(-not ('ShellSmokeWin' -as [type])){
    Add-Type 'using System;using System.Runtime.InteropServices;public static class ShellSmokeWin{[StructLayout(LayoutKind.Sequential)]public struct Rect{public int L,T,R,B;}[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);[DllImport("user32.dll")]public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint f);[DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);}'
}
function Read-Metrics {
    for($attempt=0;$attempt -lt 60;$attempt++) {
        if(Test-Path -LiteralPath $metrics){
            try { return Get-Content -Raw $metrics | ConvertFrom-Json } catch { Start-Sleep -Milliseconds 30 }
        }
        Start-Sleep -Milliseconds 50
    }
    throw 'Metrics file did not become readable'
}
function Wait-State([scriptblock]$Cond,[int]$TimeoutSec=20) {
    $deadline=(Get-Date).AddSeconds($TimeoutSec)
    $last=$null
    while((Get-Date) -lt $deadline) {
        if($app.HasExited){throw "App exited early with code $($app.ExitCode)"}
        try {
            if(Test-Path -LiteralPath $metrics){
                $last=Get-Content -Raw $metrics | ConvertFrom-Json
                if(& $Cond $last){return $last}
            }
        } catch {}
        Start-Sleep -Milliseconds 80
    }
    throw "Timeout waiting for state: $($last | ConvertTo-Json -Compress)"
}
function Find-Node([string]$Name,[int]$TimeoutSec=10) {
    $deadline=(Get-Date).AddSeconds($TimeoutSec)
    while((Get-Date) -lt $deadline) {
        $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
        $cond=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Name)
        $node=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,$cond)
        if($node){return $node}
        Start-Sleep -Milliseconds 100
    }
    throw "UI node not found: $Name"
}
function Invoke-Node([string]$Name) {
    $node=Find-Node $Name
    $node.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
    Start-Sleep -Milliseconds 120
}
function Capture([string]$Name) {
    $rect=New-Object ShellSmokeWin+Rect
    [ShellSmokeWin]::GetWindowRect($app.MainWindowHandle,[ref]$rect)|Out-Null
    $bitmap=New-Object Drawing.Bitmap(($rect.R-$rect.L),($rect.B-$rect.T))
    $graphics=[Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
    try { $null=[ShellSmokeWin]::PrintWindow($app.MainWindowHandle,$dc,2) } finally { $graphics.ReleaseHdc($dc);$graphics.Dispose() }
    try { $bitmap.Save((Join-Path $directory $Name)) } finally { $bitmap.Dispose() }
}

$app=$null
try {
    # 2. Cold launch via --images-to-pdf-list --delete-list
    $app=Start-Process -FilePath $exe -ArgumentList @('--images-to-pdf-list', $manifest1, '--delete-list', '--metrics', $metrics, '--state-dir', $stateDir) -WorkingDirectory (Split-Path $exe) -PassThru
    $deadline=(Get-Date).AddSeconds(15)
    while(-not $app.HasExited -and $app.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline){ Start-Sleep -Milliseconds 100; $app.Refresh() }
    if($app.HasExited -or $app.MainWindowHandle -eq 0){throw 'App window did not appear'}

    $null=Wait-State {param($s) $s.imagesPdfDialog}
    if(Test-Path -LiteralPath $manifest1){throw 'Temporary manifest file was not deleted by --delete-list'}
    $null=Find-Node '1. 01-portrait.png'
    $null=Find-Node '2. 02-landscape.jpg'
    Capture 'cold-launch-prefilled-dialog.png'
    Write-Host 'PASS shell-ext UI: cold-launch-prefilled-dialog'

    Invoke-Node 'Batal'
    $null=Wait-State {param($s) -not $s.imagesPdfDialog}

    # 3. Warm IPC quick-convert while Origo is already running
    $manifest2=Join-Path $directory 'ipc-quick-list.txt'
    [IO.File]::WriteAllLines($manifest2, @($img1, $img2), (New-Object Text.UTF8Encoding($false)))
    $before=(Read-Metrics).imagesPdfCompleted
    $sender=Start-Process -FilePath $exe -ArgumentList @('--images-to-pdf-quick-list', $manifest2, '--delete-list', '--state-dir', $stateDir) -WorkingDirectory (Split-Path $exe) -PassThru
    if(-not $sender.WaitForExit(6000)){ $sender.Kill(); throw 'IPC forwarder did not exit promptly' }
    if($sender.ExitCode -ne 0){ throw "IPC forwarder failed with exit code $($sender.ExitCode)" }
    if(Test-Path -LiteralPath $manifest2){throw 'Quick manifest file was not deleted'}

    $done1=Wait-State {param($s) -not $s.imagesPdfRunning -and $s.imagesPdfCompleted -gt $before -and $s.tabCount -eq 1 -and $s.pageCount -eq 2}
    if(-not $done1.imagesPdfSuccess -or $done1.imagesPdfPages -ne 2){throw 'Quick convert failed'}
    $pdf1=Join-Path $photoDir 'Foto Proyek.pdf'
    if(-not (Test-Path -LiteralPath $pdf1)){throw "Expected quick PDF missing: $pdf1"}
    $pdf1Hash=(Get-FileHash $pdf1).Hash
    Capture 'ipc-quick-convert-auto-open.png'
    Write-Host 'PASS shell-ext UI: ipc-quick-convert-auto-open'

    # 4. Warm IPC second quick-convert must pick non-clobber suffix (-2.pdf) and preserve first PDF
    $manifest3=Join-Path $directory 'ipc-quick-list-2.txt'
    [IO.File]::WriteAllLines($manifest3, @($img1, $img2), (New-Object Text.UTF8Encoding($false)))
    $before2=(Read-Metrics).imagesPdfCompleted
    $sender2=Start-Process -FilePath $exe -ArgumentList @('--images-to-pdf-quick-list', $manifest3, '--delete-list', '--state-dir', $stateDir) -WorkingDirectory (Split-Path $exe) -PassThru
    if(-not $sender2.WaitForExit(6000)){ $sender2.Kill(); throw 'Second IPC forwarder did not exit promptly' }
    $done2=Wait-State {param($s) -not $s.imagesPdfRunning -and $s.imagesPdfCompleted -gt $before2 -and $s.tabCount -eq 2}
    if(-not $done2.imagesPdfSuccess){throw 'Second quick convert failed'}
    $pdf2=Join-Path $photoDir 'Foto Proyek-2.pdf'
    if(-not (Test-Path -LiteralPath $pdf2)){throw "Expected no-clobber PDF missing: $pdf2"}
    if((Get-FileHash $pdf1).Hash -ne $pdf1Hash){throw 'Original quick PDF was overwritten'}
    Write-Host 'PASS shell-ext UI: ipc-quick-convert-noclobber-suffix'

    # 5. Warm IPC dialog forwarding while 2 tabs are open
    $manifest4=Join-Path $directory 'ipc-dialog-list.txt'
    [IO.File]::WriteAllLines($manifest4, @($img2), (New-Object Text.UTF8Encoding($false)))
    $sender3=Start-Process -FilePath $exe -ArgumentList @('--images-to-pdf-list', $manifest4, '--delete-list', '--state-dir', $stateDir) -WorkingDirectory (Split-Path $exe) -PassThru
    if(-not $sender3.WaitForExit(6000)){ $sender3.Kill(); throw 'Dialog IPC forwarder did not exit promptly' }
    $dlgState=Wait-State {param($s) $s.imagesPdfDialog -and $s.tabCount -eq 2}
    $null=Find-Node '1. 02-landscape.jpg'
    Capture 'ipc-warm-prefilled-dialog.png'
    Invoke-Node 'Batal'
    $null=Wait-State {param($s) -not $s.imagesPdfDialog}
    Write-Host 'PASS shell-ext UI: ipc-warm-prefilled-dialog'

    # 6. Warm IPC 1-click PDF combine (no-clobber: Foto Proyek-3.pdf) combining $pdf1 (2p) + $pdf2 (2p) = 4p
    $pdf2Hash=(Get-FileHash $pdf2).Hash
    $manifest5=Join-Path $directory 'ipc-merge-quick-list.txt'
    [IO.File]::WriteAllLines($manifest5, @($pdf1, $pdf2), (New-Object Text.UTF8Encoding($false)))
    $mergeBefore=(Read-Metrics).pdfMergeCompleted
    $sender4=Start-Process -FilePath $exe -ArgumentList @('--merge-pdf-quick-list', $manifest5, '--delete-list', '--state-dir', $stateDir) -WorkingDirectory (Split-Path $exe) -PassThru
    if(-not $sender4.WaitForExit(6000)){ $sender4.Kill(); throw 'Quick PDF merge IPC forwarder did not exit promptly' }
    if($sender4.ExitCode -ne 0){ throw "Quick PDF merge forwarder failed with exit code $($sender4.ExitCode)" }
    if(Test-Path -LiteralPath $manifest5){throw 'Merge quick manifest was not deleted'}
    $mergeDone=Wait-State {param($s) -not $s.pdfMergeRunning -and $s.pdfMergeCompleted -gt $mergeBefore -and $s.tabCount -eq 3 -and $s.pageCount -eq 4}
    if(-not $mergeDone.pdfMergeSuccess -or $mergeDone.pdfMergePages -ne 4){throw 'Quick PDF combine failed'}
    $mergedPdf=Join-Path $photoDir 'Foto Proyek-3.pdf'
    if(-not (Test-Path -LiteralPath $mergedPdf)){throw "Expected no-clobber merged PDF missing: $mergedPdf"}
    if((Get-FileHash $pdf1).Hash -ne $pdf1Hash -or (Get-FileHash $pdf2).Hash -ne $pdf2Hash){throw 'Source PDFs were modified'}
    Capture 'ipc-pdf-quick-combine.png'
    Write-Host 'PASS shell-ext UI: ipc-pdf-quick-combine-noclobber'

    # 7. Warm IPC combine PDFs in Organizer mode ($pdf1 + $pdf2 -> 4 pages, 2 sources, organizing=true)
    $manifest6=Join-Path $directory 'ipc-merge-org-list.txt'
    [IO.File]::WriteAllLines($manifest6, @($pdf1, $pdf2), (New-Object Text.UTF8Encoding($false)))
    $sender5=Start-Process -FilePath $exe -ArgumentList @('--merge-pdf-list', $manifest6, '--delete-list', '--state-dir', $stateDir) -WorkingDirectory (Split-Path $exe) -PassThru
    if(-not $sender5.WaitForExit(6000)){ $sender5.Kill(); throw 'Organizer PDF merge IPC forwarder did not exit promptly' }
    $orgState=Wait-State {param($s) $s.tabCount -eq 4 -and $s.organizing -and $s.sourceCount -eq 2 -and $s.pageCount -eq 4}
    Capture 'ipc-pdf-combine-organizer.png'
    Write-Host 'PASS shell-ext UI: ipc-pdf-combine-organizer'

    # 8. Warm IPC single-PDF Split action (--pdf-split-list -> activates tab & opens Split PDF dialog)
    $manifest7=Join-Path $directory 'ipc-pdf-split-list.txt'
    [IO.File]::WriteAllLines($manifest7, @($pdf1), (New-Object Text.UTF8Encoding($false)))
    $sender6=Start-Process -FilePath $exe -ArgumentList @('--pdf-split-list', $manifest7, '--delete-list', '--state-dir', $stateDir) -WorkingDirectory (Split-Path $exe) -PassThru
    if(-not $sender6.WaitForExit(6000)){ $sender6.Kill(); throw 'Split PDF IPC forwarder did not exit promptly' }
    $splitState=Wait-State {param($s) $s.splitDialog}
    Capture 'ipc-pdf-split-dialog.png'
    Invoke-Node 'Batal'
    $null=Wait-State {param($s) -not $s.splitDialog}
    Write-Host 'PASS shell-ext UI: ipc-pdf-split-dialog'

    if((Get-FileHash $img1).Hash -ne $hash1 -or (Get-FileHash $img2).Hash -ne $hash2){throw 'Source images modified'}
    Write-Host "Shell-Ext UI evidence: $directory"
} catch {
    if($app -and -not $app.HasExited -and $app.MainWindowHandle -ne 0){ Capture 'failure.png' }
    throw
} finally {
    if($app -and -not $app.HasExited){
        for($attempt=0;$attempt -lt 8 -and -not $app.HasExited;$attempt++){
            [ShellSmokeWin]::PostMessage($app.MainWindowHandle,0x10,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
            if($app.WaitForExit(1000)){ break }
        }
        if(-not $app.HasExited){ $app.Kill() }
    }
}
