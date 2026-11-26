# Tracks, replays, and the first simulation routine

Analysis date: 2026-11-26. This continues the [initial executable reconstruction](reverse-engineering.md) using the same MCGA SHA256 and ReStunts reference commit.

## Track layout

All 41 `.TRK` files in this copy are 1802 bytes and can be read by `src/track.rs`.

| Offset | Bytes | Interpretation |
| --- | ---: | --- |
| `0x0000` | 900 | Element IDs, 30 rows of 30 cells, top to bottom |
| `0x0384` | 1 | Scenery: 0 desert, 1 tropical, 2 alpine, 3 city, 4 country |
| `0x0385` | 900 | Terrain IDs, stored bottom to top |
| `0x0709` | 1 | Unverified final byte, preserved unchanged |

The parser presents both layers in display coordinates: `(0, 0)` is the top-left cell. Element index is `y * 30 + x`; terrain index is `901 + (29 - y) * 30 + x`. Layer reversal is essential: reading both arrays in their storage order would place terrain under the wrong elements. The reference `docs/format.html` describes these opposing orders, and `stunts_main` initializes corresponding row tables in the executable.

Unknown element, terrain, and scenery values are preserved. The parser checks structure, not track legality, connectivity, or drivability. Occupied element cells are not necessarily individual track pieces because pieces can span multiple cells.

## Replay layout

`DEFAULT.RPL` is 12474 bytes:

| Offset | Bytes | Interpretation |
| --- | ---: | --- |
| `0x0000` | 4 | Player car ID |
| `0x0004` | 1 | Player material |
| `0x0005` | 1 | Player transmission |
| `0x0006` | 1 | Opponent type |
| `0x0007` | 4 | Opponent car ID |
| `0x000B` | 1 | Opponent material |
| `0x000C` | 1 | Opponent transmission |
| `0x000D` | 9 | Track name buffer |
| `0x0016` | 2 | Frames per second, little-endian |
| `0x0018` | 2 | Recorded frame count, little-endian |
| `0x001A` | 1802 | Complete embedded track |
| `0x0724` | frame count | One input byte per frame |

Evidence combines packed `GAMEINFO` in reference `c/externs.h`, track/replay buffer allocation in `c/restunts.c`, and the original `file_write_replay` assembly. That assembly adds `0x724` to the frame count when writing the replay. The reference C rewrite in `fileio.c` instead uses `sizeof(GAMEINFO) + frames`, omitting the track from its size expression; the assembly and actual file layout take precedence here.

The default replay has player ID `PMIN`, no enabled opponent (`type=0`), 20 FPS, and 10646 commands, corresponding to 532.3 seconds of recorded frame intervals. Its embedded track matches `HELL5.TRK` byte for byte. The nine-byte track name buffer is `HELL5\0MP\0`: bytes after the first NUL are retained by the parser but omitted from display. The unused opponent ID contains a non-ASCII leading byte; it is not assumed to name a valid car.

The reader currently accepts this exact replay layout and rejects extra or missing command bytes and zero FPS. Older variants or extended formats may need separate parsers.

## Recorded controls

Masks checked in `player_op` and `update_car_speed`:

| Bits | Meaning |
| --- | --- |
| 0–1 | Throttle code: 1 selects acceleration; 2 selects braking; other values remain raw |
| 2–3 | Steering code passed to `upd_statef20_from_steer_input`; direction labels are not assigned yet |
| 4 | Upshift request |
| 5 | Downshift request |
| 6–7 | Unverified bits, preserved |

These are requests, not guaranteed resulting actions. For example, transmission state and gear-change state gate shifting. The tool exports requests without pretending to apply the car simulation. CSV timestamps are frame indices divided by FPS, not samples of independently measured wall-clock time.

## Confirmed execution path in Ghidra

`TraceSimulation.java` checks the exact executable hash and each function's opening instruction bytes before assigning reference-derived names. Prefix checks avoid relocated segment words. The exported [call inventory](analysis/simulation-calls.tsv) records static calls discovered by Ghidra; indirect calls and incorrectly inferred boundaries may need additional review.

| Ghidra address | Name | Evidence / role |
| --- | --- | --- |
| `21B7:000A` | `run_game` | Matches the reference prologue and calls `track_setup` and `update_gamestate` |
| `1471:28F8` | `update_gamestate` | Reads the frame's input byte, increments the frame counter, and dispatches to `player_op` |
| `1471:2AD8` | `player_op` | Calls speed handling, steering response, grip, and player-state updates |
| `1471:3368` | `update_car_speed` | Handles engine/gear/speed logic; its prologue branches on 20 FPS |
| `1E1A:2534` | `track_setup` | Allocates track compilation data and checks track elements/terrain |
| `2EA2:1624` | `multiply_and_scale` | Fully checked 26-byte arithmetic leaf routine, now ported to Rust |

The decompiled `update_gamestate` reads the frame index from DS:`8E64` and the replay-input far pointer from DS:`9EBE`. It reads the input before incrementing the frame counter. Periodic state snapshots and random-seed handling occur before that increment. Nonzero input enables the active-input mode, after which `player_op` is called and optional opponent processing follows. These observations establish control flow, not a complete executable Rust simulation.

Targeted pseudocode is saved locally under `reverse/exports/simulation/`. Inferred types and most function signatures still need review. The two signed arguments and signed return value of `multiply_and_scale` have been explicitly set in Ghidra because its BP-relative argument loads and AX return are verified.

## First numerical port

`src/fixed.rs` implements `multiply_and_scale(a: i16, b: i16) -> i16`. The original sequence is:

```text
IMUL signed word     ; DX:AX = a * b
SHL AX,1 / RCL DX,1  ; shift the product left twice
SHL AX,1 / RCL DX,1
SHL AX,1 / ADC DX,0  ; use the next discarded bit for rounding
MOV AX,DX            ; return the resulting word
```

It computes the signed fixed-point product with 14 fractional bits, rounds ties toward positive infinity, and wraps the 16-bit return. Floating-point arithmetic, truncation toward zero, and saturation would change the behavior. Examples: `(1, 8192)` returns `1`, `(-1, 8192)` returns `0`, and `(-32768, -32768)` wraps to `0`.

Validation has two independent layers:

- 249 expected results generated by executing the original DOS machine-code bytes in the Unicorn 16-bit x86 emulator, including boundary pairs and deterministic random inputs. These vectors are committed as `stunts-rs/tests/fixtures/fixed-mul-dos.csv`.
- 655360 comparisons against an independent wide-integer expression, covering every signed 16-bit first operand with ten representative second operands.

`tools/verify_fixed_mul.py` checks the original executable hash and instruction bytes, models the far-call stack, runs the routine through its final `POP BP`, and records AX before `RETF`. It also checks restored BP/SP. It does not execute the full game. Rust tests consume the committed vectors without requiring Python or Unicorn.

## Commands

From the repository root, with Cargo on PATH:

```powershell
cargo run --manifest-path stunts-rs/Cargo.toml -- track stunts_dos/HELL5.TRK
cargo run --manifest-path stunts-rs/Cargo.toml -- replay stunts_dos/DEFAULT.RPL
cargo run --manifest-path stunts-rs/Cargo.toml -- replay-inputs stunts_dos/DEFAULT.RPL reverse/decoded/DEFAULT-inputs.csv
cargo run --manifest-path stunts-rs/Cargo.toml -- fixed-mul -1 8192
cargo test --manifest-path stunts-rs/Cargo.toml
```

The CSV destination must not already exist. To regenerate emulation vectors with Python and the `unicorn` package installed, choose a fresh path:

```powershell
python tools/verify_fixed_mul.py reverse/decoded/MCGA.exe reverse/decoded/fixed-mul-dos.csv
```

To reproduce the Ghidra names and targeted exports on the reconstructed project:

```powershell
analyzeHeadless reverse/projects Stunts -process MCGA.exe -noanalysis -scriptPath tools/ghidra -postScript TraceSimulation.java reverse/exports/simulation -postScript ExportAnalysis.java reverse/exports
```

This milestone had 14 passing tests, including the 41 original tracks, the replay layout, the 79 compressed resources, all 256 control-byte decompositions, malformed inputs, and the arithmetic checks above.

## Remaining work

The next milestone now maps [steering-response tables and car simulation resources](steering-car-parameters.md), with steering and aero-table ports validated against original CPU execution. Complete speed and grip have since been ported; full world physics, suspension, collisions, opponent behavior, rendering, sound and complete replay world-state comparison remain unimplemented.
