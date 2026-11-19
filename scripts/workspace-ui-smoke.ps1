param([string]$ExecutablePath='target/tab-background-check/release/pdf-studio.exe')
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile release -TestName 'workspace-ui' -ExecutablePath $ExecutablePath -IsolatedState
$cases=New-Object Collections.Generic.List[object]
function Missing([string]$name){$ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle);if($ui.FindFirst([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$name))){throw "Unexpected control: $name"}}
function Pass([string]$name){Capture ($name+'.png');$cases.Add($name);Write-Host "PASS workspace: $name"}
Add-Type 'using System;using System.Runtime.InteropServices;public class WorkspaceResize{[DllImport("user32.dll")]public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int ht,uint flags);}'
try {
 $null=Wait-State {param($s) $s.pageCount -eq 4 -and -not $s.textLoading}
 foreach($name in @('Halaman pertama','Halaman sebelumnya','Halaman berikutnya','Halaman terakhir','Undo','Redo','Pen','Berkas')){Missing $name}
 $menu=(Find 'Menu aplikasi').Current.BoundingRectangle;$tabs=(Find 'Daftar tab').Current.BoundingRectangle
 if([Math]::Abs($menu.Y-$tabs.Y) -gt 3){throw 'Menu and tabs occupy separate rows'}
 $search=(Find 'Cari teks').Current.BoundingRectangle;$zoom=(Find 'Perkecil').Current.BoundingRectangle
 if($zoom.X-$search.Right -lt 12){throw 'Search and zoom need separation'}
 Pass 'compact-read-header'
 Menu-Action 'Tampilan' 'Sesuaikan halaman';Pass 'hamburger-submenu-action'
 $field=Find 'Nomor halaman tujuan';$field.SetFocus();$field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('2')
 [OutputDialog]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]13,[IntPtr]0x10001)|Out-Null
 [OutputDialog]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]13,[IntPtr]0xC0010001)|Out-Null
 $null=Wait-State {param($s) $s.activePage -eq 2};Pass 'page-number-navigation'
 Invoke 'Anotasi';$null=Find 'Pen';$null=Find 'Undo';Pass 'contextual-annotations'
 Invoke 'Anotasi';Missing 'Pen';Missing 'Undo'
 Invoke 'Panel alat';$null=Find 'Undo';Invoke 'Hapus';$null=Wait-State {param($s) $s.pageCount -eq 3 -and $s.dirty}
 Invoke 'Panel alat';$null=Find 'Undo';Invoke 'Undo';$null=Wait-State {param($s) $s.pageCount -eq 4 -and -not $s.dirty}
 if(-not (Find 'Redo').Current.IsEnabled){throw 'Read mode lost redo access'}
 Pass 'history-available-in-read-mode'
 Menu-Action 'Tampilan' 'Gelap';[WorkspaceResize]::SetWindowPos($app.MainWindowHandle,[IntPtr]::Zero,50,50,900,620,0x14)|Out-Null;Start-Sleep -Milliseconds 500
 $null=Find 'Menu aplikasi';$null=Find 'Anotasi';Pass 'narrow-dark-layout'
 Menu-Action 'Berkas' 'Tutup tab  Ctrl+W';$null=Wait-State {param($s) $s.tabCount -eq 0};Missing 'Daftar tab';$null=Find 'Buka PDF';Pass 'empty-hamburger-only'
 Picker 'Buka PDF';Fill-Picker (Join-Path $root '.research/search-fixtures/blank.pdf');$null=Wait-State {param($s) $s.tabCount -eq 1 -and $s.pageCount -eq 1 -and -not $s.textLoading}
 $ocr=Find 'Butuh OCR';if($ocr.Current.IsEnabled){throw 'OCR must be coming soon, not actionable'};Pass 'ocr-disabled-chip'
 if((Get-FileHash $a).Hash -ne $hashA){throw 'Source changed'}
 Close-Window;if(-not $app.WaitForExit(10000)){throw 'Fixture did not close'}
 @{status='passed';cases=$cases.ToArray();sourceUnchanged=$true}|ConvertTo-Json|Set-Content (Join-Path $directory 'result.json')
 Write-Host "Workspace evidence: $directory"
}catch{Capture 'failure.png';throw}
