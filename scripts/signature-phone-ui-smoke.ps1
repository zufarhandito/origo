param([string]$ExecutablePath='target/tab-background-check/release/pdf-studio.exe')
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile release -TestName 'signature-phone-ui' -IsolatedState -ExecutablePath $ExecutablePath
function Phone-Url {
 $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
 $end=(Get-Date).AddSeconds(8)
 do {
  $node=$ui.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)|Where-Object {$_.Current.Name.StartsWith('Alamat gambar HP: ')}|Select-Object -First 1
  if($node){return $node.Current.Name.Substring('Alamat gambar HP: '.Length)}
  Start-Sleep -Milliseconds 100
 }while((Get-Date)-lt $end)
 throw 'Phone pairing unavailable'
}
function Request-Phone([string]$url,[switch]$Submit){
 $uri=[Uri]$url;$request=[Net.HttpWebRequest]::Create($uri);$request.Proxy=$null;$request.Timeout=2000;$request.KeepAlive=$false;$request.ServicePoint.Expect100Continue=$false
 if($Submit){
  $request.Method='POST';$request.ContentType='application/json';$request.Headers.Add('Origin',$uri.GetLeftPart([UriPartial]::Authority))
  $bytes=[Text.Encoding]::UTF8.GetBytes('[[[0.2,0.3],[0.5,0.7],[0.8,0.2]]]');$request.ContentLength=$bytes.Length
  $stream=$request.GetRequestStream();$stream.Write($bytes,0,$bytes.Length);$stream.Dispose()
 }
 $response=$request.GetResponse();try{$reader=[IO.StreamReader]::new($response.GetResponseStream());try{return $reader.ReadToEnd()}finally{$reader.Dispose()}}finally{$response.Dispose()}
}
try{
 $null=Wait-State {param($s) $s.pageCount -gt 0};Invoke 'Anotasi';Invoke 'Tanda tangan'
 Invoke 'Gambar dari HP';$old=Phone-Url
 if((Request-Phone $old) -notmatch 'canvas'){throw 'Mobile pad missing'}
 Capture 'qr-lan.png';Invoke 'Batalkan QR';Start-Sleep -Milliseconds 400
 $rejected=$false;try{$null=Request-Phone $old}catch{$rejected=$true};if(-not $rejected){throw 'Cancelled session still available'}
 Invoke 'Gambar dari HP';$url=Phone-Url;if($url -eq $old){throw 'QR not renewed'}
 $null=Request-Phone ($url+'/submit') -Submit
 $null=Wait-State {param($s) $s.signatureAsset -and -not $s.dirty -and $s.signaturePanel};Capture 'received-preview-no-dirty.png'
 Invoke 'Gunakan tanda tangan';Invoke 'Isi halaman 1';$null=Wait-State {param($s) $s.signatureCount -eq 1 -and $s.dirty}
 $saved=Join-Path $directory 'PhoneSigned.pdf';$s=Save-Pdf 'Simpan sebagai' $saved
 if(-not $s.exportSuccess -or $s.dirty){throw 'Phone signature save failed'}
 if((Get-FileHash $a).Hash -ne $hashA){throw 'Source changed'}
 Invoke 'Tanda tangan';Invoke 'Gambar dari HP';$url=Phone-Url;Invoke 'Batal tanda tangan';Start-Sleep -Milliseconds 400
 $rejected=$false;try{$null=Request-Phone $url}catch{$rejected=$true};if(-not $rejected){throw 'Dismissed editor still serves phone'}
 Close-Window;if(-not $app.WaitForExit(3000)){
  $owned=Get-CimInstance Win32_Process -Filter "ProcessId=$($app.Id)"
  if($owned.ExecutablePath -ne $exe -or -not $owned.CommandLine.Contains($directory)){throw 'Not an owned fixture'}
  Stop-Process -Id $app.Id
 }
 Write-Host "PASS LAN QR UI: page, cancel, new token, receive preview, placement, Save As, dismiss: $directory"
}catch{Capture 'failure.png';throw}
