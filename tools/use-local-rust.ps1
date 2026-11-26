# Optional helper for the toolchain installed locally during the initial analysis.
# Dot-source this file: . .\tools\use-local-rust.ps1
$projectRoot = Split-Path $PSScriptRoot -Parent
$localCargoBin = Join-Path $projectRoot 'reverse\tools\cargo\bin'
if (!(Test-Path -LiteralPath (Join-Path $localCargoBin 'cargo.exe'))) {
    throw 'Local Rust toolchain not found. Install Rust normally or recreate the local toolchain described in docs/reverse-engineering.md.'
}
$env:RUSTUP_HOME = Join-Path $projectRoot 'reverse\tools\rustup'
$env:CARGO_HOME = Join-Path $projectRoot 'reverse\tools\cargo'
# Dot-sourcing exposes Cargo only to this shell; no persistent PATH is changed.
$env:PATH = $localCargoBin + [IO.Path]::PathSeparator + $env:PATH
# The local GNU toolchain uses its bundled linker instead of a system GCC install.
$env:RUSTFLAGS = '-C linker=rust-lld'
