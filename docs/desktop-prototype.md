# Experimental desktop driving and original wheel prediction

The `desktop` feature builds `stunts-play`, a native Windows application using
minifb 0.29 and a CPU framebuffer. It runs Rust directly; DOSBox, Ghidra, Python
and the reconstructed DOS executable are not required at runtime.

## Play

Run `./play.ps1` from PowerShell, or build with:

```powershell
cargo build --release --manifest-path stunts-rs/Cargo.toml --features desktop --bin stunts-play
.\stunts-rs\target\release\stunts-play.exe --data stunts_dos --track HELL5.TRK
```

W/Up accelerates, S/Down brakes, A/Left and D/Right steer, R resets the car, and
Escape exits. Automatic shifting uses the verified drivetrain. Telemetry appears
in the window title; the display includes a minimap, chase camera and speed bar.
The optional launcher uses the existing ignored local toolchain if Cargo is not
on PATH. Original game files are opened for reading only.

## Implemented behavior and limits

`prototype.rs` calls the verified speed, steering and detailed grip kernels at
20 Hz. Its world motion is an explicitly new floating-point bicycle model;
speed-to-world conversion, wheelbase and steering calibration are provisional.
Four spring/damper contacts support the car on a flat plane and body pitch/roll
follow approximate load transfer. Crossing the enclosing map boundary clamps
position and stops the car. The prototype has no collisions with original
scenery, opponents, walls or track obstacles. It does not yet implement original
jumps, bridges, banked roads, loops, damage, reverse driving, timing or sound.
Original terrain height data is parsed by the library but not used by this solver.

Rendering uses a depth buffer and perspective projection. It reads the original
Indy `STPMIN.P3S` archive, parses `car0` vertex/primitive records and draws its
polygons. Wheel primitives are provisional defining faces rather than the
original specialized wheel rasterizer. Colors are an arbitrary prototype
palette; primitive visibility masks, original paint tables and near-plane
polygon clipping are not reproduced. Polygons crossing the near plane are
discarded. Track elements are flat cross-shaped schematic patches, including
unsupported objects; the scene is not an authentic rendering of the track.
Ground-surface classification is likewise coarse. Missing start tile 0x01 falls
back to the center of the map; other original start orientations remain unmapped.

This prototype makes movement visible while the remaining DOS physics is being
mapped. Passing its tests does not validate it against original game replays.

## New original-code mapping

`motion::predict_wheels` extends the previously validated prefix of
`update_player_state` from physical address `14ECE` through `1513E` (exclusive).
It performs:

- Original upside-down contact adjustment and wheel attachment displacement
  using `SIMD.wheel_coords` and `CARSTATE.car_rc2`.
- Skid rotation through `mat_rot_zxy(0,0,-angle_z,0)` and body rotation.
- Original per-wheel travel; front wheels subtract the signed quarter steering
  angle. The initial `plane_rotate_op` branch uses `planindex_copy = -1`.
- Signed word narrowing and wrapping world-coordinate addition.

Returned base/predicted wheel arrays are intermediate stack values. The function
only commits the original preparation writes to car state; it does not perform
terrain contact, commit final wheel coordinates or move the body. It is kept
separate from the experimental desktop solver, which does not consume these
raw fixed-point predictions yet.

The Ghidra `MapMotion.java` script marks boundary `1471:0A2E` and documents the
stack arrays at BP-448 and BP-374, plus the adjustment at BP-240. The local
Ghidra database remains under ignored `reverse/`.

`tools/verify_wheels.py` executes the unchanged reconstructed DOS code in Unicorn,
with SS=DS and initialized quadrant rotation matrices. Its committed fixture
contains **704 cases**: 64 per original car, both frame rates, zero/nonzero speed,
contact and inverted-orientation cases. Tests compare every state byte, all 24
base/predicted coordinate components and the upside-down adjustment.

## Verification

```powershell
cargo test --manifest-path stunts-rs/Cargo.toml --features desktop
stunts-rs/target/release/stunts-play.exe --headless --frames 200 --screenshot reverse/drive.ppm
stunts-rs/target/release/stunts-play.exe --frames 10
```

Headless mode requires a finite frame count and applies acceleration at each
20 Hz step. Windowed `--frames` counts presentation frames and uses real elapsed
time for physics. Screenshot files use binary PPM and must not already exist.
Screenshots, executables, downloaded dependencies and Ghidra databases remain
outside Git. `Cargo.lock` records the optional desktop dependency graph.
