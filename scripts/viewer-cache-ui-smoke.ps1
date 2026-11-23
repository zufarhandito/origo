param([string]$ExecutablePath='target/release/pdf-studio.exe',[switch]$Baseline)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -ExecutablePath $ExecutablePath -TestName 'viewer-cache' -Fixture '.research/book-benchmark/vector-1000.pdf' -StartEmpty

function Sample([string]$Phase) {
    $s=Wait-State {param($s) -not $s.textLoading}
    $app.Refresh()
    [pscustomobject]@{
        Phase=$Phase; PageBytes=$s.totalPageCacheBytes; ThumbnailBytes=$s.totalThumbnailCacheBytes
        WorkingSetMiB=[Math]::Round($app.WorkingSet64/1MB,1); PrivateMiB=[Math]::Round($app.PrivateMemorySize64/1MB,1)
        NativeDocuments=$s.nativeActiveDocuments; GpuErrors=$s.windowGpuReleaseErrors
    }
}

try {
    $null=Wait-State {param($s) $s.tabCount -eq 0}
    $samples=@(Sample 'startup-empty')
    Picker 'Buka PDF'; Fill-Picker $a
    $null=Wait-State {param($s) $s.pageCount -eq 1000 -and $s.totalThumbnailCacheBytes -gt 0}
    $samples+=Sample 'opened'
    Invoke 'Daftar isi'
    $samples+=Sample 'outline'
    Invoke 'Thumbnail'
    $null=Wait-State {param($s) $s.totalThumbnailCacheBytes -gt 0}
    Invoke 'Panel navigasi'
    $samples+=Sample 'sidebar-hidden'
    foreach($zoom in @(150,200,400,100)) {
        Invoke 'Zoom'; Invoke "$zoom%"
        $null=Wait-State {param($s) $s.zoomPercent -eq $zoom -and $s.totalPageCacheBytes -gt 0}
        $samples+=Sample "zoom-$zoom"
    }
    Invoke 'Panel navigasi'
    $null=Wait-State {param($s) $s.totalThumbnailCacheBytes -gt 0}
    $samples+=Sample 'restored'
    Menu-Action 'Berkas' 'Tutup tab  Ctrl+W'
    $null=Wait-State {param($s) $s.tabCount -eq 0 -and $s.nativeActiveDocuments -eq 0}
    $samples+=Sample 'closed-idle'
    $samples|Export-Csv -NoTypeInformation (Join-Path $directory 'samples.csv')
    $samples|Format-Table -AutoSize|Out-String|Write-Host
    if(!$Baseline){
        $hidden=$samples|Where-Object Phase -in @('outline','sidebar-hidden')
        if($hidden|Where-Object ThumbnailBytes -gt 0){throw 'Hidden thumbnails still occupy the cache'}
        $large=$samples|Where-Object Phase -eq 'zoom-400'
        $normal=$samples|Where-Object Phase -eq 'zoom-100'
        if($normal.PageBytes -ge $large.PageBytes){throw 'High-resolution rasters survived returning to 100%'}
    }
    if($samples[-1].PageBytes -or $samples[-1].ThumbnailBytes -or ($samples|Where-Object GpuErrors)){throw 'A closed tab retained viewer resources'}
    if((Get-FileHash -LiteralPath $a).Hash -ne $hashA){throw 'Synthetic source changed'}
    Write-Host "PASS viewer cache: $directory"
} finally {
    if(!$app.HasExited){Close-Window;$null=$app.WaitForExit(5000)}
}
