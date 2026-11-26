# Stunts in Rust

An early-stage reimplementation of Stunts for DOS in Rust, using Ghidra to study the original routines.

The project cannot run the game yet. The current tool lists files and inspects DOS MZ headers and ASCII strings.

## Running

Install the Rust toolchain. The original game files are in the `stunts_dos/` folder at the repository root.

```powershell
cd stunts-rs
cargo run -- inventory ..\stunts_dos
cargo run -- inspect ..\stunts_dos\LOAD.EXE
cargo run -- inspect ..\stunts_dos\MCGA.HDR
```

## Layout

- `stunts-rs/`: Rust source for the analysis tools and future reimplementation.
- `stunts_dos/`: original game files, included in Git for analysis.
- `reverse/`: local Ghidra project and analysis records, excluded from Git.

## Planned steps

1. Identify the game version and study module loading and decompression.
2. Reconstruct a static image for analysis in Ghidra.
3. Implement resource readers using confirmed formats.
4. Port math and physics while preserving DOS integer behavior.
5. Compare per-frame states using replays before integrating graphics, input, and audio.

## References

- [ReStunts](https://github.com/4d-stunts/restunts)
- [ReStunts2](https://github.com/dstien/restunts2)

The executables and resources in `stunts_dos/` belong to their respective rights holders. Their inclusion does not change ownership of these files.
