# Automated checks on detected physical displays. Does not change global DPI,
# inject global input, or claim that HWND messages prove physical device delivery.
param([ValidateSet('debug','release')][string]$Profile='release',[switch]$LeaveOpen)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -TestName 'device-acceptance'
Add-Type 'using System;using System.Runtime.InteropServices;public static class AcceptanceDisplay{[DllImport("user32.dll")]public static extern uint GetDpiForWindow(IntPtr h);[DllImport("user32.dll")]public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int height,bool repaint);[DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);}'
$checks=New-Object Collections.Generic.List[object]
function Record([string]$Name,[string]$Status,[string]$Evidence) {
    $checks.Add([pscustomobject]@{Check=$Name;Status=$Status;Evidence=$Evidence})
    $checks|ConvertTo-Json -Depth 6|Set-Content -Encoding UTF8 (Join-Path $directory 'checks.json')
    Write-Host "$Status device case: $Name"
}
function Key([int]$Code) {
    [AcceptanceDisplay]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]$Code,[IntPtr]1)|Out-Null
    [AcceptanceDisplay]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]$Code,[IntPtr]0xC0000001)|Out-Null
    Start-Sleep -Milliseconds 200
}
try {
    $null=Wait-State {param($s) $s.pageCount -eq 4}
    $screens=@([Windows.Forms.Screen]::AllScreens)
    $screens|ForEach-Object {[pscustomobject]@{Device=$_.DeviceName;Primary=$_.Primary;X=$_.Bounds.X;Y=$_.Bounds.Y;Width=$_.Bounds.Width;Height=$_.Bounds.Height}}|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'displays.json')
    $observedDpi=@()
    foreach($screen in $screens){
        [AcceptanceDisplay]::MoveWindow($app.MainWindowHandle,$screen.WorkingArea.X+20,$screen.WorkingArea.Y+20,[math]::Min(1200,$screen.WorkingArea.Width-40),[math]::Min(900,$screen.WorkingArea.Height-40),$true)|Out-Null
        Start-Sleep -Milliseconds 500
        $dpi=[AcceptanceDisplay]::GetDpiForWindow($app.MainWindowHandle);$observedDpi+=$dpi
        $s=Wait-State {param($s) [math]::Abs($s.deviceScaleMilli-($dpi/96*1000)) -lt 2}
        Invoke 'Zoom';Invoke 'Sesuaikan halaman'
        $s=Wait-State {param($s) [math]::Abs($s.deviceScaleMilli-($dpi/96*1000)) -lt 2 -and -not $s.textLoading}
        foreach($name in @('Berkas','Edit','Tampilan','Alat','Bantuan','Daftar tab','Buka PDF','Simpan sebagai','Nomor halaman tujuan','Zoom')){
            $node=Find $name
            if($node.Current.IsOffscreen -or $node.Current.BoundingRectangle.IsEmpty){throw "Required control not visible: $name"}
        }
        Capture ("display-"+$screen.DeviceName.Replace('\','').Replace('.','')+"-$dpi.png")
        Record ("physical-display-dpi-$dpi") 'passed' ("Windows DPI and GPUI scale agree; viewport "+$s.viewportWidth+'x'+$s.viewportHeight)
    }
    foreach($dpi in @(96,144,192)){
        if($observedDpi -notcontains $dpi){Record ("physical-scale-"+[int]($dpi/96*100)+'-percent') 'pending-hardware' 'No connected display observed at this actual DPI; no global scale settings changed.'}
    }
    if($screens.Count -lt 2){Record 'physical-mixed-monitor-transition' 'pending-hardware' 'Only one active display is connected.'}
    else{Record 'physical-mixed-monitor-transition' 'partial' 'Owned window moved across connected displays; human reading/drag assessment still required.'}
    $tab=Find 'A.pdf';$close=Find 'Tutup tab: A.pdf'
    if($tab.Current.ControlType -ne [Windows.Automation.ControlType]::TabItem -or $close.Current.ControlType -ne [Windows.Automation.ControlType]::Button){throw 'Tab/close accessibility role mismatch'}
    $input=Find 'Nomor halaman tujuan'
    $input.SetFocus();$input.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue('3')
    Key 13
    $null=Wait-State {param($s) $s.activePage -eq 3}
    Key 34;$null=Wait-State {param($s) $s.activePage -eq 4}
    Key 33;$null=Wait-State {param($s) $s.activePage -eq 3}
    Record 'targeted-hwnd-enter-pageup-pagedown' 'passed' 'Input ValuePattern and targeted key messages changed the expected page; not a physical-keyboard claim.'
    $input=Find 'Nomor halaman tujuan';$input.SetFocus()
    $deadline=(Get-Date).AddSeconds(5)
    do {Start-Sleep -Milliseconds 100;$input=Find 'Nomor halaman tujuan'}while(-not $input.Current.HasKeyboardFocus -and (Get-Date) -lt $deadline)
    if(-not $input.Current.HasKeyboardFocus){throw 'Page input focus was not projected before Tab'}
    Key 9
    $deadline=(Get-Date).AddSeconds(5);$focused=$null
    do {
        Start-Sleep -Milliseconds 100
        $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
        $focused=$ui.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::HasKeyboardFocusProperty,$true))|Where-Object {$_.Current.Name -and $_.Current.Name -ne 'Nomor halaman tujuan'}|Select-Object -First 1
    }while(-not $focused -and (Get-Date) -lt $deadline)
    if(-not $focused){throw 'Tab traversal did not reach a named accessible control'}
    Record 'accessible-roles-valuepattern-and-tab-focus' 'passed' ('Tab focus moved to '+$focused.Current.Name)
    $dpi=[AcceptanceDisplay]::GetDpiForWindow($app.MainWindowHandle)
    [AcceptanceDisplay]::MoveWindow($app.MainWindowHandle,20,20,[int](900*$dpi/96),[math]::Min(900,$screens[0].WorkingArea.Height-40),$true)|Out-Null
    Start-Sleep -Milliseconds 300
    Invoke 'Panel alat';Start-Sleep -Milliseconds 300
    Key 27
    $null=Wait-State {param($s) -not $s.organizing}
    Record 'targeted-escape-tool-panel-focus' 'passed' 'Escape returns to reader through the toolkit action path.'
    Record 'physical-modifier-shortcuts' 'pending-human' 'Ctrl+O, Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+W, Ctrl+Shift+S, Ctrl+C and text-field editing need physical delivery acceptance.'
    Record 'physical-trackpad-pinch-scroll-pan' 'pending-human' 'A HID touch pad is present; device presence and unit math are not gesture delivery evidence.'
    Record 'screen-reader-announcements' 'pending-human' 'UIAutomation names/roles/focus were checked; Narrator announcements and reading order require listening with assistive technology.'
    if((Get-FileHash $a).Hash -ne $hashA){throw 'Device acceptance changed its source'}
    Write-Host "PASS automated device/accessibility checks; physical pending cases recorded: $directory"
} catch {
    Record 'automated-suite' 'failed' $_.Exception.Message
    if($app -and -not $app.HasExited){Capture 'failure.png'}
    throw
} finally {
    if(-not $LeaveOpen -and $app -and -not $app.HasExited){Close-Window;$null=$app.WaitForExit(10000)}
}
