param([ValidateSet('debug','release')][string]$Profile='release',[string]$ExecutablePath)
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'branding-ui' -IsolatedState -ExecutablePath $ExecutablePath
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class BrandIcon {
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode)] static extern IntPtr LoadLibraryEx(string path,IntPtr file,uint flags);
 [DllImport("kernel32.dll")] static extern bool FreeLibrary(IntPtr module);
 [DllImport("kernel32.dll")] static extern IntPtr FindResource(IntPtr module,IntPtr name,IntPtr type);
 [DllImport("kernel32.dll")] static extern IntPtr LoadResource(IntPtr module,IntPtr resource);
 [DllImport("kernel32.dll")] static extern IntPtr LockResource(IntPtr data);
 [DllImport("user32.dll",EntryPoint="GetClassLongPtrW")] public static extern IntPtr ClassIcon(IntPtr hwnd,int index);
 public static int[] Sizes(string path) {
  IntPtr module=LoadLibraryEx(path,IntPtr.Zero,0x22);
  if(module==IntPtr.Zero)throw new Exception("Cannot inspect executable resources");
  try {
   IntPtr resource=FindResource(module,new IntPtr(1),new IntPtr(14));
   if(resource==IntPtr.Zero)throw new Exception("Missing icon group ID 1");
   IntPtr data=LockResource(LoadResource(module,resource));
   int count=Marshal.ReadInt16(data,4);int[] sizes=new int[count];
   for(int i=0;i<count;i++){int w=Marshal.ReadByte(data,6+i*14);sizes[i]=w==0?256:w;}
   return sizes;
  } finally{FreeLibrary(module);}
 }
}
'@
try {
 $null=Wait-State {param($s) $s.pageCount -eq 4}
 if(([BrandIcon]::Sizes($exe)-join ',') -ne '16,20,24,32,40,48,64,128,256'){throw 'Unexpected embedded icon sizes'}
 $version=[Diagnostics.FileVersionInfo]::GetVersionInfo($exe)
 if($version.ProductName -ne 'Origo' -or $version.FileDescription -ne 'Origo'){throw 'Origo product metadata missing'}
 $app.Refresh();if($app.MainWindowTitle -ne 'Origo'){throw "Unexpected native title: $($app.MainWindowTitle)"}
 $handle=[BrandIcon]::ClassIcon($app.MainWindowHandle,-14)
 if($handle -eq [IntPtr]::Zero){throw 'Window class has no icon (taskbar/Alt+Tab fallback)'}
 $native=[Drawing.Icon]::FromHandle($handle).ToBitmap()
 $expected=New-Object Drawing.Bitmap (Join-Path $root 'apps/desktop/resources/branding/origo-32.png')
 try {
  if($native.Width -ne 32 -or $native.Height -ne 32){throw 'Unexpected native icon dimensions'}
  for($y=0;$y -lt 32;$y++){for($x=0;$x -lt 32;$x++){
   if($native.GetPixel($x,$y).ToArgb() -ne $expected.GetPixel($x,$y).ToArgb()){throw "Window icon differs from supplied mark at $x,$y"}
  }}
  $native.Save((Join-Path $directory 'native-window-icon.png'))
 }finally{$native.Dispose();$expected.Dispose()}
 Capture 'origo-light.png'
 Menu-Action 'Tampilan' 'Gelap';Capture 'origo-dark.png'
 Menu-Action 'Bantuan' 'Tentang Origo';$null=Find 'OK';Capture 'origo-about.png'
 Invoke 'OK'
 if((Get-FileHash $a).Hash -ne $hashA){throw 'Source changed'}
 Close-Window
 if(-not $app.WaitForExit(10000)){throw 'Owned fixture did not close'}
 [pscustomobject]@{Status='passed';IconSizes=[BrandIcon]::Sizes($exe);NativeWindowIconMatches=$true;ProductName=$version.ProductName;ExecutableSha256=(Get-FileHash $exe).Hash}|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'result.json')
 Write-Host "PASS Origo branding: native resources, class icon, display name and About. Evidence: $directory"
}catch{Capture 'failure.png';throw}
