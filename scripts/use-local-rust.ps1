# Dot-source: . ./scripts/use-local-rust.ps1
$root = Split-Path $PSScriptRoot -Parent
if (-not (Test-Path -LiteralPath (Join-Path $root '.tools/cargo/bin/cargo.exe'))) {
    throw 'Project-local Rust is unavailable. Install the pinned Rust MSVC toolchain with rustup.'
}
$env:CARGO_HOME = Join-Path $root '.tools/cargo'
$env:RUSTUP_HOME = Join-Path $root '.tools/rustup'
$env:PATH = (Join-Path $root '.tools/cargo/bin') + ';' + $env:PATH

