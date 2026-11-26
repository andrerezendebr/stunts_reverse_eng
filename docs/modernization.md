# Modern renderer and portability plan

The modernization is additive. The `modern` feature builds `stunts-modern` with
wgpu 27, egui/eframe 0.33.3, winit 0.30 (through eframe), glam 0.30 and bytemuck.
Those versions are pinned by Cargo.lock as a compatible group. The original
software frontend remains available as `stunts-play` with the `desktop` feature.
Default library/analysis CLI builds do not enable either graphics stack.

## Implemented foundation

- Original scene and car polygons uploaded to GPU vertex buffers. Static meshes
  are uploaded once per race; camera and car transforms update through uniforms.
- Hardware perspective projection, near-plane clipping and a depth buffer.
- Optional directional flat lighting. This is not PBR or shadow mapping yet.
- Rear chase camera using glam matrices, retaining the existing model orientation.
- GPU-rendered egui main menu, track/car selectors, track preview, controls,
  telemetry and pause/restart flow. Window events and DPI use winit via eframe.
- Shared `frontend::Catalog` and `session::Session` across both frontends.
  Resource readers, original binary assets, confirmed fixed-point kernels and
  the current experimental world solver are reused rather than rewritten.
- Vulkan, DirectX 12 and OpenGL compatibility backends enabled explicitly. Linux window support
  includes Wayland and X11. A GPU driver supporting the selected API is required.

The modern frontend still uses `prototype::World`: its body integration, spring
contacts and triangle sweeps are experimental. Rendering on a GPU does not make
those equations equivalent to the DOS game. CPU oracle tests remain the authority
for the original routines that have actually been ported.

## Run

Windows:

```powershell
.\modern.ps1
# With an installed Rust toolchain:
cargo run --release --manifest-path stunts-rs/Cargo.toml --features modern --bin stunts-modern -- --data stunts_dos
```

Linux:

```sh
sh modern.sh
# Or:
cargo run --release --manifest-path stunts-rs/Cargo.toml --features modern --bin stunts-modern -- --data stunts_dos
```

The native game uses the mouse for menus, WASD/arrows for driving, R to reset and
Escape to pause/resume. `--track NAME.TRK`, `--car CARXXXX.RES` and `--quick-start`
set startup choices. `--frames N` closes after a bounded number of UI updates.
`--smoke` starts a race and supplies acceleration for GPU checks. `--report FILE`
records the adapter/backend and test outcome; `--screenshot FILE.ppm` requests a
capture of the actual GPU-composited window before closing. Output files must
not already exist. These GUI smoke options need a windowing session, not just a
headless terminal.

The existing local Windows GNU toolchain needs `llvm-tools-preview` to generate
import libraries used by newer Windows crates. `tools/use-local-rust.ps1` exposes
LLVM's dlltool driver locally. All downloaded tools remain in ignored `reverse/`.
Normal MSVC/system Rust installations do not need that project-specific helper.

The GitHub workflow compiles and tests both frontends on Windows and Linux.
Compilation in CI and GPU/window runtime verification are separate checks;
neither alone establishes support for every driver or Linux distribution.

## Validation performed

- Windows: all 53 release tests passed with `desktop,modern` enabled.
- Ubuntu 22.04 under WSL: all 53 release tests passed, including both frontend
  suites and the original-code numerical fixtures. The native Linux executable
  was compiled and retained locally at `reverse/bin/linux/stunts-modern`.
- Windows GPU runtime: Vulkan and DirectX 12 both rendered the 22,165-triangle
  HELL5 scene successfully on the available Intel adapter. The Vulkan race and
  egui menu were captured from the actual composited window and inspected.
- Linux GUI runtime remains unverified in this environment: WSLg advertises a
  Wayland display, but its Wayland compositor and X11 sockets are absent. The
  native application reports the missing compositor during window creation.
  Running on a Linux graphical desktop is still required to verify presentation
  and the OpenGL/Vulkan driver path there. No global Ubuntu graphics settings
  were changed for the test.
- All 210 original DOS file hashes remained unchanged. Runtime captures,
  native binaries, local toolchains and Ghidra data stay outside Git.

## Remaining modernization stages

| Subsystem | Proposed implementation | Current status |
| --- | --- | --- |
| GPU graphics | wgpu + egui/eframe/winit | Implemented foundation |
| Rendering math | glam | Camera/model transforms implemented |
| Rigid bodies and scene collisions | Rapier 3D | Pending, separate modern physics mode |
| Suspension | Rapier ray queries plus spring/damper code | Pending Rapier integration |
| Tires | Brush first; Pacejka if justified by calibration data | Pending |
| PBR and dynamic shadows | Extend the wgpu pipeline, or adopt Bevy as a deliberate engine migration | Pending |
| Digital/spatial audio | Kira | Pending |
| Engine synthesis/sample pitch | Kira or FunDSP | Pending |
| Gamepad | Gilrs | Pending |
| Track editor | egui with 3D picking and original-format export | Pending |
| Parallel work | Rayon for measured asset-processing bottlenecks | Pending profiling |
| Opponent driving | Custom route planning and vehicle control | Pending |

Rapier and a tire model must be introduced as a modern dynamics option with
their own validation, while keeping original-code regression fixtures intact.
Parameters and units need calibration before claiming realistic vehicle behavior.
Bevy is an alternative whole-engine route; it is not installed alongside the
current graphics stack merely to provide a second window/input/rendering loop.
Likewise, libraries for pending stages are not added as unused dependencies.

Original DOS files remain read-only and `reverse/` remains excluded from Git.
