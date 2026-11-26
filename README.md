# Stunts reverse engineering in Rust

A partial native port of the original DOS game, using Ghidra for analysis and original x86 execution to validate behavior. The project provides a reusable Rust library, an analysis CLI and a **playable experimental desktop prototype**. Full original-game physics and track rendering are still being reconstructed.

## Implemented

- Resource decompression validated against all 79 compressed files; directory readers for 13 plain and 71 decoded resource archives.
- Readers for all 41 tracks, the default replay, and parameters for all eleven cars.
- Reconstruction of the original DOS executables for MCGA, CGA, Tandy and EGA.
- Complete speed and grip kernels, gearbox/torque/RPM handling, steering, exact trigonometry, matrices, rotations and random state.
- Original player-physics preparation and four-wheel prediction, checked against 704 original-code cases across all eleven cars. Original terrain contact, suspension resolution and final world integration remain pending.
- Optional desktop prototype: keyboard driving, original track/terrain/scenery meshes and Indy polygons, chase camera, minimap, experimental suspension against scene triangles and swept wall collisions.
- Original slope-road substitution validated against all 65,536 byte input pairs; scene metadata extracted directly from the unchanged executable.
- Native control traces checked against 6600 sequential frames of original DOS function execution.
- Native main/pause menus with all 41 tracks and 11 cars, track preview, controls and race restart. Opening a menu freezes simulation time.
- **50 tests with the desktop feature**: 45 library tests plus five desktop tests, including original-code vectors, menu navigation, pause/resume timing, uphill suspension, wall/bridge contacts and rendering.

## Run

To build and play the experimental Windows prototype from the repository root:

```powershell
.\play.ps1
# Or, with a normal Rust installation:
cargo run --release --manifest-path stunts-rs/Cargo.toml --features desktop --bin stunts-play -- --data stunts_dos
```

The game opens at its main menu. Use **Up/Down** to select an option, **Left/Right** to change the track/car and **Enter** to confirm. Driving: **W/Up** accelerate, **S/Down** brake, **A/Left** and **D/Right** steer, **R** reset, **Escape** pause. The pause menu offers resume, restart, main menu and exit. The Windows executable is `stunts-rs/target/release/stunts-play.exe`. `--track NAME.TRK` and `--car CARXXXX.RES` set the initial menu choices; `--quick-start` bypasses the menu. See [prototype status and validation](docs/desktop-prototype.md) for the current limits.

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

The output file must not already exist. This CLI command runs control calculations without position integration, suspension, collisions or graphics. Values are raw game quantities, not real-world telemetry. Default library/CLI builds have no third-party runtime dependencies; the optional desktop feature uses minifb. Optional Python/Unicorn tools regenerate development fixtures.

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

Next major work: remaining `update_player_state` physics, original terrain/contact planes, authentic materials and special rendering rules, full replay world-state comparison, opponents and audio. The experimental solver must not be treated as evidence that those original routines are ported.

## References

- [ReStunts](https://github.com/4d-stunts/restunts)
- [ReStunts2](https://github.com/dstien/restunts2)

The executables and resources in `stunts_dos/` belong to their respective rights holders. Their inclusion does not change ownership of these files.
