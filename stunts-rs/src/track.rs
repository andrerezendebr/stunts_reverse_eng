//! Read the original 30-by-30 track format without changing its stored bytes.
pub const SIDE: usize = 30;
pub const CELLS: usize = SIDE * SIDE;
pub const FILE_SIZE: usize = CELLS * 2 + 2;

/// A borrowed track image. Unknown tile IDs and the final byte are preserved.
#[derive(Debug)]
pub struct Track<'a> {
    bytes: &'a [u8; FILE_SIZE],
}

impl<'a> Track<'a> {
    /// This parser supports the exact 1802-byte format used by this game copy.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        let bytes = bytes.try_into().map_err(|_| {
            format!(
                "Expected a {FILE_SIZE}-byte track, got {} bytes",
                bytes.len()
            )
        })?;
        Ok(Self { bytes })
    }

    /// Return an element ID using display coordinates: x rightward, y downward.
    pub fn element(&self, x: usize, y: usize) -> Option<u8> {
        (x < SIDE && y < SIDE).then(|| self.bytes[y * SIDE + x])
    }

    /// Terrain rows are stored in the opposite direction to element rows.
    /// Reverse them here so both layers describe the same visible tile.
    pub fn terrain(&self, x: usize, y: usize) -> Option<u8> {
        (x < SIDE && y < SIDE).then(|| self.bytes[CELLS + 1 + (SIDE - 1 - y) * SIDE + x])
    }

    pub fn scenery_id(&self) -> u8 {
        self.bytes[CELLS]
    }

    pub fn scenery_name(&self) -> &'static str {
        match self.scenery_id() {
            0 => "desert",
            1 => "tropical",
            2 => "alpine",
            3 => "city",
            4 => "country",
            _ => "unknown",
        }
    }

    /// The meaning of the final byte has not been verified; do not discard it.
    pub fn trailing_byte(&self) -> u8 {
        self.bytes[FILE_SIZE - 1]
    }

    pub fn element_counts(&self) -> [usize; 256] {
        let mut counts = [0; 256];
        for &element in &self.bytes[..CELLS] {
            counts[element as usize] += 1;
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_layers_and_preserves_unknown_fields() {
        let mut bytes = [0; FILE_SIZE];
        bytes[0] = 0xaa;
        bytes[CELLS - 1] = 0xbb;
        bytes[CELLS + 1 + 29 * SIDE] = 0x11;
        bytes[CELLS + SIDE] = 0x22;
        bytes[CELLS] = 0xff;
        bytes[FILE_SIZE - 1] = 0x7e;
        let track = Track::parse(&bytes).unwrap();
        assert_eq!(track.element(0, 0), Some(0xaa));
        assert_eq!(track.terrain(0, 0), Some(0x11));
        assert_eq!(track.element(29, 29), Some(0xbb));
        assert_eq!(track.terrain(29, 29), Some(0x22));
        assert_eq!(track.element(30, 0), None);
        assert_eq!(track.terrain(0, usize::MAX), None);
        assert_eq!(track.scenery_name(), "unknown");
        assert_eq!(track.trailing_byte(), 0x7e);
        assert!(Track::parse(&bytes[..FILE_SIZE - 1]).is_err());
    }

    #[test]
    fn reads_all_original_tracks() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let mut count = 0;
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("trk"))
            {
                let bytes = std::fs::read(&path).unwrap();
                let track = Track::parse(&bytes).unwrap();
                assert_eq!(track.element_counts().iter().sum::<usize>(), CELLS);
                count += 1;
            }
        }
        assert!(count > 0);
        println!("Parsed {count} original tracks");
    }
}
