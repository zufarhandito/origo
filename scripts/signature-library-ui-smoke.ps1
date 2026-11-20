param([string]$ExecutablePath='target/tab-background-check/release/pdf-studio.exe')
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile release -TestName 'signature-library-ui' -IsolatedState -ExecutablePath $ExecutablePath
function Finish-Owned {
 Close-Window
 if(-not $app.WaitForExit(3000)){
  $owned=Get-CimInstance Win32_Process -Filter "ProcessId=$($app.Id)"
  if($owned.ExecutablePath -ne $exe -or -not $owned.CommandLine.Contains($directory)){throw 'Not an owned fixture'}
  Stop-Process -Id $app.Id
 }
}
try{
 $null=Wait-State {param($s) $s.pageCount -gt 0}
 Invoke 'Anotasi';Invoke 'Tanda tangan';Invoke 'Ketik'
 $field=Find 'Nama atau inisial tanda tangan';$field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('Nama Pribadi')
 $field=Find 'Nama template tanda tangan';$field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('Template lokal')
 Invoke 'Simpan template';$null=Find 'Pakai template: Template lokal';Capture 'saved.png'
 $null=Wait-State {param($s) -not $s.dirty -and -not $s.opening}
 Finish-Owned
 $app=Start-Process -FilePath $exe -ArgumentList ('"'+$a+'" --metrics "'+$metrics+'" --state-dir "'+$stateDirectory+'"') -WorkingDirectory (Split-Path $exe) -WindowStyle Normal -PassThru
 $end=(Get-Date).AddSeconds(20);do{Start-Sleep -Milliseconds 200;$app.Refresh()}while($app.MainWindowHandle -eq 0 -and (Get-Date)-lt $end)
 $null=Wait-State {param($s) $s.pageCount -gt 0}
 Invoke 'Anotasi';Invoke 'Tanda tangan';$null=Find 'Pakai template: Template lokal';Capture 'after-restart.png'
 Invoke 'Pakai template: Template lokal';Invoke 'Gunakan tanda tangan';Invoke 'Isi halaman 1'
 $null=Wait-State {param($s) $s.signatureCount -eq 1 -and $s.dirty}
 Invoke 'Tanda tangan';Invoke 'Pakai template: Template lokal';Invoke 'Hapus template'
 $null=Wait-State {param($s) -not $s.opening -and $s.signatureCount -eq 1}
 $entries=@((Get-Content -Raw (Join-Path $stateDirectory 'signature-library.json')|ConvertFrom-Json).entries)
 if($entries.Count -ne 0){throw 'Template delete not persisted'}
 if((Get-FileHash $a).Hash -ne $hashA){throw 'Source changed'}
 Capture 'deleted-template-object-retained.png';Finish-Owned
 Write-Host "PASS local library restart, reuse and independent deletion: $directory"
}catch{Capture 'failure.png';throw}
