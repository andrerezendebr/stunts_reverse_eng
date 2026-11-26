Stunts reimplementation in Rust — getting started

This project provides resource decompression and executable reconstruction tools.
It cannot run the game yet.
It does not modify the original files and has no external dependencies.

From the stunts-rs folder:
  cargo run -- inventory ..\stunts_dos
  cargo run -- inspect ..\stunts_dos\LOAD.EXE
  cargo run -- inspect ..\stunts_dos\MCGA.HDR
  cargo test

See ../docs/reverse-engineering.md for the completed initial investigation,
the decompress/combine commands, validation results, and Ghidra instructions.

Roadmap:
1. Identify the version and reconstruct the decompressed game image.
2. Use the Ghidra project at ..\reverse\projects\Stunts.gpr to document routines.
3. Implement resource readers with bounds checking and confirmed formats.
4. Port math, car state, and physics using integer widths and overflow behavior
   that match the DOS code.
5. Compare per-frame states against original replays before integrating modern
   graphics, input, and audio.

Ghidra helps read assembly and C pseudocode. Porting to Rust will be done one
routine at a time, with behavior validation; it is not an automatic conversion.
Addresses and function names from other projects should only be used after
confirming they match this version.
