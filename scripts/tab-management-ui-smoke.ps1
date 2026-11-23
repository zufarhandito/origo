param([ValidateSet('debug','release')][string]$Profile='debug',[string]$ExecutablePath)
. (Join-Path $PSScriptRoot 'tab-test-support.ps1') -Profile $Profile -ExecutablePath $ExecutablePath -TestName 'tab-management-ui' -IsolatedState
function Assert-Hidden([string]$Name) {
    $ui=[Windows.Automation.AutomationElement]::FromHandle($app.MainWindowHandle)
    $condition=[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$Name)
    if($ui.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)){throw "Unexpected control: $Name"}
}
function Context-Tab([string]$Name) {
    $n=Find $Name
    $r=$n.Current.BoundingRectangle
    $pt=New-Object OrganizerClick+Point
    $pt.X=[int]($r.X+$r.Width/2);$pt.Y=[int]($r.Y+$r.Height/2)
    [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$pt)|Out-Null
    $lp=[IntPtr](($pt.Y -shl 16) -bor ($pt.X -band 0xffff))
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x204,[IntPtr]2,$lp)|Out-Null
    [OrganizerClick]::SendMessage($app.MainWindowHandle,0x205,[IntPtr]0,$lp)|Out-Null
    Start-Sleep -Milliseconds 200
}
function Drag-Tab([string]$From,[string]$To,[switch]$Cancel) {
    $points=@()
    foreach($name in @($From,$To)) {
        $r=(Find $name).Current.BoundingRectangle
        $pt=New-Object OrganizerClick+Point
        $pt.X=[int]($r.X+$r.Width/2);$pt.Y=[int]($r.Y+$r.Height/2)
        [OrganizerClick]::ScreenToClient($app.MainWindowHandle,[ref]$pt)|Out-Null
        $points+=$pt
    }
    function Mouse($message,$flags,$x,$y) {
        $packed=[IntPtr](([int]$y -shl 16) -bor ([int]$x -band 0xffff))
        [OutputDialog]::PostMessage($app.MainWindowHandle,$message,[IntPtr]$flags,$packed)|Out-Null
        Start-Sleep -Milliseconds 35
    }
    $originPoint=$points[0];$destinationPoint=$points[1]
    Mouse 0x200 0 $originPoint.X $originPoint.Y
    Mouse 0x201 1 $originPoint.X $originPoint.Y
    for($i=1;$i -le 12;$i++){Mouse 0x200 1 ($originPoint.X+($destinationPoint.X-$originPoint.X)*$i/12) ($originPoint.Y+($destinationPoint.Y-$originPoint.Y)*$i/12)}
    if($Cancel){
        [OutputDialog]::PostMessage($app.MainWindowHandle,0x100,[IntPtr]27,[IntPtr]0x10001)|Out-Null
        [OutputDialog]::PostMessage($app.MainWindowHandle,0x101,[IntPtr]27,[IntPtr]0xC0010001)|Out-Null
        Start-Sleep -Milliseconds 200
    }
    Mouse 0x202 0 $destinationPoint.X $destinationPoint.Y
}
try {
    $s=Wait-State {param($s) $s.tabCount -eq 1 -and $s.favoritesReady}
    $idA=$s.tabId
    $close=Find 'Tutup tab: A.pdf'
    $plus=Find 'Buka PDF di tab baru'
    if($plus.Current.BoundingRectangle.X-$close.Current.BoundingRectangle.Right -gt 55){throw 'Plus does not follow final tab'}
    Invoke 'Bintangi: A.pdf'
    $null=Wait-State {param($s) $s.starredTabIds -contains $idA -and $s.pendingPreferences -eq 0}
    if(-not (Test-Path (Join-Path $directory 'state/favorites.json'))){throw 'Favorite was not persisted in isolated state directory'}
    Picker 'Buka PDF di tab baru';Fill-Picker $b
    $s=Wait-State {param($s) $s.tabCount -eq 2 -and $s.tabId -ne $idA}
    $idB=$s.tabId
    $null=Find 'Bintangi: B.pdf'
    Assert-Hidden 'Hapus bintang: A.pdf'
    Context-Tab 'A.pdf'
    Assert-Hidden 'Hapus bintang'
    Assert-Hidden 'Bintangi'
    Invoke 'Pindahkan ke akhir'
    $null=Wait-State {param($s) $s.tabOrder[0] -eq $idB -and $s.tabOrder[1] -eq $idA -and $s.tabId -eq $idB}
    Context-Tab 'A.pdf'
    Invoke 'Pindahkan ke awal'
    $null=Wait-State {param($s) $s.tabOrder[0] -eq $idA -and $s.tabId -eq $idB}
    Switch-Tab $a
    $null=Wait-State {param($s) $s.tabId -eq $idA -and $s.starredTabIds -contains $idA}
    $null=Find 'Hapus bintang: A.pdf'
    Assert-Hidden 'Bintangi: B.pdf'
    Switch-Tab $b
    Drag-Tab 'B.pdf' 'A.pdf' -Cancel
    $null=Wait-State {param($s) $s.tabOrder[0] -eq $idA -and -not $s.dirty}
    Drag-Tab 'B.pdf' 'A.pdf'
    $null=Wait-State {param($s) $s.tabOrder[0] -eq $idB -and -not $s.dirty}
    Context-Tab 'A.pdf';Invoke 'Pindahkan ke awal'
    $null=Wait-State {param($s) $s.tabOrder[0] -eq $idA}
    # Dirty B must retain both tabs if closing tabs to the right is cancelled.
    Switch-Tab $b
    Invoke 'Panel alat';Invoke 'Putar kanan'
    $null=Wait-State {param($s) $s.dirty}
    Context-Tab 'A.pdf';Invoke 'Tutup tab di kanan'
    $null=Find 'Simpan perubahan sebelum melanjutkan?'
    Invoke 'Batal'
    $null=Wait-State {param($s) $s.tabCount -eq 2 -and $s.dirty -and -not $s.closingTabs}
    Context-Tab 'A.pdf';Invoke 'Tutup tab lainnya'
    $null=Find 'Simpan perubahan sebelum melanjutkan?'
    Invoke 'Buang perubahan'
    $null=Wait-State {param($s) $s.tabCount -eq 1 -and $s.tabId -eq $idA -and $s.starredTabIds -contains $idA}
    Capture 'tab-management.png'
    Invoke 'Tutup tab: A.pdf'
    $null=Wait-State {param($s) $s.tabCount -eq 0}
    Close-Window
    if(-not $app.WaitForExit(15000)){throw 'Fixture did not close'}
    $arguments='"'+$a+'" --metrics "'+$metrics+'" --state-dir "'+$stateDirectory+'"'
    $app=Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory (Split-Path $exe) -WindowStyle Normal -PassThru
    $deadline=(Get-Date).AddSeconds(20)
    do {Start-Sleep -Milliseconds 200;$app.Refresh()}while((Get-Date) -lt $deadline -and $app.MainWindowHandle -eq 0 -and !$app.HasExited)
    $null=Wait-State {param($s) $s.tabCount -eq 1 -and $s.favoritesReady -and $s.starredTabIds -contains $s.tabId}
    Invoke 'Hapus bintang: A.pdf'
    $null=Wait-State {param($s) $s.starredTabIds.Count -eq 0 -and $s.pendingPreferences -eq 0}
    Close-Window
    if(-not $app.WaitForExit(15000)){throw 'Restart fixture did not close'}
    if((Get-FileHash $a).Hash -ne $hashA -or (Get-FileHash $b).Hash -ne $hashB){throw 'Source changed'}
    'PASS plus placement, active-tab-only stars, persistent favorites, stable reorder, inactive context target, dirty close batch cancel/discard and source hashes' | Set-Content (Join-Path $directory 'result.txt')
    Write-Host "PASS tab management: $directory"
} catch {
    if($app -and -not $app.HasExited){Capture 'failure.png'}
    throw
}
