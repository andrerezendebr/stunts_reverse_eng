# Complete speed/grip kernels and numerical foundations

Analysis date: 2026-11-26. This continues [drivetrain mapping](drivetrain.md). `reverse/` remains local and excluded from Git.

The native library now ports complete `update_car_speed` and `update_grip`, exact table-based trigonometry, matrix operations, rotation builders and the six-byte random generator. A harness executes speed, steering and grip in sequence and exports CSV. Original world integration, suspension/contact resolution, collisions, opponents, complete `player_op` and audio remain unimplemented. A separate [experimental desktop prototype](desktop-prototype.md) supplies provisional world physics and graphics. These control traces are not race replays.

## Original-code validation

All expected results come from execution of the original DOS instructions in Unicorn. Rust has no DOS, Python or Unicorn runtime dependency.

| Component | New comparisons | Checked result |
| --- | ---: | --- |
| Complete speed update | 1408 | All 208 state bytes and global top speed |
| Sequential speed updates | 5720 frames | 11 cars × 2 FPS settings × 260 frames |
| Complete grip update | 1584 | All state bytes, surfaces and banked markers |
| Sine/cosine | 1152 angle pairs | Complete 1024-unit cycle and word aliases |
| Combined control kernels | 6600 frames | 11 cars × 2 FPS × 2 surfaces × 150 frames |
| Matrix operations | 768 | Vector transform, multiplication and transpose |
| Rotation builders | 768 | Original X/Y/Z matrices |
| Random generator | 1280 | Output and all six seed bytes |
| Composed rotations | 756 | Both orders, skipped axes and original Y-matrix cache |
| Motion preparation prefix | 512 | State writes and local preparation values |
| Extended wheel prediction | 704 | State, four base/predicted wheel vectors, inverted adjustment |

The suite now has **36 passing tests**, retaining previous format and component checks. Isolated cases include word boundaries, signed overflow, airborne branches, braking, shifts and opponent modifiers. Sequential comparisons keep state between calls and compare every frame.

Aero buffers in speed/control comparisons are seeded from earlier original-code fixtures. The engine, gearbox, RPM and long-arithmetic helper calls execute as real instructions. Grip comparisons also execute the original trig and fixed-point helpers, without mocks.

## Complete speed update

`drivetrain::speed_tick` ports Ghidra `1471:3368`: gearbox, aerodynamic lookup, pseudo-gravity, max-RPM handling, acceleration/braking, shifting and airborne behavior, opponent modifiers, speed limits, wheel-speed reconciliation, RPM and top-speed recording.

Important instruction details:

- Torque/ratio `MUL` discards its high word **before** shifting AX right four places.
- Mass scaling sign-extends a signed delta and multiplies by 25, then calls an **unsigned-long** divider. The quotient narrows before arithmetic right shift. Ordinary signed C division is not equivalent.
- The max-RPM branch leaves some throttle-status bytes unchanged.
- Word wrapping, signed/unsigned comparisons and the original unusual limiter branch are preserved explicitly.

Tests compare all state bytes and the separate global top-speed word. Resource errors leave caller state/top speed unchanged. Sequential traces accelerate, coast and brake; they do not integrate position.

## Complete grip update

`grip::tick` ports `1471:38C6`. The third DOS argument selects a detailed path when nonzero. The reference calls it `isOpponent`, but `player_op` passes one; Rust uses `detailed` to avoid repeating that misleading name.

The port covers contact early return, grass deceleration per wheel, demand/available grip, skid/slip angles, front-wheel angle, sliding speed loss and flags. Surface lookup starts at the car's sliding word: code zero reads it, codes one through four read following surface words, and water code five reads the next raw word. Unknown surface codes are rejected.

Banked-tile handling uses the raw element map and row table, including `FD`, `FE` and `FF` marker adjustments. Independent tests exercise each marker and tilted-tile variant. Rust rejects coordinates outside the mapped track instead of reading unrelated memory.

Raw state fields include rotations at `18/1A/1C`, pseudo-gravity `1E`, steering `20`, slip `36`, angle Z `3E`, front-wheel angle `40`, residual `42`, grip demand `44`, available grip `46`, surfaces `C2`, sliding `C7`, crash `C9`, and flags `CF`.

## Numerical routines

`trig` uses the exact 257-word quarter-wave sine table at DS:`4C84`, extracted into `src/sine_table.rs`. Angles have 1024 units per full cycle, amplitude `0x4000`. Quadrant symmetry and wrapping quarter-turn addition reproduce sine/cosine without floating point. Tests also cover every 16-bit alias of the period.

`matrix` uses column-major Q14 words. Each product shifts by 14 **before** wrapping word accumulation; it does not round like `multiply_and_scale` or sum full-width products first. Original `mat_multiply(right,left,out)` computes `left * right`. Original `mat_invert` merely transposes, so Rust names it `transpose`. Pure APIs return fresh arrays; effects of overlapping DOS input/output buffers are outside the supported contract. Tests execute the original non-aliasing functions with full-range inputs and all three axis rotation builders.

`random::KevinRandom` preserves six bytes, performs descending cumulative wrapping additions, then increments byte five with carries toward byte zero. It returns the updated byte zero. Fixtures follow each seed for multiple steps and include full carry propagation. This state is needed for future replay work, but no complete game loop is implemented yet.

## Combined harness and native executable

`simulation::control_tick` calls player-side speed, steering, and detailed grip in the order found inside `player_op`. It excludes that function's other work and `update_player_state`. Positions stay fixed. The independent oracle places the state at real global DS:`8E76`, so original steering reads exactly the state passed to speed/grip. The 44 traces cover all cars at 10/20 FPS on asphalt/ice.

The project now separates a reusable library (`src/lib.rs`) from the CLI (`src/main.rs`), without third-party runtime dependencies. From the repository root:

```powershell
cargo build --release --manifest-path stunts-rs/Cargo.toml
.\stunts-rs\target\release\stunts-rs.exe control-trace stunts_dos/CARPMIN.RES 20 150 reverse/decoded/INDY-control-native.csv
```

The directory must exist and the output file must not. The scenario accelerates straight for 50 frames, turns positively for 30, negatively for 20, coasts for 20, then brakes. Fields are raw game quantities. This is a native Windows analysis executable, not DOS or a finished game. A sample output is [indy-control-trace.csv](analysis/indy-control-trace.csv).

## Ghidra map

| Address | Function/data |
| --- | --- |
| `1471:3368` | `update_car_speed` |
| `1471:38C6` | `update_grip` |
| `2EA2:3CBE`, `2EA2:3D0C` | `sin_fast`, `cos_fast` |
| `2EA2:3ECE`, `2EA2:3FD2` | `mat_mul_vector`, `mat_multiply` |
| `2EA2:4052` | `mat_transpose` (reference `mat_invert`) |
| `36F2:000A`, `36F2:0060`, `36F2:00B8` | X/Y/Z rotation builders |
| `19DC:00BB` | `get_kevinrandom` |
| `3B77:4C84`, `3B77:A1DA` | Quarter-wave table, six-byte RNG state |
| `3B77:95D2`, `3B77:8C3C` | Element-map far pointer, row offsets |

`TraceSimulation.java` checks prefixes before naming functions. `MapDynamics.java` records validation scope and defines vector, matrix and grip-state data views. Database and binary exports remain in ignored `reverse/`.

## Reproduce oracles

With Python and Unicorn, choose fresh output paths:

```powershell
python tools/verify_speed.py reverse/decoded/MCGA.exe stunts_dos reverse/new-speed-vectors
python tools/verify_grip.py reverse/decoded/MCGA.exe stunts_dos reverse/new-grip.csv
python tools/verify_controls.py reverse/decoded/MCGA.exe stunts_dos reverse/new-controls.csv
python tools/verify_numeric.py reverse/decoded/MCGA.exe reverse/new-numeric-vectors
python tools/verify_motion.py reverse/decoded/MCGA.exe reverse/new-motion-vectors
python tools/extract_trig.py reverse/decoded/MCGA.exe reverse/new-sine-table.rs reverse/new-trig.csv
```

Ghidra:

```powershell
analyzeHeadless reverse/projects Stunts -process MCGA.exe -noanalysis -scriptPath tools/ghidra -postScript MapDrivetrain.java -postScript TraceSimulation.java reverse/exports/simulation -postScript MapDynamics.java -postScript MapMotion.java -postScript ExportAnalysis.java reverse/exports
```

The bounded preparation prefix of `update_player_state` is now ported as `motion::prepare`, stopping at `1471:07BE` (linear `14ECE`). It copies previous position and computes speed scale, front-angle quartering, composed rotations and pseudo-gravity. All state writes and the two stack-local scale values match 512 original-code cases. The oracle initializes the four predefined Y matrices by calling original rotation builders and sets SS=DS when stack-local vectors are passed through near pointers. A cold executable image alone does not satisfy these initialization preconditions. Composed rotations additionally match 756 original cases. This prefix is not called by the control harness and does not integrate position.

The extended `motion::predict_wheels` prefix now continues through `1471:0A2E` (linear `1513E`), including wheel/suspension attachment geometry and travel prediction. It matches 704 original cases; see [the detailed boundary and oracle contract](desktop-prototype.md#new-original-code-mapping).

Next major dependency: the remainder of `update_player_state`, including terrain planes, contact resolution, final world integration and collisions. Verified math helpers reduce that work but do not replace it. Full replay initialization and world-state comparisons also remain pending.
