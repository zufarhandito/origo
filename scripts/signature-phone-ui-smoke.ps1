param([string]$ExecutablePath='target/tab-background-check/release/pdf-studio.exe')
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile release -TestName 'signature-phone-ui' -IsolatedState -ExecutablePath $ExecutablePath
function Assert-QR-Isolated {
 $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
 $nodes=$ui.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
 $hidden=@('Area gambar tanda tangan','Undo goresan','Hapus semua','Impor gambar','Pilih gambar tanda tangan','Hitam','Biru','Merah','Tipis','Sedang','Tebal')
 foreach($node in $nodes){
  $name=$node.Current.Name
  if($name -match 'http://|Alamat gambar HP:' -or $name -in $hidden){throw 'QR view exposed a drawing control or raw URL'}
 }
}
function Phone-Url {
 $null=Find 'Pindai QR dengan kamera HP';$null=Find 'Menunggu gambar dari HP...'
 Assert-QR-Isolated
 if((Find 'Gunakan Tanda Tangan').Current.IsEnabled){throw 'Use enabled while waiting for phone drawing'}
 Invoke 'Salin tautan';$null=Find 'Tautan disalin.'
 $link=[Windows.Forms.Clipboard]::GetText()
 if($link -notmatch '^http://(?:\d{1,3}\.){3}\d{1,3}:\d+/draw/[a-f0-9]{48}$'){throw 'Pairing link was not copied'}
 $script:copiedPhoneUrl=$link
 return $link
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
# Preserve the existing clipboard without writing its contents into test output.
$savedClipboard=[Windows.Forms.Clipboard]::GetDataObject()
$script:copiedPhoneUrl=$null
try{
 $null=Wait-State {param($s) $s.pageCount -gt 0};Invoke 'Anotasi';Invoke 'Tanda tangan'
 Invoke 'Gambar dari HP';$old=Phone-Url
 if((Request-Phone $old) -notmatch 'canvas'){throw 'Mobile pad missing'}
 Capture 'qr-lan.png';Invoke 'Batalkan QR';$null=Find 'Area gambar tanda tangan';Start-Sleep -Milliseconds 400
 $rejected=$false;try{$null=Request-Phone $old}catch{$rejected=$true};if(-not $rejected){throw 'Cancelled session still available'}
 Invoke 'Gambar dari HP';$url=Phone-Url;if($url -eq $old){throw 'QR not renewed'}
 $null=Request-Phone ($url+'/submit') -Submit
 $null=Wait-State {param($s) $s.signatureAsset -and -not $s.dirty -and $s.signaturePanel}
 $null=Find 'Area gambar tanda tangan';Capture 'received-preview-no-dirty.png'
 Invoke 'Gunakan Tanda Tangan';Invoke 'Isi halaman 1';$null=Wait-State {param($s) $s.signatureCount -eq 1 -and $s.dirty}
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
 Write-Host "PASS LAN QR UI: isolated view, link copy, page, cancel, new token, receive preview, placement, Save As, dismiss: $directory"
}catch{Capture 'failure.png';throw}
finally{
 if($script:copiedPhoneUrl -and [Windows.Forms.Clipboard]::GetText() -ceq $script:copiedPhoneUrl){
  if($savedClipboard){[Windows.Forms.Clipboard]::SetDataObject($savedClipboard,$true)}else{[Windows.Forms.Clipboard]::Clear()}
 }
}
