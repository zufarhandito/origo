# Explicit network audit: sends public crate names/versions only to OSV.
param([Parameter(Mandatory=$true)][string]$InventoryPath)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'distribution-support.ps1')
$inventory = Get-Content -Raw -LiteralPath $InventoryPath | ConvertFrom-Json
$packages = @($inventory | Where-Object Source -Like 'registry+*')
if (-not $packages.Count) { throw 'Inventory has no registry packages to audit' }
$results = New-Object Collections.Generic.List[object]
foreach ($package in $packages) { $results.Add([pscustomobject]@{Name=$package.Name;Version=$package.Version;Ids=@()}) }
# Limit batches and follow pagination rather than treating an incomplete page as clear.
for ($offset=0; $offset -lt $packages.Count; $offset+=100) {
    $last = [math]::Min($offset+99,$packages.Count-1)
    $queries = @($packages[$offset..$last] | ForEach-Object {@{package=@{name=$_.Name;ecosystem='crates.io'};version=$_.Version}})
    $batch = Invoke-RestMethod -Method Post -Uri 'https://api.osv.dev/v1/querybatch' -ContentType 'application/json' -Body (@{queries=$queries}|ConvertTo-Json -Depth 6) -TimeoutSec 30
    if ($batch.results.Count -ne $queries.Count) { throw 'Incomplete OSV batch response' }
    for ($j=0; $j -lt $queries.Count; $j++) {
        $entry = $batch.results[$j]
        $ids = @($entry.vulns | ForEach-Object id)
        while ($entry.next_page_token) {
            $query = $queries[$j].Clone(); $query.page_token = $entry.next_page_token
            $next = Invoke-RestMethod -Method Post -Uri 'https://api.osv.dev/v1/querybatch' -ContentType 'application/json' -Body (@{queries=@($query)}|ConvertTo-Json -Depth 6) -TimeoutSec 30
            if ($next.results.Count -ne 1) { throw 'Incomplete OSV pagination response' }
            $entry = $next.results[0]; $ids += @($entry.vulns | ForEach-Object id)
        }
        $results[$offset+$j].Ids = @($ids | Sort-Object -Unique)
    }
    Write-Host "Queried OSV: $($last+1)/$($packages.Count)"
}
$path = Join-Path (Split-Path $InventoryPath -Parent) 'osv-audit.json'
Write-DistributionJson ([pscustomobject]@{QueriedUtc=[DateTime]::UtcNow.ToString('o');Service='https://api.osv.dev/v1/querybatch';Scope='Cargo desktop normal/build superset; native engines and reachability not assessed';Results=$results.ToArray()}) $path
$findings = @($results | Where-Object {$_.Ids.Count})
Write-Host "OSV results: $($findings.Count) package versions with advisory matches. Review applicability; not a release clearance. $path"
$findings | Format-Table Name,Version,Ids -AutoSize | Out-Host
