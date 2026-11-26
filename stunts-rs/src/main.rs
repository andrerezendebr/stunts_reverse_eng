use std::{env, error::Error, fs, path::Path, process};

fn word(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn inspect(path: &Path) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(path)?;
    println!("{}: {} bytes", path.display(), bytes.len());
    if bytes.starts_with(b"MZ") && bytes.len() >= 28 {
        let header = usize::from(word(&bytes, 8)) * 16;
        let pages = usize::from(word(&bytes, 4));
        let last = usize::from(word(&bytes, 2));
        let declared = if last == 0 { pages * 512 } else {
            pages.saturating_sub(1) * 512 + last
        };
        println!("  DOS MZ: header={header}, declared size={declared}");
        println!("  Entry CS:IP={:04X}:{:04X}, stack SS:SP={:04X}:{:04X}",
            word(&bytes, 22), word(&bytes, 20), word(&bytes, 14), word(&bytes, 16));
        println!("  Relocations={}, table=0x{:04X}", word(&bytes, 6), word(&bytes, 24));
        if header > bytes.len() || declared > bytes.len() {
            println!("  Incomplete file or separate header: do not treat as a standalone EXE.");
        }
    }
    println!("  ASCII strings (minimum 8 bytes):");
    for string in bytes.split(|b| !(0x20..=0x7e).contains(b)).filter(|s| s.len() >= 8).take(30) {
        println!("    {}", String::from_utf8_lossy(string));
    }
    Ok(())
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.first().and_then(|s| s.to_str()) {
        Some("inventory") if args.len() == 2 => {
            let mut entries = fs::read_dir(&args[1])?.collect::<Result<Vec<_>, _>>()?;
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let meta = entry.metadata()?;
                if meta.is_file() {
                    println!("{:>8}  {}", meta.len(), entry.file_name().to_string_lossy());
                }
            }
        }
        Some("inspect") if args.len() == 2 => inspect(Path::new(&args[1]))?,
        _ => return Err("Usage: stunts-rs inventory <directory> | inspect <file>".into()),
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(1);
    }
}
