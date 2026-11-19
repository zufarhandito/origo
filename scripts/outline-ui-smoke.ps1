param([ValidateSet('debug','release')][string]$Profile='release',[string]$ExecutablePath)
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'outline-ui' -Fixture '.research/outline-print-fixtures/outline.pdf' -ExecutablePath $ExecutablePath -IsolatedState
$cases=New-Object Collections.Generic.List[object]
function Pass([string]$Name){Capture ($Name+'.png');$cases.Add([pscustomobject]@{Case=$Name;Status='passed'});$cases.ToArray()|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'cases.json');Write-Host "PASS outline: $Name"}
try {
 $null=Wait-State {param($s) $s.pageCount -eq 3}
 Invoke 'Daftar isi';$null=Find 'Bab 1';$null=Find 'Kelompok'
 $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
 foreach($name in @('Subbab bernama','Tautan eksternal')){
  if($ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$name))){throw 'Child visible before expanding parent'}
 }
 $null=Find 'Buka bab: Bab 1';Pass 'initial-root-only-collapsed'
 Invoke 'Buka bab: Bab 1';$null=Find 'Subbab bernama'
 $null=Wait-State {param($s) $s.outlineOpen -and -not $s.dirty}
 if((Find 'Tautan eksternal').Current.IsEnabled){throw 'External action must be disabled'}
 Pass 'nested-tree-and-disabled-external-action'
 Invoke 'Tutup bab: Bab 1';$null=Find 'Buka bab: Bab 1';Invoke 'Buka bab: Bab 1';$null=Find 'Subbab bernama'
 Pass 'expand-collapse-parent'
 Invoke 'Subbab bernama';$null=Wait-State {param($s) $s.activePage -eq 2 -and -not $s.dirty}
 Pass 'named-destination-preserves-clean-state'
 Invoke 'Bab 3 terotasi';$null=Wait-State {param($s) $s.activePage -eq 3 -and -not $s.dirty}
 Pass 'rotated-cropped-destination'
 Invoke 'Thumbnail';$null=Wait-State {param($s) -not $s.outlineOpen};Invoke 'Daftar isi';$null=Find 'Bab 1'
 Pass 'sidebar-mode-switch'
 Invoke 'Panel alat';Invoke 'Halaman 3';Invoke 'Hapus';$null=Wait-State {param($s) $s.pageCount -eq 2 -and $s.dirty}
 Invoke 'Panel alat';$null=Find 'Bab 1'
 if((Find 'Bab 3 terotasi').Current.IsEnabled){throw 'Deleted page destination remained enabled'}
 Invoke 'Undo';$null=Wait-State {param($s) $s.pageCount -eq 3 -and -not $s.dirty}
 if(-not (Find 'Bab 3 terotasi').Current.IsEnabled){throw 'Undo did not restore destination'}
 Pass 'deleted-target-and-undo'
 Menu-Action 'Tampilan' 'Tampilan kedua PDF ini'
 $null=Wait-State {param($s) $s.splitView}
 Invoke 'Subbab bernama';$null=Wait-State {param($s) $s.activePage -eq 2}
 Pass 'outline-in-split-view'
 Menu-Action 'Tampilan' 'Keluar tampilan berdampingan'
 Picker 'Buka PDF';Fill-Picker (Join-Path $root '.research/outline-print-fixtures/no-outline.pdf')
 $null=Wait-State {param($s) $s.tabCount -eq 2 -and $s.pageCount -eq 1}
 Invoke 'Daftar isi';$null=Wait-State {param($s) $s.outlineOpen -and -not $s.dirty};Start-Sleep -Milliseconds 500
 Pass 'empty-outline-state'
 Picker 'Buka PDF';Fill-Picker (Join-Path $root '.research/outline-print-fixtures/long-outline.pdf')
 $null=Wait-State {param($s) $s.tabCount -eq 3 -and $s.pageCount -eq 3}
 Invoke 'Daftar isi'
 $title='Bab III identifikasi kasus dan pemetaan pengetahuan dalam sistem informasi spasial serta pembahasan hasil penelitian'
 $null=Find $title;Start-Sleep -Milliseconds 300
 $wrapped=(Find $title).Current.BoundingRectangle;$following=(Find 'Kelompok').Current.BoundingRectangle
 if($wrapped.Height -lt 60){throw 'Long title is not wrapping into multiple lines'}
 if($following.Y -lt $wrapped.Bottom-1){throw 'Variable-height outline rows overlap'}
 Pass 'wrapped-long-title-no-overlap'
 Invoke $title;$null=Wait-State {param($s) $s.activePage -eq 3 -and -not $s.dirty}
 if((Get-FileHash $a).Hash -ne $hashA){throw 'Source changed'}
 Close-Window
 Write-Host "Outline evidence: $directory"
}catch{Capture 'failure.png';throw}
