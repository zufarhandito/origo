param([ValidateSet('debug','release')][string]$Profile='debug')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$ell=[char]0x2026
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$directory=Join-Path $root ('.research/encrypted-viewer-ui-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $directory | Out-Null
$plain=Join-Path $directory 'plain.pdf'
$locked=Join-Path $directory 'rahasia.pdf'
Copy-Item (Join-Path $root 'tests/fixtures/text-selection.pdf') $plain

$qpdf=Join-Path $root 'sidecars/qpdf/bin/qpdf.exe'
if(-not (Test-Path -LiteralPath $qpdf)){throw "Missing qpdf: $qpdf"}
& $qpdf --encrypt origo2026 owner2026 256 -- $plain $locked
if($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $locked)){throw "Failed to encrypt test fixture"}

$metrics=Join-Path $directory 'metrics.json'
$stateDir=Join-Path $directory 'state'
$exe=Join-Path $root "target/$Profile/pdf-studio.exe"
if(-not (Test-Path -LiteralPath $exe)){throw "Missing binary: $exe"}

$oldIds=@(Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | ForEach-Object Id)
$desktop=New-Object -ComObject Shell.Application
$desktop.ShellExecute($exe,('"'+$locked+'" --metrics "'+$metrics+'" --state-dir "'+$stateDir+'"'),$root,'open',1)

$deadline=(Get-Date).AddSeconds(20)
do {
    Start-Sleep -Milliseconds 200
    $app=Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | Where-Object Id -NotIn $oldIds | Select-Object -First 1
    if($app){$app.Refresh()}
} while((Get-Date) -lt $deadline -and (-not $app -or $app.MainWindowHandle -eq 0))
if(-not $app -or $app.MainWindowHandle -eq 0){throw 'Visible test window unavailable'}

Add-Type 'using System;using System.Runtime.InteropServices;public static class EncClick{[StructLayout(LayoutKind.Sequential)]public struct Point{public int X,Y;}[DllImport("user32.dll")]public static extern bool ScreenToClient(IntPtr h,ref Point p);[DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);[DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);}'

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
        $pt=New-Object EncClick+Point
        $pt.X=[int]($r.X+$r.Width/2)
        $pt.Y=[int]($r.Y+$r.Height/2)
        [EncClick]::ScreenToClient($app.MainWindowHandle,[ref]$pt)|Out-Null
        $lp=[IntPtr](($pt.Y -shl 16) -bor ($pt.X -band 0xffff))
        [EncClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
        [EncClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$lp)|Out-Null
    }
    Start-Sleep -Milliseconds 250
}

function Set-Input([string]$Name, [string]$Value) {
    $field=Find $Name
    $val=$null
    if($field.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern,[ref]$val)) {
        $field.SetFocus()
        $val.SetValue($Value)
        Start-Sleep -Milliseconds 150
        return
    }
    $edit=$field.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::Edit))
    if($edit -and $edit.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern,[ref]$val)) {
        $edit.SetFocus()
        $val.SetValue($Value)
        Start-Sleep -Milliseconds 150
        return
    }
    $r=$field.Current.BoundingRectangle
    $pt=New-Object EncClick+Point
    $pt.X=[int]($r.X+40)
    $pt.Y=[int]($r.Y+$r.Height/2)
    [EncClick]::ScreenToClient($app.MainWindowHandle,[ref]$pt)|Out-Null
    $lp=[IntPtr](($pt.Y -shl 16) -bor ($pt.X -band 0xffff))
    [EncClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
    [EncClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$lp)|Out-Null
    Start-Sleep -Milliseconds 120
    # Clear existing characters via Backspace + Delete
    for($i=0; $i -lt 32; $i++) {
        [EncClick]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]0x08,[IntPtr]1)|Out-Null
        [EncClick]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]0x08,[IntPtr]1)|Out-Null
        [EncClick]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]0x2E,[IntPtr]1)|Out-Null
        [EncClick]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]0x2E,[IntPtr]1)|Out-Null
    }
    Start-Sleep -Milliseconds 120
    foreach($ch in $Value.ToCharArray()) {
        [EncClick]::PostMessage($app.MainWindowHandle,0x102,[IntPtr][int]$ch,[IntPtr]1)|Out-Null
        Start-Sleep -Milliseconds 15
    }
    Start-Sleep -Milliseconds 150
}

try {
    # 1. Opening encrypted PDF automatically shows Password Prompt modal
    $s=Wait-State {param($x) $x.passwordPromptDialog -eq $true -and $x.tabCount -eq 0}
    $null=Find 'PDF Dilindungi Password'
    Write-Host 'PASS encrypted-viewer UI: password-prompt-opened'

    # 2. Wrong password stays in modal and shows inline error
    Set-Input 'Password buka PDF' 'password-salah'
    Invoke-Control 'Buka Dokumen'
    $s=Wait-State {param($x) $x.passwordPromptDialog -eq $true -and $x.passwordPromptError -eq $true}
    $null=Find 'Password salah. Periksa kembali lalu coba lagi.'
    Write-Host 'PASS encrypted-viewer UI: wrong-password-rejected-inline'

    # 3. Correct password unlocks and opens all 4 pages directly in the viewer
    Set-Input 'Password buka PDF' 'origo2026'
    Invoke-Control 'Buka Dokumen'
    $s=Wait-State {param($x) -not $x.passwordPromptDialog -and -not $x.opening -and -not $x.rendering -and $x.tabCount -eq 1 -and $x.pageCount -eq 4 -and $x.fullRenders -ge 1}
    Write-Host 'PASS encrypted-viewer UI: unlocked-and-rendered-4-pages'

    # 4. Opening "Buka Kunci PDF..." pre-fills the currently open encrypted PDF
    Invoke-Control 'Menu aplikasi'
    Invoke-Control 'Alat'
    Invoke-Control ("Buka Kunci PDF$ell")
    $s=Wait-State {param($x) $x.securityDialog -eq $true}
    $null=Find 'Buka Kunci PDF Terenkripsi'
    Invoke-Control 'Batal'
    $s=Wait-State {param($x) -not $x.securityDialog}
    Write-Host 'PASS encrypted-viewer UI: decrypt-dialog-available-from-encrypted-tab'

    Write-Host "Encrypted viewer UI evidence: $directory"
} finally {
    if($app -and -not $app.HasExited){
        try { $app.Kill() } catch {}
    }
}
