# Build and launch the GPU frontend from any working directory.
param([string]$Track='HELL5.TRK',[string]$Car='CARPMIN.RES')
$ErrorActionPreference='Stop'
if (!(Get-Command cargo -ErrorAction SilentlyContinue)) { . (Join-Path $PSScriptRoot 'tools/use-local-rust.ps1') }
& cargo build --release --manifest-path (Join-Path $PSScriptRoot 'stunts-rs/Cargo.toml') --features modern --bin stunts-modern
if ($LASTEXITCODE -ne 0) {throw 'GPU frontend build failed.'}
& (Join-Path $PSScriptRoot 'stunts-rs/target/release/stunts-modern.exe') --data (Join-Path $PSScriptRoot 'stunts_dos') --track $Track --car $Car
if ($LASTEXITCODE -ne 0) {throw 'GPU frontend failed.'}
