# Build and launch the experimental native game from any working directory.
param([string]$Track = 'HELL5.TRK', [switch]$Software)
$ErrorActionPreference = 'Stop'
if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    . (Join-Path $PSScriptRoot 'tools/use-local-rust.ps1')
}
$frontendFeature = if ($Software) { 'desktop' } else { 'modern' }
$frontendBinary = if ($Software) { 'stunts-play' } else { 'stunts-modern' }
& cargo build --release --manifest-path (Join-Path $PSScriptRoot 'stunts-rs/Cargo.toml') --features $frontendFeature --bin $frontendBinary
if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed.' }
& (Join-Path $PSScriptRoot "stunts-rs/target/release/$frontendBinary.exe") --data (Join-Path $PSScriptRoot 'stunts_dos') --track $Track
if ($LASTEXITCODE -ne 0) { throw 'Desktop game failed.' }
