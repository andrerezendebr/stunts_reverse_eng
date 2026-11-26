# Experimental desktop driving and original wheel prediction

The `desktop` feature builds `stunts-play`, a native Windows application using
minifb 0.29 and a CPU framebuffer. It runs Rust directly; DOSBox, Ghidra, Python
and the reconstructed DOS executable are not required at runtime.

On Windows the executable uses the GUI subsystem: launching its desktop shortcut
opens the game without a console window. Startup/runtime errors appear in a
Windows message dialog. The separate `stunts-rs` analysis CLI retains its console.

## Play

Run `./play.ps1` from PowerShell, or build with:

```powershell
cargo build --release --manifest-path stunts-rs/Cargo.toml --features desktop --bin stunts-play
.\stunts-rs\target\release\stunts-play.exe --data stunts_dos --track HELL5.TRK
```

The main menu opens first. Up/Down selects an option, Left/Right changes the
selected track/car, and Enter confirms. All 41 tracks and 11 cars are discovered
from the original resources. The menu shows the selected track's layout and car
name. Start Race loads the selected car's parameters and `car0` mesh, and the
selected track's scene. A failed race load leaves the menu open with an error.

Mouse navigation works in the main, pause and controls menus: hovering selects
an option, and a left click activates it. Track/car arrow buttons select the
previous or next entry; clicking the label advances to the next entry. Each
press activates once, even when held. A stationary pointer does not undo keyboard
selection. Unscaled client coordinates are mapped to the stretched framebuffer,
keeping hit regions aligned after resizing. The on-screen pause button also
opens the pause menu with the mouse.

W/Up accelerates, S/Down brakes, A/Left and D/Right steer, R resets the car, and
Escape opens the pause menu. Resume preserves the race; Restart resets it;
Main Menu permits another selection; Exit closes the application. Physics and
its accumulated time stop in menus, avoiding a catch-up jump on resume.
Automatic shifting uses the verified drivetrain. Telemetry appears on screen;
the display includes a minimap, rear chase camera and speed bar.
The optional launcher uses the existing ignored local toolchain if Cargo is not
on PATH. Original game files are opened for reading only.

`--track NAME.TRK` and `--car CARXXXX.RES` set the initially selected entries.
`--quick-start` starts them immediately. Headless mode also starts a race by
default, preserving the existing smoke-test workflow. Add `--menu-preview` to
render the menu without a window, for example:

```powershell
stunts-rs/target/release/stunts-play.exe --headless --menu-preview --frames 1 --screenshot reverse/menu.ppm
```

`src/bin/stunts-play/menu.rs` implements this new Rust interface and its built-in
bitmap font. It does not claim to reproduce the original DOS menu routines or
screens. Navigation and pause timing are tested separately from driving physics.

## Implemented behavior and limits

`prototype.rs` calls the verified speed, steering and detailed grip kernels at
20 Hz. Its world motion is an explicitly new floating-point bicycle model;
speed-to-world conversion, wheelbase and steering calibration are provisional.
Four spring/damper contacts sample the loaded original scene triangles. Pitch
and roll follow the sampled road slope plus approximate load transfer. Each
wheel supplies its own surface code to the verified grip kernel. The highest
support within reach is selected, so a car can stay on the lower road beneath
a bridge instead of jumping onto its deck. Contact reach grows with one tick
of travel to avoid skipping slopes at speed. A floor clamp prevents deep
penetration; this is still an approximate solver, not original suspension.

Three points spanning the car's width sweep against near-vertical mesh faces;
on impact the car returns to its previous position and speed is cleared.
The independent map boundary also clamps position. These checks reduce wall
tunnelling but do not reproduce the original collision hull or response.
Overhead and banked geometry can be displayed, but inverted loop driving,
original jump trajectories, damage, opponents, reverse driving, timing and sound
remain unimplemented. Water is not yet a distinct physical hazard. Degenerate
and specialized non-polygon scene primitives are omitted. Contact queries use
visible mesh faces rather than the original physical-model planes, and can
therefore differ from the DOS game around scenery, bridges and stunt elements.

Rendering uses a depth buffer and perspective projection. It reads the original
Indy `STPMIN.P3S` archive, parses `car0` vertex/primitive records and draws its
polygons. Wheel primitives are provisional defining faces rather than the
original specialized wheel rasterizer. The original GAME1/GAME2.P3S models now
supply track pieces, terrain and scenery. Placement uses the original object
table, quadrant rotations, overlays, paint-job indices and multi-tile origins.
Near-plane clipping keeps partially visible polygons, and a small depth
tolerance reduces coplanar road/grass flicker. Colors are an approximate palette;
original material tables, dithering, visibility masks, scenery fences and special
elevated-corner terrain rules are not fully reproduced. Missing/unsupported start
tiles fall back to the map center; plain, dirt, ice, highway and pipe start
orientations are recognized. Start position/height is provisional rather than
the original complete grid initialization.

This prototype makes movement visible while the remaining DOS physics is being
mapped. Passing its tests does not validate it against original game replays.

## New original-code mapping

`tools/extract-scene.mjs` validates the reconstructed MCGA SHA256 and extracts
116 four-byte shape IDs, 215 object records and 19 terrain records into the
3740-byte `scene_metadata.bin`. It does not copy reference source or databases.
The extracted bytes were regenerated and checked for equality.

| Original address | Data |
| --- | --- |
| `3B77:0A50` | 116 names stored at a five-byte stride |
| `3B77:2098` | 215 object records at a fourteen-byte stride |
| `3B77:2C5A` | 19 terrain records |
| `3B77:764C` | Runtime SHAPE3D array, stride 22 |
| `1E1A:38BA` | `subst_hillroad_track`, complete Rust port |

`scene::hillroad` maps compatible roads on terrain IDs 7..10 to internal IDs
B6..C5 before mesh placement. All **65,536 byte input pairs** match execution
of the original function in `tools/verify_hillroad.py`; the raw result fixture
is committed as `hillroad-dos.bin`. This is a port of original behavior, distinct
from the experimental triangle contact solver.

Original initialization sets `trackrows[i]=30*(29-i)`, `terrainrows[i]=30*i`, and
`trackcenterpos[i]=(29-i)*1024+512`. Consequently a raw file element row y has
world Z `y*1024+512`: local mesh Z must not be mirrored. Two-row pieces anchor
at raw row y's lower edge and span y-1 through y; two-column pieces span x
through x+1. The prototype's 60-unit tile gives a mesh scale of 60/1024. Plateau
height is the original 450 units before scaling. Terrain record zero is a fence
helper and must not be rendered for every blank tile. `MapScene.java` records
these tables and the verified substitution routine in the ignored Ghidra database.

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

Regression tests cover original ramp/slope heights, driving uphill at speed,
unobstructed original straight roads, bridge contact selection, swept wall
collisions, start orientation/reset, framebuffer content and near-plane clipping.
