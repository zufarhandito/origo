param([ValidateSet('debug','release')][string]$Profile='debug')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$ell=[char]0x2026
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$directory=Join-Path $root ('.research/compress-ui-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $directory | Out-Null
$bigPdf=Join-Path $directory 'scan-besar.pdf'

# Generate a realistic 2-page uncompressed RGB image PDF (~2.8 MB) so Smart Compression shows real shrinkage.
function New-LargeScanPdf([string]$Path) {
    $w=600; $h=800
    $imgBytes=New-Object byte[] ($w * $h * 3)
    for($y=0; $y -lt $h; $y++) {
        $rowOff=$y * $w * 3
        $band=[byte]((($y * 255) / $h) -band 0xff)
        for($x=0; $x -lt $w; $x++) {
            $i=$rowOff + $x * 3
            $imgBytes[$i]=[byte]((($x * 255) / $w) -band 0xff)
            $imgBytes[$i+1]=$band
            $imgBytes[$i+2]=[byte](220 - (($x + $y) % 40))
        }
    }
    $ascii=[System.Text.Encoding]::ASCII
    $ms=New-Object System.IO.MemoryStream
    function Write-Ascii([string]$s){ $b=$ascii.GetBytes($s); $ms.Write($b,0,$b.Length) }
    $offsets=@()
    Write-Ascii "%PDF-1.4`n"
    $offsets+=$ms.Position
    Write-Ascii "1 0 obj`n<< /Type /Catalog /Pages 2 0 R >>`nendobj`n"
    $offsets+=$ms.Position
    Write-Ascii "2 0 obj`n<< /Type /Pages /Kids [5 0 R 8 0 R] /Count 2 >>`nendobj`n"
    for($p=0; $p -lt 2; $p++) {
        $imgId=3 + $p * 3
        $cntId=4 + $p * 3
        $pagId=5 + $p * 3
        $offsets+=$ms.Position
        Write-Ascii "$imgId 0 obj`n<< /Type /XObject /Subtype /Image /Width $w /Height $h /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length $($imgBytes.Length) >>`nstream`n"
        $ms.Write($imgBytes,0,$imgBytes.Length)
        Write-Ascii "`nendstream`nendobj`n"
        $content="q`n595.000 0 0 842.000 0 0 cm`n/Im0 Do`nQ`n"
        $offsets+=$ms.Position
        Write-Ascii "$cntId 0 obj`n<< /Length $($content.Length) >>`nstream`n$content`endstream`nendobj`n"
        $offsets+=$ms.Position
        Write-Ascii "$pagId 0 obj`n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595.000 842.000] /Resources << /XObject << /Im0 $imgId 0 R >> >> /Contents $cntId 0 R >>`nendobj`n"
    }
    $xref=$ms.Position
    $total=$offsets.Count + 1
    Write-Ascii "xref`n0 $total`n0000000000 65535 f `n"
    foreach($off in $offsets) {
        Write-Ascii ("{0:D10} 00000 n `n" -f $off)
    }
    Write-Ascii "trailer`n<< /Size $total /Root 1 0 R >>`nstartxref`n$xref`n%%EOF`n"
    [System.IO.File]::WriteAllBytes($Path,$ms.ToArray())
}

New-LargeScanPdf $bigPdf
$origSize=(Get-Item $bigPdf).Length
if($origSize -lt 2000000){throw "Expected large synthetic scan PDF, got $origSize bytes"}

$qpdf=Join-Path $root 'sidecars/qpdf/bin/qpdf.exe'
if(-not (Test-Path -LiteralPath $qpdf)){throw "Missing qpdf: $qpdf"}
$metrics=Join-Path $directory 'metrics.json'
$stateDir=Join-Path $directory 'state'
$exe=Join-Path $root "target/$Profile/pdf-studio.exe"
if(-not (Test-Path -LiteralPath $exe)){throw "Missing binary: $exe"}

$oldIds=@(Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | ForEach-Object Id)
$desktop=New-Object -ComObject Shell.Application
$desktop.ShellExecute($exe,('"'+$bigPdf+'" --metrics "'+$metrics+'" --state-dir "'+$stateDir+'"'),$root,'open',1)

$deadline=(Get-Date).AddSeconds(20)
do {
    Start-Sleep -Milliseconds 200
    $app=Get-Process -Name pdf-studio -ErrorAction SilentlyContinue | Where-Object Id -NotIn $oldIds | Select-Object -First 1
    if($app){$app.Refresh()}
} while((Get-Date) -lt $deadline -and (-not $app -or $app.MainWindowHandle -eq 0))
if(-not $app -or $app.MainWindowHandle -eq 0){throw 'Visible test window unavailable'}

Add-Type 'using System;using System.Runtime.InteropServices;public static class CompClick{[StructLayout(LayoutKind.Sequential)]public struct Point{public int X,Y;}[DllImport("user32.dll")]public static extern bool ScreenToClient(IntPtr h,ref Point p);[DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);}'

function Wait-State([scriptblock]$Predicate, [int]$Seconds=35) {
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
        $pt=New-Object CompClick+Point
        $pt.X=[int]($r.X+$r.Width/2)
        $pt.Y=[int]($r.Y+$r.Height/2)
        [CompClick]::ScreenToClient($app.MainWindowHandle,[ref]$pt)|Out-Null
        $lp=[IntPtr](($pt.Y -shl 16) -bor ($pt.X -band 0xffff))
        [CompClick]::SendMessage($app.MainWindowHandle,0x201,[IntPtr]1,$lp)|Out-Null
        [CompClick]::SendMessage($app.MainWindowHandle,0x202,[IntPtr]0,$lp)|Out-Null
    }
    Start-Sleep -Milliseconds 250
}

try {
    $s=Wait-State {param($x) -not $x.opening -and -not $x.rendering -and $x.pageCount -eq 2}

    # 1. Open Smart Compress dialog from Alat menu
    Invoke-Control 'Menu aplikasi'
    Invoke-Control 'Alat'
    Invoke-Control ("Kompres PDF (Ukuran)$ell")
    $s=Wait-State {param($x) $x.compressDialog -eq $true}
    $null=Find 'Kompres PDF (Ukuran)'
    Write-Host 'PASS compress UI: dialog-opened'

    # 2. Choose Compact preset + Grayscale and run compression
    Invoke-Control 'Ukuran Minimal (Upload)'
    Invoke-Control 'Skala abu-abu (Grayscale): Nonaktif'
    $null=Find 'Skala abu-abu (Grayscale): Aktif'
    Invoke-Control 'Kompres PDF'
    $s=Wait-State {param($x) -not $x.compressDialog -and -not $x.compressRunning -and $x.compressCompleted -eq 1 -and $x.compressSuccess -eq $true}
    Write-Host "PASS compress UI: compressed $($s.compressBeforeBytes) -> $($s.compressAfterBytes) bytes"

    if($s.compressAfterBytes -ge ($s.compressBeforeBytes / 5)) {
        throw "Expected >80% reduction on large uncompressed scan PDF, got $($s.compressBeforeBytes) -> $($s.compressAfterBytes)"
    }
    $out1=Join-Path $directory 'scan-besar-kompres-minimal-bw.pdf'
    if(-not (Test-Path -LiteralPath $out1)){throw "Missing compressed PDF: $out1"}
    & $qpdf --check $out1 | Out-Null
    if($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne 3){throw "qpdf --check failed with exit $LASTEXITCODE"}
    Write-Host 'PASS compress UI: qpdf-check-valid-and-dramatic-size-reduction'

    # 3. Auto-opened in a second tab with 2 pages
    $s=Wait-State {param($x) -not $x.opening -and -not $x.rendering -and $x.tabCount -eq 2 -and $x.pageCount -eq 2}
    Write-Host 'PASS compress UI: compressed-pdf-auto-opened-in-new-tab'

    # 4. Run Balanced RGB compression on active tab (or source) and verify no-clobber produces a distinct file
    Invoke-Control 'Menu aplikasi'
    Invoke-Control 'Alat'
    Invoke-Control ("Kompres PDF (Ukuran)$ell")
    $s=Wait-State {param($x) $x.compressDialog -eq $true}
    Invoke-Control 'Tinggi (Cetak / Arsip)'
    Invoke-Control 'Buka PDF hasil di Origo: Ya'
    Invoke-Control 'Kompres PDF'
    $s=Wait-State {param($x) -not $x.compressDialog -and -not $x.compressRunning -and $x.compressCompleted -eq 2 -and $x.compressSuccess -eq $true}
    $out2=Join-Path $directory 'scan-besar-kompres-minimal-bw-kompres-tinggi.pdf'
    if(-not (Test-Path -LiteralPath $out2)){throw "Missing second compressed PDF: $out2"}
    if((Get-Item $bigPdf).Length -ne $origSize){throw "Original PDF size changed!"}
    Write-Host 'PASS compress UI: second-preset-and-source-untouched'

    Write-Host "Compress UI evidence: $directory"
} finally {
    if($app -and -not $app.HasExited){
        try { $app.Kill() } catch {}
    }
}
