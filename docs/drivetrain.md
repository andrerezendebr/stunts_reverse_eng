# Gearbox, torque selection, and RPM mapping

Analysis date: 2026-11-26. This continues [steering and car-parameter mapping](steering-car-parameters.md) on the same reconstructed MCGA image. `reverse/` remains ignored by Git; the original binary, Ghidra database, local tools, and caches are not added to the repository. Scripts, Rust source, test vectors, and text inventories are versioned separately.

## Verified scope

Three bounded stages are now ported:

| Stage | Original code | Rust | Validation |
| --- | --- | --- | ---: |
| Gear selection and lever motion | `1471:3368` to just before `1471:359C` | `drivetrain::gearbox_tick` | 704 original-code cases |
| Grounded torque selection | `1471:36D6` to just before `1471:3724` | `Car::selected_torque` | 1540 original-code cases |
| Complete RPM helper | `19DC:0006` | `drivetrain::rpm_from_speed` | 520 original-code cases |

The gearbox and torque regions are internal parts of `update_car_speed`, not separate DOS functions. The helper for RPM is a complete function. The next milestone now implements the [complete speed/grip kernels](speed-grip-numerics.md); a drivable car remains pending.

The executable is loaded at segment `1000`, so the corresponding linear CPU boundaries are `17A78`, `17CAC`, `17DE6`, `17E34`, and `19DC6`. `MapDrivetrain.java` marks the internal stages without splitting them into invented functions and defines a partial packed `/Stunts/CarRuntimeState` type. `TraceSimulation.java` checks the RPM helper's complete 34-byte body and sets its five unsigned-word parameters and unsigned-word result in Ghidra.

## Packed state fields

`CARSTATE` is 208 bytes. The Rust staged port stores the entire block and changes only the verified fields, allowing tests to detect accidental writes to unmapped data.

| Offset | Width | Field |
| --- | ---: | --- |
| `0x22` | 2 | Current RPM |
| `0x24` | 2 | Previous RPM |
| `0x28` | 2 | Speed difference |
| `0x2C` | 2 | Speed2 |
| `0x2E` | 2 | Previous speed |
| `0x30` | 2 | Active gear ratio |
| `0x32` | 2 | Gear ratio shifted right by eight |
| `0x34`, `0x38` | 2 each | Current lever X/Y |
| `0x3A`, `0x3C` | 2 each | Target lever X/Y |
| `0xBE` | 1 | Selected gear |
| `0xC0` | 1 | Rear-wheel contact sum |
| `0xC6` | 1 | Engine-limiter timer |
| `0xCA` | 1 | Gear-change flag |
| `0xCB` | 1 | Shift timer |
| `0xCC` | 1 | Transmission mode |

Offsets and widths come from the original instructions and reference `asmorig/structs.inc`. Signedness follows each instruction rather than a blanket signed interpretation of all words.

## Gear selection and motion

At the start of the prefix, the engine-limiter byte decrements if nonzero. Speed history is updated with wrapping word subtraction, and current RPM is copied to previous RPM.

With transmission mode zero and no active gear change, bits `0x10` and `0x20` request upshift and downshift. Upshift has precedence when both bits are present, even if already in the top gear; the code does not then fall through to downshift. Upshifts can leave neutral (gear zero). Downshifts stop at gear one.

The automatic branch requires a nonzero gear, no active change, and nonzero rear-wheel contact. RPM must be strictly above the upshift threshold or strictly below the downshift threshold. Equality does not shift. A gear change sets a new selected gear and target lever position but retains the previous active ratio.

The lever follows an H-pattern path: first move vertically to neutral Y, then horizontally toward target X, then vertically toward target Y. Movement is six coordinate units at 20 FPS and twelve otherwise. Signed word subtraction and the original ABS behavior are retained. The active ratio changes only on a tick that begins with the lever already at its target; reaching the target during movement does not commit the ratio until the next tick.

The shift timer is initialized from the low FPS byte using the original sign-extension, shift, and byte-add behavior. At normal rates this is 30 at 20 FPS or 15 at 10 FPS. It decrements only on ticks that enter the non-changing branch. It is not itself the condition that finishes lever motion.

Rust validates gear-table bounds before mutation. It does not emulate out-of-range gear IDs reading unrelated memory.

## Torque selection

The isolated grounded-car block selects idle torque when gear is zero or one and the signed RPM word is below 2600. Otherwise it reads the unsigned byte at `torque_curve[rpm >> 7]`. There is no interpolation. A nonzero limiter timer and signed RPM below 5000 replace the chosen torque with the integer average of that value and idle torque.

The resource curve has 104 samples. Rust rejects a selected curve index beyond those samples instead of reading the next resource field. It keeps the original signed comparisons and unsigned shift where applicable.

This block alone does not apply the caller's max-RPM check, airborne branch, changing-gear branch, aerodynamic contribution, ratio/mass scaling, or opponent adjustments. In the original function, the limiter timer is decremented in the earlier gearbox prefix before this torque block is reached. A standalone torque query takes the timer value at the block entry.

## RPM helper

If a gear change is active, retain the supplied current RPM. Otherwise compute the high word of the unsigned product `speed * gear_ratio`, equivalent to `(u32(speed) * u32(ratio)) >> 16`. Both paths then enforce an unsigned lower bound of `idle_rpm`.

This is integer fixed-width behavior. Signed multiplication, a low-word product, or floating-point conversion would give different results for some inputs. The 520 execution vectors cover boundary speeds/ratios, active/inactive changes, idle bounds, and deterministic random full-range words.

## CPU-execution evidence

`tools/verify_drivetrain.py` verifies the known executable hash, relocates the MZ image, and executes its original instructions in Unicorn. It does not substitute Rust or Python implementations for called DOS helpers.

- Gearbox: run the original function from its entry to `17CAC`, then compare all 208 car-state bytes. Cases cover every original car, 10/20 FPS, manual/automatic modes, simultaneous request bits, rear-contact states, and several lever-motion phases.
- Torque: establish the original function's BP frame and inherited `BX=carState`, then run only `17DE6..17E34`. Cases cover threshold boundaries, limiter states, all 104 curve indices for each car, and idle-torque selection.
- RPM: execute the full helper through its far return and check restored BP/SP and the return address.

The committed fixtures are `gearbox-dos.csv`, `torque-dos.csv`, and `rpm-dos.csv` under `stunts-rs/tests/fixtures/`. They contain expected results from the original CPU execution. Rust tests do not require Unicorn or the ignored `reverse/` directory to read these fixtures. Tests still use the original game files already committed under `stunts_dos/`.

## Commands

```powershell
cargo run --manifest-path stunts-rs/Cargo.toml -- rpm 1000 20000 32000 0 2000
cargo run --manifest-path stunts-rs/Cargo.toml -- torque stunts_dos/CARPMIN.RES 2 5000 0
cargo run --manifest-path stunts-rs/Cargo.toml -- gear-tick stunts_dos/CARPMIN.RES 1 9000 16 20
cargo test --manifest-path stunts-rs/Cargo.toml
```

The first two examples return `9765` and `33`. `gear-tick` initializes a stationary manual-transmission state at the supplied gear and performs only the gearbox prefix. In this example it selects gear two, starts lever motion, and retains ratio 32000. It is an analysis demonstration, not a complete vehicle tick.

To regenerate fixtures with Python and Unicorn, choose a fresh output directory:

```powershell
python tools/verify_drivetrain.py reverse/decoded/MCGA.exe stunts_dos reverse/new-drivetrain-vectors
```

To reproduce the Ghidra annotations and exports:

```powershell
analyzeHeadless reverse/projects Stunts -process MCGA.exe -noanalysis -scriptPath tools/ghidra -postScript MapDrivetrain.java -postScript TraceSimulation.java reverse/exports/simulation -postScript ExportAnalysis.java reverse/exports
```

This earlier milestone had 25 passing tests; the current suite is documented in speed-grip-numerics.md. `reverse/` remains excluded from Git.

## Follow-up speed-update work

The next section applies aerodynamic drag, gravity, torque/ratio/mass scaling, braking, airborne behavior, wheel-speed reconciliation, and opponent adjustments before calling the RPM helper. Some details differ from a straightforward C rewrite: torque multiplication uses the low 16-bit result before shifting, and mass scaling calls an unsigned-long division helper. These observations are now covered by the validated complete port in [speed/grip kernels](speed-grip-numerics.md). Complete grip is now ported as well; suspension, collisions and full replay world-state comparison remain pending.
