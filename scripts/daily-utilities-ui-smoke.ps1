param([ValidateSet('debug','release')][string]$Profile='debug')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$ell=[char]0x2026
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$directory=Join-Path $root ('.research/daily-utilities-ui-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $directory | Out-Null
$pdf=Join-Path $directory 'sample.pdf'
Copy-Item (Join-Path $root 'tests/fixtures/text-selection.pdf') $pdf
$metrics=Join-Path $directory 'metrics.json'
$stateDir=Join-Path $directory 'state'
$exe=Join-Path $root "target/$Profile/pdf-studio.exe"
if(-not (Test-Path -LiteralPath $exe)){throw "Missing binary: $exe"}

$oldIds=@(Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | ForEach-Object Id)
$desktop=New-Object -ComObject Shell.Application
$desktop.ShellExecute($exe,('"'+$pdf+'" --metrics "'+$metrics+'" --state-dir "'+$stateDir+'"'),$root,'open',1)

$deadline=(Get-Date).AddSeconds(20)
do {
    Start-Sleep -Milliseconds 200
    $app=Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | Where-Object Id -NotIn $oldIds | Select-Object -First 1
    if($app){$app.Refresh()}
} while((Get-Date) -lt $deadline -and (-not $app -or $app.MainWindowHandle -eq 0))
if(-not $app -or $app.MainWindowHandle -eq 0){throw 'Visible test window unavailable'}

Add-Type 'using System;using System.Runtime.InteropServices;public static class DailyUtilClick{[StructLayout(LayoutKind.Sequential)]public struct Point{public int X,Y;}[DllImport("user32.dll")]public static extern bool ScreenToClient(IntPtr h,ref Point p);[DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);[DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);}'

function Wait-State([scriptblock]$Predicate, [int]$Seconds=25) {
    $d=(Get-Date).AddSeconds($Seconds)
    $s=$null
    do {
        Start-Sleep -Milliseconds 150
        if($app.HasExited){throw 'Viewer exited unexpectedly'}
        try {$s=Get-Content -Raw $metrics | ConvertFrom-Json} catch {continue}
        if(& $Predicate $s){return $s}
    } while((Get-Date) -lt $d)
    throw "Timeout waiting for state. Last: $($s|ConvertTo-Json -Compress)"
}

function Find($Name) {
    $d=(Get-Date).AddSeconds(10)
    do {
        $app.Refresh()
        if($app.HasExited){throw "Viewer exited while finding $Name"}
        $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
        $node=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Name))
        if($node){return $node}
        Start-Sleep -Milliseconds 100
    } while((Get-Date) -lt $d)
    throw "Missing UI control: $Name"
}

function Invoke-Control($Name) {
    $n=Find $Name
    $pattern=$null
    if($n.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)) {
        $pattern.Invoke()
    } else {
        $r=$n.Current.BoundingRectangle
        $pt=New-Object DailyUtilClick+Point
        $pt.X=[int]($r.X+$r.Width/2)
        $pt.Y=[int]($r.Y+$r.Height/2)
        [DailyUtilClick]::ScreenToClient($app.MainWindowHandle,[ref]$pt)|Out-Null
        $lp=[IntPtr](($pt.Y -shl 16) -bor ($pt.X -band 0xffff))
        [DailyUtilClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
        [DailyUtilClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$lp)|Out-Null
    }
    Start-Sleep -Milliseconds 250
}

function Invoke-Menu([string]$Group, [string]$Item) {
    Invoke-Control 'Menu aplikasi'
    Invoke-Control $Group
    Invoke-Control $Item
}

try {
    $s=Wait-State {param($x) -not $x.opening -and -not $x.rendering -and $x.pageCount -eq 4}

    # 1. Page Numbers: open dialog, apply to all 4 pages, verify 1-step Undo & Redo
    Invoke-Menu 'Alat' ("Nomor Halaman$ell")
    $s=Wait-State {param($x) $x.pageNumbersDialog -eq $true}
    Invoke-Control 'Terapkan Nomor Halaman'
    $s=Wait-State {param($x) -not $x.pageNumbersDialog -and $x.textAnnotationCount -eq 4 -and $x.dirty}
    Write-Host 'PASS daily-utilities UI: page-numbers-applied-all-pages'

    Invoke-Menu 'Edit' 'Undo  Ctrl+Z'
    $s=Wait-State {param($x) $x.textAnnotationCount -eq 0}
    Invoke-Menu 'Edit' 'Redo  Ctrl+Y'
    $s=Wait-State {param($x) $x.textAnnotationCount -eq 4}
    Write-Host 'PASS daily-utilities UI: page-numbers-single-step-undo-redo'

    # 2. Watermark: open dialog, apply diagonal watermark to all 4 pages, verify 1-step Undo & Redo
    Invoke-Menu 'Alat' ("Watermark$ell")
    $s=Wait-State {param($x) $x.watermarkDialog -eq $true}
    Invoke-Control 'Terapkan Watermark'
    $s=Wait-State {param($x) -not $x.watermarkDialog -and $x.signatureCount -eq 4}
    Write-Host 'PASS daily-utilities UI: watermark-applied-all-pages'

    Invoke-Menu 'Edit' 'Undo  Ctrl+Z'
    $s=Wait-State {param($x) $x.signatureCount -eq 0}
    Invoke-Menu 'Edit' 'Redo  Ctrl+Y'
    $s=Wait-State {param($x) $x.signatureCount -eq 4}
    Write-Host 'PASS daily-utilities UI: watermark-single-step-undo-redo'

    # 3. Protect PDF dialog opens & closes cleanly
    Invoke-Menu 'Alat' ("Lindungi PDF (Password)$ell")
    $s=Wait-State {param($x) $x.securityDialog -eq $true}
    $null=Find 'Lindungi PDF dengan Password (AES-256)'
    Invoke-Control 'Batal'
    $s=Wait-State {param($x) -not $x.securityDialog}
    Write-Host 'PASS daily-utilities UI: encrypt-pdf-dialog'

    # 4. Unlock PDF dialog opens & closes cleanly
    Invoke-Menu 'Alat' ("Buka Kunci PDF$ell")
    $s=Wait-State {param($x) $x.securityDialog -eq $true}
    $null=Find 'Buka Kunci PDF Terenkripsi'
    Invoke-Control 'Batal'
    $s=Wait-State {param($x) -not $x.securityDialog}
    Write-Host 'PASS daily-utilities UI: decrypt-pdf-dialog'

    Write-Host "Daily utilities UI evidence: $directory"
} finally {
    if($app -and -not $app.HasExited){
        try { $app.Kill() } catch {}
    }
}
