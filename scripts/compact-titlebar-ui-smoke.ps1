param([string]$ExecutablePath='target/tab-background-check/release/pdf-studio.exe')
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile release -TestName 'compact-titlebar' -ExecutablePath $ExecutablePath -IsolatedState
Add-Type 'using System;using System.Runtime.InteropServices;public class CaptionTest{[DllImport("user32.dll")]public static extern bool IsZoomed(IntPtr h);[DllImport("user32.dll")]public static extern bool IsIconic(IntPtr h);[DllImport("user32.dll")]public static extern bool ShowWindow(IntPtr h,int n);}'
function Hit([int]$x,[int]$y){
 $point=New-Object OrganizerClick+Point;$point.X=$x;$point.Y=$y
 [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$point)|Out-Null
 $client=[IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
 [OrganizerClick]::SendMessage($app.MainWindowHandle,0x200,[IntPtr]::Zero,$client)|Out-Null
 Start-Sleep -Milliseconds 150
 $lp=[IntPtr](($y -shl 16) -bor ($x -band 0xffff));[OrganizerClick]::SendMessage($app.MainWindowHandle,0x84,[IntPtr]::Zero,$lp).ToInt32()}
function Caption([int]$code){[OutputDialog]::PostMessage($app.MainWindowHandle,0x112,[IntPtr]$code,[IntPtr]::Zero)|Out-Null;Start-Sleep -Milliseconds 500}
try{
 $null=Wait-State {param($s) $s.pageCount -gt 0}
 $menu=(Find 'Berkas').Current.BoundingRectangle
 $rect=New-Object M3Capture+Rect;[M3Capture]::GetWindowRect($app.MainWindowHandle,[ref]$rect)|Out-Null
 if($menu.Y-$rect.T -gt 12){throw 'Menu still occupies a second row'}
 if((Hit ([int]($menu.X+$menu.Width/2)) ([int]($menu.Y+$menu.Height/2))) -ne 1){throw 'Menu incorrectly treated as drag area'}
 if((Hit ($rect.L+600) ($rect.T+17)) -ne 2){throw 'Empty titlebar is not draggable'}
 foreach($control in @(@(17,20),@(51,9),@(85,8))){if((Hit ($rect.R-$control[0]) ($rect.T+17)) -ne $control[1]){throw 'Caption control hit-test failed'}}
 Menu-Action 'Tampilan' 'Terang';Capture 'light.png'
 Menu-Action 'Tampilan' 'Gelap';Capture 'dark.png'
 Caption 0xF030;if(-not [CaptionTest]::IsZoomed($app.MainWindowHandle)){throw 'Maximize failed'};$null=Find 'Restore'
 Caption 0xF120;if([CaptionTest]::IsZoomed($app.MainWindowHandle)){throw 'Restore failed'};$null=Find 'Maximize'
 Caption 0xF020;if(-not [CaptionTest]::IsIconic($app.MainWindowHandle)){throw 'Minimize failed'}
 [CaptionTest]::ShowWindow($app.MainWindowHandle,9)|Out-Null
 if((Get-FileHash $a).Hash -ne $hashA){throw 'Source changed'}
 Close-Window
 if(-not $app.WaitForExit(10000)){throw 'Window close failed'}
 @{status='passed';cases=@('single-row','menu-client-hit','drag-caption-hit','caption-controls-hit','menus-light-dark','maximize-restore','minimize','close');sourceUnchanged=$true}|ConvertTo-Json|Set-Content (Join-Path $directory 'result.json')
 Write-Host "PASS compact titlebar: $directory"
}catch{Capture 'failure.png';throw}
