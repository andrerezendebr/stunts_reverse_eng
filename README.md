# Stunts reverse engineering in Rust

A partial native port of the original DOS game, using Ghidra for analysis and original x86 execution to validate behavior. The current project provides a reusable Rust library and an analysis CLI. It is **not a playable game yet**.

## Implemented

- Resource decompression validated against all 79 compressed files; directory readers for 13 plain and 71 decoded resource archives.
- Readers for all 41 tracks, the default replay, and parameters for all eleven cars.
- Reconstruction of the original DOS executables for MCGA, CGA, Tandy and EGA.
- Complete speed and grip kernels, gearbox/torque/RPM handling, steering, exact trigonometry, matrices, rotations and random state.
- A bounded preparation prefix of player physics; full world movement, suspension and collisions remain pending.
- Native control traces checked against 6600 sequential frames of original DOS function execution.
- **36 tests**, including original-code vectors, sequential state comparisons, format validation and failure rollback.

## Run

With Rust installed, from the repository root:

```powershell
cargo test --manifest-path stunts-rs/Cargo.toml
cargo run --manifest-path stunts-rs/Cargo.toml -- car stunts_dos/CARPMIN.RES
cargo run --manifest-path stunts-rs/Cargo.toml -- replay stunts_dos/DEFAULT.RPL
cargo run --manifest-path stunts-rs/Cargo.toml -- track stunts_dos/HELL5.TRK
```

To export a native control-kernel trace on Windows:

```powershell
New-Item -ItemType Directory -Force reverse/decoded
cargo build --release --manifest-path stunts-rs/Cargo.toml
.\stunts-rs\target\release\stunts-rs.exe control-trace stunts_dos/CARPMIN.RES 20 150 reverse/decoded/INDY-control-native.csv
```

The output file must not already exist. This runs control calculations without position integration, suspension, collisions or graphics. Values are raw game quantities, not real-world telemetry. The library has no third-party runtime dependencies; optional Python/Unicorn tools regenerate the development fixtures.

## Layout

- `stunts-rs/`: reusable Rust library, command-line tool and validation fixtures.
- `stunts_dos/`: original game files, included for analysis and tests.
- `docs/`: findings, formats and exported text inventories.
- `tools/`: Ghidra scripts and optional original-CPU comparison tools.
- `reverse/`: local Ghidra database, reconstructed binaries, caches and reference tools; **excluded from Git**.

## Analysis notes

- [Executable reconstruction and compression](docs/reverse-engineering.md)
- [Tracks, replays and simulation entry points](docs/track-replay-simulation.md)
- [Steering tables and car parameters](docs/steering-car-parameters.md)
- [Gearbox, torque and RPM stages](docs/drivetrain.md)
- [Complete speed/grip kernels and numerical foundations](docs/speed-grip-numerics.md)

Next major work: the remaining `update_player_state` physics, terrain/contact planes, full replay world-state comparison, opponents, rendering, audio and device input.

## References

- [ReStunts](https://github.com/4d-stunts/restunts)
- [ReStunts2](https://github.com/dstien/restunts2)

The executables and resources in `stunts_dos/` belong to their respective rights holders. Their inclusion does not change ownership of these files.
