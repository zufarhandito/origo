$ErrorActionPreference = 'Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    & cargo run --locked -p pdf-smoke
    if ($LASTEXITCODE -ne 0) { throw 'Native smoke test failed' }
} finally { Pop-Location }

