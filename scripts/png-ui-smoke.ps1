param([ValidateSet('debug','release')][string]$Profile='release',[string]$ExecutablePath)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'png-ui' -ExecutablePath $ExecutablePath
$cases=New-Object Collections.Generic.List[object]
function Pass([string]$Name) {
    $cases.Add([pscustomobject]@{Case=$Name;Status='passed'})
    $cases.ToArray()|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'cases.json')
    Write-Host "PASS PNG UI: $Name"
}
function Configure-Png([string]$Folder,[switch]$Selected,[switch]$High) {
    Invoke 'Alat';Invoke 'Ekspor PNG…';$null=Find 'Ekspor PNG'
    if($Selected){Invoke 'Halaman terpilih'}
    if($High){Invoke '300 DPI'}
    $field=Find 'Nama folder PNG';$field.SetFocus()
    $field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue($Folder)
    Start-Sleep -Milliseconds 200
}
function Run-Png([string]$Folder,[switch]$Selected,[switch]$High) {
    $before=(Read-Metrics).exportsCompleted
    Configure-Png $Folder -Selected:$Selected -High:$High
    Click-Control 'Pilih folder induk';Fill-Picker $directory
    Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before}
}
function Check-Png([string]$Folder,[int]$Count,[int]$Width,[int]$Height) {
    $files=@(Get-ChildItem -LiteralPath (Join-Path $directory $Folder) -File)
    if($files.Count -ne $Count){throw 'PNG file count mismatch'}
    foreach($file in $files){$bitmap=[Drawing.Bitmap]::FromFile($file.FullName);try{if($bitmap.Width -ne $Width -or $bitmap.Height -ne $Height){throw "PNG dimensions mismatch: $($bitmap.Width)x$($bitmap.Height)"}}finally{$bitmap.Dispose()}}
}
try {
    $original=Wait-State {param($s) $s.pageCount -eq 4}
    Configure-Png 'unused'
    if((Find 'Halaman terpilih').Current.IsEnabled){throw 'Empty page selection incorrectly enabled'}
    Capture 'configuration.png'
    Invoke 'Batal'
    if((Read-Metrics).revision -ne $original.revision){throw 'Configuration cancel changed revision'}
    Pass 'configuration-defaults-empty-selection-and-cancel'
    Configure-Png 'picker-cancel'
    $before=(Read-Metrics).exportsCompleted
    Click-Control 'Pilih folder induk';Fill-Picker '' -Cancel
    $null=Wait-State {param($s) -not $s.exporting -and $s.exportsCompleted -gt $before -and -not $s.exportSuccess}
    if(Test-Path (Join-Path $directory 'picker-cancel')){throw 'Cancelled picker produced output'}
    Pass 'native-folder-picker-cancel'
    Invoke 'Panel alat';Invoke 'Halaman 1';Invoke 'Putar kanan'
    $dirty=Wait-State {param($s) $s.dirty -and $s.selectedPages -eq 1}
    $state=Run-Png 'selected-150' -Selected
    if(-not $state.exportSuccess -or -not $state.dirty -or $state.revision -ne $dirty.revision -or $state.exportPages -ne 1){throw 'PNG export changed PDF dirty baseline or selection scope'}
    Check-Png 'selected-150' 1 875 625
    Pass 'selected-150-rotation-and-dirty-baseline'
    $state=Run-Png 'selected-300' -Selected -High
    if(-not $state.exportSuccess -or $state.exportPages -ne 1){throw '300 DPI export failed'}
    Check-Png 'selected-300' 1 1750 1250
    Pass 'selected-300-exact-dimensions'
    $state=Run-Png 'all-150'
    if(-not $state.exportSuccess -or $state.exportPages -ne 4){throw 'All-page PNG export failed'}
    if(@(Get-ChildItem (Join-Path $directory 'all-150') -Filter '*.png').Count -ne 4){throw 'All-page file count mismatch'}
    Pass 'all-pages-export'
    $protected=Join-Path $directory 'selected-150/page-0001.png';$hash=(Get-FileHash $protected).Hash
    $state=Run-Png 'selected-150' -Selected
    if($state.exportSuccess){throw 'Existing folder accepted'}
    $null=Find 'PNG belum dapat diekspor';Invoke 'Tutup'
    if((Get-FileHash $protected).Hash -ne $hash){throw 'Existing PNG changed'}
    Pass 'existing-folder-rejected-with-unchanged-image'
    # The active selection/snapshot belongs to A even if another tab is opened later.
    Picker 'Buka PDF';Fill-Picker $b
    $other=Wait-State {param($s) $s.tabCount -eq 2 -and -not $s.dirty}
    Invoke 'Daftar tab';Invoke $a
    $null=Wait-State {param($s) $s.tabId -eq $dirty.tabId -and $s.dirty}
    $state=Run-Png 'origin-tab' -Selected
    Invoke 'Daftar tab';Invoke $b
    $null=Wait-State {param($s) $s.tabId -eq $other.tabId -and -not $s.dirty -and $s.exportsCompleted -eq $other.exportsCompleted}
    Invoke 'Daftar tab';Invoke $a
    $null=Wait-State {param($s) $s.tabId -eq $dirty.tabId -and $s.dirty}
    Pass 'separate-tab-export-origin-and-clean-state'
    $c=Join-Path $directory 'C.pdf'
    Copy-Item -LiteralPath (Join-Path $root 'tests/fixtures/hundred-pages.pdf') -Destination $c
    $hashC=(Get-FileHash $c).Hash
    Picker 'Buka PDF';Fill-Picker $c
    $null=Wait-State {param($s) $s.tabCount -eq 3 -and $s.pageCount -eq 100}
    Invoke 'Panel alat';Invoke 'Halaman 1';Invoke 'Putar kanan'
    $origin=Wait-State {param($s) $s.dirty}
    Configure-Png 'cancelled-batch' -High
    Click-Control 'Pilih folder induk';Fill-Picker $directory
    $null=Wait-State {param($s) $s.exporting -and $s.pngDone -ge 1}
    Switch-Tab $b
    $null=Wait-State {param($s) $s.tabId -eq $other.tabId -and $s.windowExportOwner -eq $origin.tabId}
    Invoke 'Batal ekspor'
    $null=Wait-State {param($s) $s.windowExportOwner -eq 0}
    Switch-Tab $c
    $null=Wait-State {param($s) $s.tabId -eq $origin.tabId -and -not $s.exporting -and -not $s.exportSuccess -and $s.dirty}
    if(Test-Path (Join-Path $directory 'cancelled-batch')){throw 'Cancelled batch published partial output'}
    Pass 'cancel-running-png-from-other-tab-and-remove-staging'
    Configure-Png 'waiting-close' -High
    Click-Control 'Pilih folder induk';Fill-Picker $directory
    $null=Wait-State {param($s) $s.exporting -and $s.pngDone -ge 1}
    Invoke 'Berkas';Invoke 'Tutup tab  Ctrl+W'
    $null=Wait-State {param($s) $s.transition -eq 'WaitingExport'}
    Capture 'waiting-close.png'
    $null=Wait-State {param($s) $s.transition -eq 'Prompt' -and -not $s.exporting -and $s.exportSuccess -and $s.dirty}
    $null=Find 'Simpan perubahan sebelum melanjutkan?';Invoke 'Batal'
    $null=Wait-State {param($s) $s.transition -eq 'Idle' -and $s.tabId -eq $origin.tabId -and $s.revision -eq $origin.revision -and $s.dirty}
    if(@(Get-ChildItem (Join-Path $directory 'waiting-close') -Filter '*.png').Count -ne 100){throw 'Waiting for close interrupted PNG batch'}
    if((Get-FileHash $c).Hash -ne $hashC){throw 'PNG batch changed C source'}
    Pass 'close-waits-for-png-then-prompts-for-unsaved-edits'
    Close-Window
    $null=Find 'Simpan perubahan sebelum melanjutkan?'
    Invoke 'Batal'
    $null=Wait-State {param($s) $s.transition -eq 'Idle' -and $s.dirty}
    Pass 'successful-png-cannot-authorize-dirty-close'
    if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $b).Hash -ne $hashB){throw 'PNG UI modified sources'}
    if(Get-ChildItem -LiteralPath $directory -Directory -Filter '.pdf-studio-*'){throw 'PNG UI leaked staging'}
    Capture 'completed.png'
    Pass 'source-hashes-and-staging-cleanup'
    Write-Host "PASS PNG UI suite: $directory"
} catch {if($app -and -not $app.HasExited){Capture 'failure.png'};throw}
finally {
    for($attempt=0;$attempt -lt 8 -and $app -and -not $app.HasExited;$attempt++){
        try{
            $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
            $errorTitle=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'PNG belum dapat diekspor'))
            if($errorTitle){Invoke 'Tutup'}
            Close-Window;Start-Sleep -Milliseconds 300
            if(-not $app.HasExited){
                $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
                $discard=$ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Buang perubahan'))
                if($discard){Invoke 'Buang perubahan'}
                $null=$app.WaitForExit(1500)
            }
        }catch [Windows.Automation.ElementNotAvailableException] {Start-Sleep -Milliseconds 100}
    }
    if($app -and -not $app.HasExited){throw 'Owned PNG test window did not close after cleanup'}
}
