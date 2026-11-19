param([string]$ExecutablePath='target/tab-background-check/release/pdf-studio.exe')
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile release -TestName 'recent-files-ui' -ExecutablePath $ExecutablePath -IsolatedState
function Wait-Recents([int]$count){
 $end=(Get-Date).AddSeconds(15);do{Start-Sleep -Milliseconds 100;try{$paths=@((Get-Content -Raw (Join-Path $stateDirectory 'recents.json')|ConvertFrom-Json).paths);if($paths.Count -eq $count){return $paths}}catch{}}while((Get-Date)-lt $end);throw 'Recent metadata timeout'
}
try {
 $null=Wait-State {param($s) $s.pageCount -gt 0};$null=Wait-Recents 1
 Invoke 'Bintangi: A.pdf';$null=Wait-State {param($s) $s.pendingPreferences -eq 0}
 Menu-Action 'Berkas' 'Tutup tab  Ctrl+W';$null=Wait-State {param($s) $s.tabCount -eq 0}
 $null=Find 'Buka terakhir: A.pdf';$null=Find 'Favorit: A.pdf'
 $null=Find ('Lokasi: '+(Split-Path $a));$null=Find ('Diubah '+(Get-Item -LiteralPath $a).LastWriteTime.ToString('dd-MM-yyyy HH:mm'));Capture 'recent-light.png'
 Invoke 'Buka terakhir: A.pdf';$null=Wait-State {param($s) $s.tabCount -eq 1 -and $s.pageCount -gt 0};$null=Wait-Recents 1
 Picker 'Buka PDF';Fill-Picker $b;$null=Wait-State {param($s) $s.tabCount -eq 2};$paths=Wait-Recents 2
 if($paths[0] -ne $b){throw 'Latest document not first'}
 Menu-Action 'Berkas' 'Tutup tab  Ctrl+W';$null=Wait-State {param($s) $s.tabCount -eq 1}
 Menu-Action 'Berkas' 'Tutup tab  Ctrl+W';$null=Wait-State {param($s) $s.tabCount -eq 0}
 $null=Find 'Buka terakhir: B.pdf';Invoke 'Buka terakhir: A.pdf';$null=Wait-State {param($s) $s.tabCount -eq 1 -and $s.pageCount -gt 0};Start-Sleep -Milliseconds 500
 $paths=Wait-Recents 2;if($paths[0] -ne $a){throw 'Reopened document not promoted'}
 Menu-Action 'Berkas' 'Tutup tab  Ctrl+W';$null=Wait-State {param($s) $s.tabCount -eq 0}
 Menu-Action 'Tampilan' 'Gelap';Capture 'recent-dark.png'
 Close-Window;if(-not $app.WaitForExit(10000)){throw 'Close before restart failed'}
 $app=Start-Process -FilePath $exe -ArgumentList ('--metrics "'+$metrics+'" --state-dir "'+$stateDirectory+'"') -WorkingDirectory (Split-Path $exe) -WindowStyle Normal -PassThru
 $end=(Get-Date).AddSeconds(20);do{Start-Sleep -Milliseconds 200;$app.Refresh()}while($app.MainWindowHandle -eq 0 -and (Get-Date)-lt $end)
 $null=Find 'Buka terakhir: A.pdf';$null=Find 'Buka terakhir: B.pdf';$null=Find 'Favorit: A.pdf'
 $null=Find ('Lokasi: '+(Split-Path $a));$null=Find ('Diubah '+(Get-Item -LiteralPath $a).LastWriteTime.ToString('dd-MM-yyyy HH:mm'))
 Invoke 'Buka terakhir: A.pdf';$null=Wait-State {param($s) $s.pageCount -gt 0}
 Invoke 'Hapus bintang: A.pdf';$null=Wait-State {param($s) $s.pendingPreferences -eq 0}
 Menu-Action 'Berkas' 'Tutup tab  Ctrl+W';$null=Wait-State {param($s) $s.tabCount -eq 0}
 $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
 if($ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,'Favorit: A.pdf'))){throw 'Stale favorite marker'}
 Invoke 'Hapus riwayat';$null=Wait-Recents 0;Capture 'cleared.png'
 if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $b).Hash -ne $hashB){throw 'Source changed'}
 Close-Window;if(-not $app.WaitForExit(10000)){throw 'Close failed'}
 @{status='passed';cases=@('persist-successful-open','recent-reopen','no-duplicates','newest-first','promote-reopened','light-dark','reload-after-restart','clear-persistent','favorite-marker','folder-location','filesystem-modified-local','metadata-after-restart','unstar-sync');sourceUnchanged=$true}|ConvertTo-Json|Set-Content (Join-Path $directory 'result.json')
 Write-Host "PASS recent files: $directory"
}catch{Capture 'failure.png';throw}
