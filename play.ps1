# Build and launch the experimental native game from any working directory.
param([string]$Track = 'HELL5.TRK')
$ErrorActionPreference = 'Stop'
if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    . (Join-Path $PSScriptRoot 'tools/use-local-rust.ps1')
}
& cargo build --release --manifest-path (Join-Path $PSScriptRoot 'stunts-rs/Cargo.toml') --features desktop --bin stunts-play
if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed.' }
& (Join-Path $PSScriptRoot 'stunts-rs/target/release/stunts-play.exe') --data (Join-Path $PSScriptRoot 'stunts_dos') --track $Track
if ($LASTEXITCODE -ne 0) { throw 'Desktop game failed.' }
