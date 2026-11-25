param([ValidateSet('debug','release')][string]$Profile='debug',[string]$ExecutablePath)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'images-to-pdf-ui' -ExecutablePath $ExecutablePath -StartEmpty
$ell=[char]0x2026
$dot=[char]0x00B7
$cases=New-Object Collections.Generic.List[object]
function Pass([string]$Name) {
    Capture ($Name+'.png')
    $cases.Add([pscustomobject]@{Case=$Name;Status='passed'})
    $cases.ToArray()|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'cases.json')
    Write-Host "PASS images-to-pdf UI: $Name"
}
function New-TestImage([string]$Path,[int]$Width,[int]$Height,[Drawing.Color]$Color,[Drawing.Imaging.ImageFormat]$Format) {
    $bmp=New-Object Drawing.Bitmap($Width,$Height)
    try {
        $g=[Drawing.Graphics]::FromImage($bmp)
        try { $g.Clear($Color) } finally { $g.Dispose() }
        $bmp.Save($Path,$Format)
    } finally { $bmp.Dispose() }
}
function Accept-OwnedOverwritePrompt {
    if(-not ('ImgPdfNativePrompt' -as [type])){
        Add-Type 'using System;using System.Text;using System.Runtime.InteropServices;public static class ImgPdfNativePrompt{public delegate bool EnumProc(IntPtr h,IntPtr l);[DllImport("user32.dll")]public static extern bool EnumWindows(EnumProc p,IntPtr l);[DllImport("user32.dll")]public static extern bool EnumChildWindows(IntPtr h,EnumProc p,IntPtr l);[DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);[DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetWindowText(IntPtr h,StringBuilder s,int n);[DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,StringBuilder s,int n);}'
    }
    $deadline=(Get-Date).AddSeconds(10)
    do {
        $script:overwriteYes=[IntPtr]::Zero
        [ImgPdfNativePrompt]::EnumWindows({param($h,$l)
            $owner=0;[ImgPdfNativePrompt]::GetWindowThreadProcessId($h,[ref]$owner)|Out-Null
            if($owner -eq $app.Id){[ImgPdfNativePrompt]::EnumChildWindows($h,{param($button,$unused)
                $label=New-Object Text.StringBuilder 256;$class=New-Object Text.StringBuilder 128
                [ImgPdfNativePrompt]::GetWindowText($button,$label,256)|Out-Null;[ImgPdfNativePrompt]::GetClassName($button,$class,128)|Out-Null
                if($class.ToString() -eq 'Button' -and $label.ToString().Trim('&',' ') -in @('Yes','Ya')){$script:overwriteYes=$button}
                return $true
            },[IntPtr]::Zero)|Out-Null};return $true
        },[IntPtr]::Zero)|Out-Null
        if($script:overwriteYes -ne [IntPtr]::Zero){[OutputDialog]::PostMessage($script:overwriteYes,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null;return}
        $s=Read-Metrics
        if(-not $s.imagesPdfRunning){return}
        Start-Sleep -Milliseconds 100
    }while((Get-Date) -lt $deadline)
    throw 'Owned overwrite confirmation was not handled'
}
try {
    $null=Wait-State {param($s) $s.tabCount -eq 0}
    $img1=Join-Path $directory '01-portrait.png'
    $img2=Join-Path $directory '02-landscape.jpg'
    New-TestImage $img1 300 500 ([Drawing.Color]::FromArgb(255,30,80,220)) ([Drawing.Imaging.ImageFormat]::Png)
    New-TestImage $img2 500 300 ([Drawing.Color]::FromArgb(255,220,40,40)) ([Drawing.Imaging.ImageFormat]::Jpeg)
    $hash1=(Get-FileHash $img1).Hash
    $hash2=(Get-FileHash $img2).Hash

    # 1. Open from empty welcome screen & cancel
    Invoke 'Gambar ke PDF'
    $null=Wait-State {param($s) $s.imagesPdfDialog}
    if((Find ("Simpan PDF$ell")).Current.IsEnabled){throw 'Empty image list should disable Simpan PDF'}
    Pass 'empty-welcome-open-and-disabled-submit'
    Invoke 'Batal'
    $null=Wait-State {param($s) -not $s.imagesPdfDialog}

    # 2. Open from Alat menu with 0 tabs, add 2 images, reorder, convert & auto-open
    Invoke 'Menu aplikasi';Invoke 'Alat';Invoke ("Gambar ke PDF$ell")
    $null=Wait-State {param($s) $s.imagesPdfDialog}
    Click-Control ("Tambah gambar$ell");Fill-Picker $img1
    $null=Find '1. 01-portrait.png'
    Click-Control ("Tambah gambar$ell");Fill-Picker $img2
    $null=Find '2. 02-landscape.jpg'
    Invoke 'Naikkan gambar 2'
    $null=Find '1. 02-landscape.jpg'
    $null=Find '2. 01-portrait.png'
    $null=Find ("2 gambar $dot A4 $dot Otomatis $dot Margin Kecil")
    Pass 'add-reorder-and-preview'

    $outPdf=Join-Path $directory 'hasil-gabungan.pdf'
    $before=(Read-Metrics).imagesPdfCompleted
    Click-Control ("Simpan PDF$ell");Fill-Picker $outPdf
    $done=Wait-State {param($s) -not $s.imagesPdfRunning -and $s.imagesPdfCompleted -gt $before -and $s.tabCount -eq 1 -and $s.pageCount -eq 2}
    if(-not $done.imagesPdfSuccess -or $done.imagesPdfPages -ne 2){throw 'Images-to-PDF conversion failed'}
    if(-not (Test-Path -LiteralPath $outPdf)){throw 'Output PDF missing'}
    Pass 'convert-and-auto-open-in-tab'

    # 3. Verify existing destination is rejected without overwriting (no-clobber)
    $outHash=(Get-FileHash $outPdf).Hash
    Invoke 'Menu aplikasi';Invoke 'Alat';Invoke ("Gambar ke PDF$ell")
    $null=Wait-State {param($s) $s.imagesPdfDialog}
    Click-Control ("Tambah gambar$ell");Fill-Picker $img1
    $null=Find '1. 01-portrait.png'
    Invoke 'Sesuai gambar'
    if((Find 'Otomatis').Current.IsEnabled){throw 'Orientation should be disabled for FitImage'}
    Invoke 'Buka PDF setelah selesai'
    $before2=(Read-Metrics).imagesPdfCompleted
    Click-Control ("Simpan PDF$ell");Fill-Picker $outPdf
    Accept-OwnedOverwritePrompt
    $rejected=Wait-State {param($s) -not $s.imagesPdfRunning -and $s.imagesPdfCompleted -gt $before2}
    if($rejected.imagesPdfSuccess){throw 'Existing PDF destination should be rejected'}
    if((Get-FileHash $outPdf).Hash -ne $outHash){throw 'Existing PDF was overwritten'}
    if($rejected.tabCount -ne 1){throw 'Rejected conversion should not open extra tab'}
    Pass 'fit-image-and-no-clobber-existing-pdf'

    # 4. Source images untouched and no staging leaked
    if((Get-FileHash $img1).Hash -ne $hash1 -or (Get-FileHash $img2).Hash -ne $hash2){throw 'Source images modified'}
    if(Get-ChildItem -LiteralPath $directory -Directory -Filter '.pdf-studio-*'){throw 'Staging directory leaked'}
    Pass 'sources-unchanged-and-clean-staging'
    Write-Host "Images-to-PDF UI evidence: $directory"
} catch {
    if($app -and -not $app.HasExited){Capture 'failure.png'}
    throw
} finally {
    for($attempt=0;$attempt -lt 8 -and $app -and -not $app.HasExited;$attempt++){
        try{
            Close-Window;Start-Sleep -Milliseconds 300
            $null=$app.WaitForExit(1500)
        }catch [Windows.Automation.ElementNotAvailableException] {Start-Sleep -Milliseconds 100}
    }
    if($app -and -not $app.HasExited){$app.Kill()}
}
