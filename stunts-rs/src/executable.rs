//! Assemble the decompressed video modules into the original packed DOS image.
use crate::compression::decompress;
use std::{error::Error, fs, path::Path};

/// Read a DOS header word after the caller has checked the 30-byte header.
fn word(bytes: &[u8], offset: usize) -> usize {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize
}

/// Rebuild one video-specific DOS EXE from the original compressed modules.
/// The returned bytes still contain EXEPACK compression; unpacking is a separate step.
pub fn combine(directory: &Path, video: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    if !["MCGA", "CGA", "TDY", "EGA"].contains(&video) {
        return Err("Video mode must be MCGA, CGA, TDY, or EGA".into());
    }
    let header = fs::read(directory.join(format!("{video}.HDR")))?;
    if header.len() != 30 || &header[..2] != b"MZ" {
        return Err("Expected a 30-byte DOS MZ module header".into());
    }
    let pages = word(&header, 4);
    let last = word(&header, 2);
    if pages == 0 || last > 511 {
        return Err("Invalid DOS executable size".into());
    }
    // MZ page counts include the header; a zero last-page size denotes 512 bytes.
    let size = (pages - 1) * 512 + if last == 0 { 512 } else { last };
    let header_size = word(&header, 8) * 16;
    let mut common = decompress(&fs::read(directory.join("EGA.CMN"))?)?;
    // EGA uses the common image directly. Other video modes patch it before
    // appending their own code, reproducing the original module-loading layout.
    if video != "EGA" {
        let patch = decompress(&fs::read(directory.join(format!("{video}.DIF")))?)?;
        apply_difference(&mut common, &patch)?;
    }
    let code = decompress(&fs::read(directory.join(format!("{video}.COD")))?)?;
    // Reject mismatched modules instead of emitting a silently truncated EXE.
    if header_size < header.len() || header_size + common.len() + code.len() != size {
        return Err("Module lengths do not match the DOS header".into());
    }
    // Header padding must be initialized; the historical combiner left it undefined.
    let mut image = vec![0u8; header_size];
    image[..header.len()].copy_from_slice(&header);
    image.extend_from_slice(&common);
    image.extend_from_slice(&code);
    Ok(image)
}

/// Apply relative DIF replacement records to the decoded common module in place.
fn apply_difference(image: &mut [u8], patch: &[u8]) -> Result<(), Box<dyn Error>> {
    // The loader starts at offset -1. Express it modulo the 20-bit address space
    // so a first delta of 1 writes at byte zero, matching segmented arithmetic.
    let mut cursor = 0xfffffusize;
    let mut position = 0;
    loop {
        let field = patch
            .get(position..position + 2)
            .ok_or("Truncated DIF offset")?;
        let record = u16::from_le_bytes([field[0], field[1]]) as usize;
        position += 2;
        if record == 0 {
            // The terminator has no replacement payload.
            if position != patch.len() {
                return Err("Unexpected data after DIF terminator".into());
            }
            return Ok(());
        }
        // The original loader advances a wrapping 16:16 real-mode address.
        cursor = (cursor + (record & 0x7fff)) & 0xfffff;
        // Low 15 bits advance the destination; bit 15 selects a four-byte write.
        let length = if record & 0x8000 != 0 { 4 } else { 2 };
        let data = patch
            .get(position..position + length)
            .ok_or("Truncated DIF data")?;
        image
            .get_mut(cursor..cursor + length)
            .ok_or("DIF write outside common module")?
            .copy_from_slice(data);
        position += length;
        // The destination stays at this record's start. The next delta is
        // relative to that address, not the end of the replacement bytes.
    }
}

#[cfg(test)]
mod tests {
    use super::apply_difference;

    #[test]
    fn dif_wraps_initial_address_and_uses_two_record_widths() {
        let mut image = [0; 8];
        apply_difference(&mut image, &[1, 0, 10, 11, 3, 0x80, 20, 21, 22, 23, 0, 0]).unwrap();
        assert_eq!(image, [10, 11, 0, 20, 21, 22, 23, 0]);
        assert!(apply_difference(&mut image, &[1, 0, 10]).is_err());
        assert!(apply_difference(&mut image, &[0x10, 0, 10, 11, 0, 0]).is_err());
    }
}
