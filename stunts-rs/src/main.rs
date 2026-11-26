//! Command-line entry point for investigating the original DOS game files.
//! Decoding and executable assembly live in separate modules so the CLI only
//! handles arguments, file access, and human-readable output.
use std::{env, error::Error, fs, io::Write, path::Path, process};
mod compression;
mod executable;
mod fixed;
mod replay;
mod track;

/// DOS strings may contain stale bytes after NUL and non-ASCII identifiers.
/// Render the meaningful prefix while escaping any bytes unsafe for a terminal.
fn dos_text(bytes: &[u8]) -> String {
    bytes
        .split(|&byte| byte == 0)
        .next()
        .unwrap_or_default()
        .iter()
        .flat_map(|&byte| std::ascii::escape_default(byte))
        .map(char::from)
        .collect()
}

/// Read a little-endian DOS header word. The caller must validate the header length.
fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

/// Print header fields and a short string preview without modifying the file.
fn inspect(path: &Path) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(path)?;
    println!("{}: {} bytes", path.display(), bytes.len());
    if bytes.starts_with(b"MZ") && bytes.len() >= 28 {
        // MZ offsets 0x08, 0x04, and 0x02 store the header size in 16-byte
        // paragraphs, the total 512-byte page count, and the last page's usage.
        let header = usize::from(word(&bytes, 8)) * 16;
        let pages = usize::from(word(&bytes, 4));
        let last = usize::from(word(&bytes, 2));
        let declared = if last == 0 {
            // Zero means the final page is full, rather than absent.
            pages * 512
        } else {
            pages.saturating_sub(1) * 512 + last
        };
        println!("  DOS MZ: header={header}, declared size={declared}");
        println!(
            "  Entry CS:IP={:04X}:{:04X}, stack SS:SP={:04X}:{:04X}",
            word(&bytes, 22),
            word(&bytes, 20),
            word(&bytes, 14),
            word(&bytes, 16)
        );
        println!(
            "  Relocations={}, table=0x{:04X}",
            word(&bytes, 6),
            word(&bytes, 24)
        );
        if header > bytes.len() || declared > bytes.len() {
            // The game's .HDR modules contain only a header, not the full image.
            println!("  Incomplete file or separate header: do not treat as a standalone EXE.");
        }
    }
    println!("  ASCII strings (minimum 8 bytes):");
    // Split on nonprintable bytes and cap the preview: binary code also contains
    // accidental printable sequences, so these strings are clues, not metadata.
    for string in bytes
        .split(|b| !(0x20..=0x7e).contains(b))
        .filter(|s| s.len() >= 8)
        .take(30)
    {
        println!("    {}", String::from_utf8_lossy(string));
    }
    Ok(())
}

/// Dispatch a command and propagate I/O or format failures to the entry point.
fn run() -> Result<(), Box<dyn Error>> {
    // Keep paths as OS strings so non-UTF-8 filenames need not be converted.
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.first().and_then(|s| s.to_str()) {
        Some("inventory") if args.len() == 2 => {
            let mut entries = fs::read_dir(&args[1])?.collect::<Result<Vec<_>, _>>()?;
            // Filesystem iteration order is unspecified; sorting makes reports stable.
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let meta = entry.metadata()?;
                if meta.is_file() {
                    println!("{:>8}  {}", meta.len(), entry.file_name().to_string_lossy());
                }
            }
        }
        Some("inspect") if args.len() == 2 => inspect(Path::new(&args[1]))?,
        Some("track") if args.len() == 2 => {
            let bytes = fs::read(&args[1])?;
            let track = track::Track::parse(&bytes)?;
            println!("Track: {}x{}, scenery={} ({}), trailing byte=0x{:02X}",
                track::SIDE, track::SIDE, track.scenery_id(), track.scenery_name(), track.trailing_byte());
            println!("Element IDs, top to bottom (terrain is aligned from reversed storage rows):");
            for y in 0..track::SIDE {
                for x in 0..track::SIDE { print!("{:02X} ", track.element(x,y).unwrap()); }
                println!();
            }
            println!("Top-left terrain ID: 0x{:02X}", track.terrain(0,0).unwrap());
            println!("Occupied element cells: {}", track::CELLS - track.element_counts()[0]);
        }
        Some("replay") if args.len() == 2 => {
            let bytes = fs::read(&args[1])?;
            let replay = replay::Replay::parse(&bytes)?;
            println!("Track: {}, scenery: {}", dos_text(replay.track_name), replay.track.scenery_name());
            println!("Player car: {}, material={}, transmission={}", dos_text(replay.player_car), replay.player_material, replay.player_transmission);
            println!("Opponent: type={}, car={}, material={}, transmission={}", replay.opponent_type, dos_text(replay.opponent_car), replay.opponent_material, replay.opponent_transmission);
            println!("Frames: {}, FPS: {}, duration: {:.3} seconds", replay.inputs.len(), replay.fps, replay.duration_seconds());
            let mut counts = [0usize; 256];
            for &input in replay.inputs { counts[input as usize] += 1; }
            println!("Raw command frequencies:");
            for (raw, &count) in counts.iter().enumerate().filter(|(_, n)| **n != 0) {
                println!("  0x{raw:02X}: {count} frames");
            }
        }
        Some("replay-inputs") if args.len() == 3 => {
            let bytes = fs::read(&args[1])?;
            let replay = replay::Replay::parse(&bytes)?;
            let mut output = std::io::BufWriter::new(fs::OpenOptions::new()
                .write(true).create_new(true).open(&args[2])?);
            writeln!(output, "frame,time_seconds,raw,throttle_code,steering_code,shift_up,shift_down,unknown_bits")?;
            for (frame, &raw) in replay.inputs.iter().enumerate() {
                let control = replay::Controls::from(raw);
                writeln!(output, "{frame},{:.6},{raw},{},{},{},{},{}", frame as f64 / f64::from(replay.fps), control.throttle_code, control.steering_code, u8::from(control.shift_up), u8::from(control.shift_down), control.unknown_bits)?;
            }
            output.flush()?;
            println!("Exported {} recorded input bytes; timestamps are frame indices divided by FPS", replay.inputs.len());
        }
        Some("fixed-mul") if args.len() == 3 => {
            let a: i16 = args[1].to_str().ok_or("Invalid integer")?.parse()?;
            let b: i16 = args[2].to_str().ok_or("Invalid integer")?.parse()?;
            println!("{}", fixed::multiply_and_scale(a,b));
        }
        Some("decompress") if args.len() == 3 => {
            let input = fs::read(&args[1])?;
            let output = compression::decompress(&input)?;
            // Exclusive creation prevents overwriting originals or previous results.
            fs::OpenOptions::new().write(true).create_new(true).open(&args[2])?
                .write_all(&output)?;
            println!("Decompressed {} -> {} bytes", input.len(), output.len());
        }
        Some("combine") if args.len() == 4 => {
            let mode = args[2].to_str().ok_or("Invalid video mode")?.to_ascii_uppercase();
            let output = executable::combine(Path::new(&args[1]), &mode)?;
            // This produces an EXEPACK-packed DOS image, not a native executable.
            fs::OpenOptions::new().write(true).create_new(true).open(&args[3])?
                .write_all(&output)?;
            println!("Combined {mode} packed executable: {} bytes (EXEPACK unpacking still required)", output.len());
        }
        _ => return Err("Usage: stunts-rs inventory <directory> | inspect <file> | track <trk> | replay <rpl> | replay-inputs <rpl> <new-csv> | fixed-mul <i16> <i16> | decompress <input> <new-output> | combine <game-directory> <video-mode> <new-output>".into()),
    }
    Ok(())
}

fn main() {
    // A nonzero status lets shell scripts stop when a decoding step fails.
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(1);
    }
}
