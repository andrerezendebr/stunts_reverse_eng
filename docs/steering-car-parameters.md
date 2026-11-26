# Steering, resource archives, and car parameters

Analysis date: 2026-11-26. This continues [the simulation mapping](track-replay-simulation.md) on the same reconstructed MCGA image and ReStunts reference commit.

## New verified code paths

| Ghidra address | Name | Result |
| --- | --- | --- |
| `1471:40A2` | `update_steering` | Full steering-angle update ported to Rust and compared with original CPU execution |
| `1471:5592` | `setup_aero_trackdata` | Resource lookup, SIMD copy, pointer replacement, and aero-table generation traced |
| `274B:15DC` | `locate_shape_alt` | Wrapper accepting a resource far pointer and a four-byte resource identifier |

The reference name for the steering routine is `upd_statef20_from_steer_input`. Its original code occupies 241 contiguous bytes, including padding; Ghidra's function body contains 240 bytes because its inferred body excludes a padding byte. Names are guarded by the binary hash and checked instruction prefixes in `TraceSimulation.java`.

`MapCarData.java` verifies both response tables, adds labels for confirmed state fields, and defines the packed `/Stunts/CarSimulationParameters` structure in the data-type manager. It does not recast every runtime buffer or force inferred code into data. Most of the car simulation remains unported.

## Steering tables and state

| Address | Data |
| --- | --- |
| `3B77:060A` | 64 signed-byte response entries for the 20-FPS path |
| `3B77:064A` | 64 signed-byte response entries for the 10-FPS path |
| `3B77:8E96` | Player steering angle, signed word |
| `3B77:8EA2` | Player speed2, read as an unsigned word by steering |
| `3B77:9260` | Frames per second |
| `3B77:9278` | Near pointer selecting the steering-response table |

The original index is `((speed2 >> 10) & 0xFC) + input`. Equivalently, the unsigned 16-bit speed selects one of 16 buckets using `speed2 >> 12`, and the two-bit input selects a column. Each row is `[0, +response, -response, 0]`.

| Speed bucket | 20-FPS response | 10-FPS response |
| ---: | ---: | ---: |
| 0 | 8 | 16 |
| 1 | 7 | 14 |
| 2 | 6 | 12 |
| 3 | 5 | 10 |
| 4–5 | 4 | 8 |
| 6–7 | 3 | 6 |
| 8–10 | 2 | 4 |
| 11–12 | 1 | 2 |
| 13–15 | 1 | 1 |

The 10-FPS table is not simply twice the 20-FPS table in the last three buckets. The full signed table export is [steering-tables.csv](analysis/steering-tables.csv).

One update follows these checked branches:

1. Inputs 1 and 2 add and subtract the selected response. Direction labels such as left/right remain unassigned until the world-angle convention is checked.
2. If the response opposes an existing angle beyond ±1, multiply that delta by four.
3. A zero response recenters a moving car using twice the positive response for its speed bucket. A stopped car skips that branch.
4. Limit the delta to ±160 at 10 FPS or ±80 otherwise. Add it with word wrapping and clamp the resulting angle to ±240.
5. With a zero table entry, snap a resulting magnitude below 8 to zero, including when stopped.

The original initialization and update paths select the 10-FPS behavior only when FPS equals 10; other values use the 20-FPS path. The Rust routine preserves that fallback. It rejects input codes above 3 instead of reading unrelated memory. Angle and speed inputs remain raw game quantities; no degrees or physical speed units are assumed.

### Independent validation

`tools/verify_steering.py` relocates the full original MZ load image to segment `1000`, sets the original DS-relative inputs and table pointer, and executes the original function in Unicorn. The routine's call to the original absolute-value helper also executes; it is not replaced by a Python implementation. The script checks the far-return sentinel and restored stack frame.

The committed `steering-dos.csv` fixture contains 1488 cases: all four input codes across selected speed-bucket boundaries and signed-angle boundaries at 10 and 20 FPS, plus 128 deterministic random cases including the non-10/20 fallback. Every result matches `src/steering.rs`, including signed wrapping and the stopped-car dead zone. This validates one steering tick, not the complete steering-to-wheel-force simulation.

## Resource archive directory

`src/resource.rs` reads the uncompressed archive used by `.RES` files and decoded resource files:

| Offset | Bytes | Meaning |
| --- | ---: | --- |
| 0 | 4 | Total file size, little-endian |
| 4 | 2 | Resource count N, little-endian |
| 6 | 4N | Four-byte identifiers |
| 6 + 4N | 4N | Little-endian offsets relative to the payload |
| 6 + 8N | remainder | Resource payload |

This layout is confirmed by `locate_resource` in the reference and by executing the original lookup through `setup_aero_trackdata`. Name order is not payload order: in car files `simd` is listed after text IDs but stored at payload offset zero. Resource extents are inferred from the next greater offset; aliases can share an offset, and inferred extents may contain padding. The parser preserves directory order, implements exact four-byte first-match lookup, and validates sizes/offsets. The original helper's additional short-name padding rules are not implemented.

Tests cover all 13 plain `.RES` archives and all 71 decoded `PRE`, `PVS`, `P3S`, and `PES` archives. Combined with the decompressor, this exposes the resources within these files rather than treating them as opaque blobs.

## Car simulation block

All eleven `CAR*.RES` archives contain a `simd` resource of `0x308` (776) bytes. The original routine copies `0x184` words into the selected runtime car configuration and replaces its final far pointer. The reader retains the raw block and exposes only confirmed fields.

| SIMD offset | Bytes | Reference field |
| --- | ---: | --- |
| `0x000` | 1 | Number of gears |
| `0x002` | 2 | Mass, raw |
| `0x004` | 2 | Braking efficiency, raw |
| `0x006` | 2 | Idle RPM field |
| `0x008` | 2 | Downshift RPM field |
| `0x00A` | 2 | Upshift RPM field |
| `0x00C` | 2 | Maximum RPM field |
| `0x00E` | 14 | Seven unsigned gear ratios, including index zero |
| `0x01C` | 28 | Seven gear-knob coordinate pairs |
| `0x038` | 2 | Aerodynamic resistance, raw |
| `0x03A` | 1 | Idle torque, raw |
| `0x03B` | 104 | Torque samples, retained as raw bytes |
| `0x0A4` | 2 | Grip, raw |
| `0x0B4` | 2 | Sliding field, raw |
| `0x0B6` | 8 | Four surface-grip words |
| `0x0C8` | 8 | Collision point pairs |
| `0x0D0` | 2 | Car height, raw |
| `0x0D2` | 24 | Four wheel coordinate triplets |
| `0x304` | 4 | Far pointer overwritten at runtime with the allocated aero table |

Other ranges include unverified fields and dashboard data. They are not discarded or assigned invented meanings. Reference field names do not establish physical units; for example the Indy car's raw mass is 15, not a claim of 15 kg. The torque sample-to-RPM indexing and force scaling are still being traced.

The [car parameter inventory](analysis/car-parameters.csv) records the primary fields for all eleven cars. Example: `CARPMIN.RES` is named `Porsche/March INDY`, has five gears, raw aero resistance 25, and raw grip 600.

### Aero-table generation and validation

The checked setup routine builds 64 signed words using `(aero_resistance * index * index) >> 9`, with signed 32-bit multiplication followed by a signed shift and a 16-bit store. Rust implements that table generation with explicit wrapping/narrowing.

Verified runtime fields:

- Player SIMD destination: `3B77:A6EA`; player aero-table far pointer: `3B77:95CE`.
- Opponent SIMD destination: `3B77:9ED0`; opponent aero-table far pointer: `3B77:95DA`.

`tools/verify_car_aero.py` executes the original setup function for both sides of all eleven cars. Its real resource lookup, 776-byte copy, long-multiply helpers, name lookup, and string-copy helpers execute in the relocated DOS image. For each run it compares the first 772 copied bytes with the original `simd` resource; the final four bytes are the intentionally replaced runtime pointer. All 1408 generated table values match Rust and are committed in `car-aero-dos.csv`.

## Run the new tools

```powershell
cargo run --manifest-path stunts-rs/Cargo.toml -- resources stunts_dos/CARPMIN.RES
cargo run --manifest-path stunts-rs/Cargo.toml -- car stunts_dos/CARPMIN.RES
cargo run --manifest-path stunts-rs/Cargo.toml -- steer -100 0 1 20
cargo test --manifest-path stunts-rs/Cargo.toml
```

`resources` expects an uncompressed archive. Use the existing `decompress` command first for packed resources. The `steer` example returns `-68`; it updates only the steering-angle value.

To regenerate the CPU-execution fixtures with Python and Unicorn, use fresh output paths:

```powershell
python tools/verify_steering.py reverse/decoded/MCGA.exe reverse/decoded/steering-dos.csv
python tools/verify_car_aero.py reverse/decoded/MCGA.exe stunts_dos reverse/decoded/car-aero-dos.csv
```

To reproduce the Ghidra map:

```powershell
analyzeHeadless reverse/projects Stunts -process MCGA.exe -noanalysis -scriptPath tools/ghidra -postScript MapCarData.java reverse/exports/simulation -postScript TraceSimulation.java reverse/exports/simulation -postScript ExportAnalysis.java reverse/exports
```

This milestone had 21 passing Rust tests; see speed-grip-numerics.md for the current suite. No Python or Unicorn dependency is needed to run them because they consume the committed fixtures.

## Next mapping targets

Torque lookup, gearbox stages, and the RPM helper are now covered by [drivetrain mapping](drivetrain.md). Complete speed integration and grip are now validated in [speed/grip kernels](speed-grip-numerics.md); the remaining wheel/suspension and world integration still need porting. These routines do not yet form a runnable car simulation or reproduce an entire replay.
