# Stunts in Rust

An early-stage reimplementation of Stunts for DOS in Rust, using Ghidra to study the original routines.

The project cannot run the game yet. The Rust tools list files, inspect DOS MZ headers, decode the game's compressed resources, and reconstruct packed executables for four video modes.

The first reverse engineering milestone decoded all 79 compressed resources and reconstructed the MCGA executable for analysis in Ghidra. See [the findings and reproduction steps](docs/reverse-engineering.md).

The next milestone adds readers for the 41 original tracks and the default replay, maps key simulation functions in Ghidra, and ports the first fixed-point arithmetic routine with checks against executed DOS instructions. See [tracks, replays, and simulation](docs/track-replay-simulation.md).

The latest mapping adds resource directories, parameters for all eleven cars, and a steering-response port checked in 1488 original-code executions. Aero tables are checked against 22 original car-setup executions. See [steering and car parameters](docs/steering-car-parameters.md).

Gearbox stages, torque selection, and the RPM helper are now ported with 2764 additional original-code comparisons. See [drivetrain mapping](docs/drivetrain.md). The full speed update is still pending, and `reverse/` remains local and excluded from Git.

## Running

Install the Rust toolchain. The original game files are in the `stunts_dos/` folder at the repository root.

```powershell
cd stunts-rs
cargo run -- inventory ..\stunts_dos
cargo run -- inspect ..\stunts_dos\LOAD.EXE
cargo run -- inspect ..\stunts_dos\MCGA.HDR
cargo run -- track ..\stunts_dos\HELL5.TRK
cargo run -- replay ..\stunts_dos\DEFAULT.RPL
cargo run -- car ..\stunts_dos\CARPMIN.RES
cargo run -- steer -100 0 1 20
cargo run -- gear-tick ..\stunts_dos\CARPMIN.RES 1 9000 16 20
cargo test
```

## Layout

- `stunts-rs/`: Rust source for the analysis tools and future reimplementation.
- `stunts_dos/`: original game files, included in Git for analysis.
- `reverse/`: local Ghidra project and analysis records, excluded from Git.
- `docs/`: reverse engineering findings and exported text inventories.
- `tools/ghidra/`: scripts for verified function names, simulation calls, and analysis exports.

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
