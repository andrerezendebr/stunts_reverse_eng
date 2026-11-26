# Initial reverse engineering results

Analysis date: 2026-11-26. All results below refer to the files in `stunts_dos/`.

## What now works

- A Rust decoder handles the resource RLE and variable-length encoding formats, including nested passes and additive VLE.
- All 79 compressed resources with extensions `CMN`, `COD`, `DIF`, `PRE`, `PVS`, `P3S`, and `PES` decode successfully.
- A Rust combiner builds packed DOS executables for MCGA, CGA, Tandy (`TDY`), and EGA.
- The MCGA executable has been unpacked with the external EXEPACK 1.4.0 tool and imported into Ghidra 12.2.
- The first startup function names have been checked against the reference disassembly.

This is preparation for a native Rust port. The tools do not implement a playable game, and the reconstructed DOS executable has not been run in an emulator.

## Evidence and reference version

Reference repository: [4d-stunts/restunts](https://github.com/4d-stunts/restunts), commit `5c38f258f482e7d28f1ccce73da8992dccbbee89`.

The following decoded modules match the reference combiner assets byte for byte, verified using SHA256:

| Module | Compressed bytes | Decoded bytes | Decoded SHA256 |
| --- | ---: | ---: | --- |
| EGA.CMN | 127337 | 143104 | `CB61434E05EEE9D4B8A107892D36DCBF23E23858A57F093720E95FD49E22DAA6` |
| MCGA.COD | 48899 | 55509 | `72242C68E3F031A7B9FF6FFBB2818621FAE37325D0F8A35216647A68AD3C9457` |
| MCGA.DIF | 17958 | 23062 | `E7B9CFDE97FD0963C74E059EE70543C84338EE666CFF041C83B5E328A63589C2` |

The local `MCGA.HDR` also matches the reference header. The reconstructed load image is 200000 bytes. Comparing it against the reference `stunts/game.exe` load image gives exactly one changed byte: image offset `0x02AC` is `00` locally and `01` in the reference. The reference file has a 10384-byte header, so this is its file offset `0x2B3C`, documented by its combiner as a protection-related patch. This project keeps the original `00` byte. Different EXEPACK tools produce different MZ header sizes, so file offsets must not be confused with load-image offsets.

These matches establish compatibility with the reference Stunts 1.1 MCGA code image; they do not independently establish every original distribution file or its release history.

## Compression format

The format was investigated using the local byte streams and the reference `src/restunts/c/fileio.c` routines.

- Every pass begins with a type byte and a 24-bit little-endian decoded size.
- Type `1` is RLE. Its subheader contains a 24-bit source length, a reserved byte, an escape-table length/flags byte, and escape bytes. Bit 7 skips the sequence-expansion stage.
- In the sequence stage, escape-table entry 1 delimits a repeated sequence, followed by its repetition count.
- The single-byte stage uses the escape byte's one-based table index. Index 1 reads an 8-bit count; index 3 reads a 16-bit little-endian count; other indices imply `index - 1` repetitions.
- Duplicate escape bytes are valid in real files: `SDGAME2.PVS` requires the final table entry to take precedence.
- Type `2` describes canonical prefix codes using counts per code width followed by the alphabet. Bits are consumed most significant first. Bit 7 of the width-count byte enables cumulative, wrapping 8-bit output.
- An outer header with bit 7 set specifies the pass count in its low seven bits and the final size in the next three bytes.

The Rust implementation bounds output growth, checks input access, and rejects unknown compression types and invalid code tables. It deliberately caps processing at 16 passes and 16 MiB per intermediate buffer. Generated outputs use exclusive creation to avoid overwriting existing files.

## Executable reconstruction

The reference `src/execombiner/main.cpp` establishes the module order:

1. Read the 30-byte video `.HDR` and derive the MZ file and header sizes.
2. Decode `EGA.CMN`.
3. For MCGA/CGA/TDY, decode and apply the selected `.DIF` to the common module. EGA uses the common module directly.
4. Append the decoded video `.COD`.
5. Remove the remaining EXEPACK layer using an external unpacker.

DIF records contain a 16-bit delta and two replacement bytes, or four when bit 15 is set. A zero delta record terminates the stream. The original segmented-address arithmetic starts one byte before image offset zero and wraps at 20 bits. The Rust implementation reproduces that behavior and checks every write.

The Rust combiner initializes header padding to zero, rather than retaining the undefined padding of the historical C++ combiner.

| Video mode | Packed EXE bytes |
| --- | ---: |
| MCGA | 199125 |
| CGA | 208023 |
| TDY | 206023 |
| EGA | 220627 |

MCGA after EXEPACK 1.4.0 unpacking:

- File size: 210752 bytes; header: 10752 bytes; load image: 200000 bytes.
- SHA256: `2E2B514D183835267BDFD4EBEEB409B1C9FACAD78751EBAAFEAC54D93453E51B`.
- Original entry CS:IP: `1CC5:0012`; initial SS:SP: `3649:1F40`.
- Relocations: 2588.

## Ghidra findings

Local project: `reverse/projects/Stunts.gpr`, program `MCGA.exe`.

Automatic analysis found 573 functions and 56472 instructions. These are provisional counts: function boundaries, data/code classification, and decompiler types still need manual review. Ghidra loads this DOS image at segment `1000`, so the DOS entry appears at `2CC5:0012`.

The opening instructions of `1000:0000` match reference `ported_stuntsmain_`: calls to `init_main` and `init_div0`, followed by initialization of 30-row track/terrain tables using a stride of 30, tile coordinates spaced by 1024, and centers offset by 512. It is named `stunts_main` in the local project. Its first checked call targets are `39E1:0046` (`init_main`) and `19DC:0128` (`init_div0`). These reference-derived names do not mean their complete semantics have been verified.

`tools/ghidra/AnnotateStartup.java` checks the exact executable SHA256 before applying those names. `tools/ghidra/ExportAnalysis.java` exports a summary, function inventory, and a sample of twelve automatically decompiled functions. The sample is pseudocode for investigation, not Rust or compilable game source. Exports and the binary database remain local under `reverse/`; text inventories are also saved under `docs/analysis/`.

## Reproduce

From the repository root, with a Rust toolchain on PATH:

```powershell
cargo test --manifest-path stunts-rs/Cargo.toml
New-Item -ItemType Directory -Force reverse/decoded
cargo run --manifest-path stunts-rs/Cargo.toml -- decompress stunts_dos/EGA.CMN reverse/decoded/EGA.CMN
cargo run --manifest-path stunts-rs/Cargo.toml -- combine stunts_dos MCGA reverse/decoded/MCGA-packed.exe
exepack -d reverse/decoded/MCGA-packed.exe reverse/decoded/MCGA.exe
```

Use fresh output paths when rerunning the Rust commands. The EXEPACK unpacker is a separate tool from [its author](https://www.bamsoftware.com/software/exepack/), not the unrelated crates.io package with the same name. The source archive used here was `exepack-1.4.0.tar.gz`, SHA256 `212F3EC55822DC67763AB79534E5C9CD5FC2CF127A557A983A6B95AE4AD89ECF`.

To regenerate Ghidra exports, substitute your installed headless launcher and project paths:

```powershell
analyzeHeadless reverse/projects Stunts -import reverse/decoded/MCGA.exe -analysisTimeoutPerFile 120
analyzeHeadless reverse/projects Stunts -process MCGA.exe -noanalysis -scriptPath tools/ghidra -postScript AnnotateStartup.java -postScript ExportAnalysis.java reverse/exports
```

During this analysis an official Rust toolchain was installed only under `reverse/tools/`. To use this existing local installation in PowerShell, dot-source `. ./tools/use-local-rust.ps1`. This changes environment variables for the current shell only. To recreate it, download the official `rustup-init.exe` from `https://win.rustup.rs/x86_64`, set `RUSTUP_HOME` and `CARGO_HOME` to the corresponding `reverse/tools/rustup` and `reverse/tools/cargo` directories, and run it with `-y --no-modify-path --profile minimal --default-host x86_64-pc-windows-gnu`. The local build uses the bundled linker through `RUSTFLAGS=-C linker=rust-lld`.

## Next investigation

The next milestone now documents [track/replay structures and simulation entry points](track-replay-simulation.md), with the first numerical routine ported and checked against executed DOS code. Resource archive directories and full car simulation remain to be investigated. Graphics, input handling, audio, and full physics remain unimplemented.
