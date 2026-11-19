param([string]$ExecutablePath='target/tab-background-check/release/pdf-studio.exe')
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile release -TestName 'empty-state-ui' -ExecutablePath $ExecutablePath -IsolatedState
function Assert-Empty {
 $null=Wait-State {param($s) $s.tabCount -eq 0}
 $null=Find 'Buka PDF'
 $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
 foreach($name in @('Daftar tab','Buka PDF di tab baru')){
  if($ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$name))){throw "Empty window still exposes $name"}
 }
}
try {
 $null=Wait-State {param($s) $s.pageCount -gt 0};$null=Find 'Daftar tab'
 Menu-Action 'Berkas' 'Tutup tab  Ctrl+W';Assert-Empty
 Menu-Action 'Tampilan' 'Terang';Capture 'empty-light.png'
 Picker 'Buka PDF';Fill-Picker $a -Cancel;Assert-Empty
 Picker 'Buka PDF';Fill-Picker $a
 $null=Wait-State {param($s) $s.tabCount -eq 1 -and $s.pageCount -gt 0};$null=Find 'Daftar tab';$null=Find 'Buka PDF di tab baru';Capture 'reopened.png'
 Menu-Action 'Berkas' 'Tutup tab  Ctrl+W';Assert-Empty
 Menu-Action 'Tampilan' 'Gelap';Capture 'empty-dark.png'
 if((Get-FileHash $a).Hash -ne $hashA){throw 'Source changed'}
 Close-Window;if(-not $app.WaitForExit(10000)){throw 'Test window did not close'}
 @{status='passed';cases=@('last-tab-hides-strip','empty-light','cancel-open-stays-empty','open-from-empty-restores-tabs','close-last-again','empty-dark');sourceUnchanged=$true}|ConvertTo-Json|Set-Content (Join-Path $directory 'result.json')
 Write-Host "PASS empty state: $directory"
}catch {Capture 'failure.png';throw}
