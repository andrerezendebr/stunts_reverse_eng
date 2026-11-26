//! Uncompressed Stunts resource directories used by RES and decoded PRE/PVS files.
//! Names and offsets are separate arrays; offsets are relative to the payload.

#[derive(Debug)]
pub struct Resource<'a> {
    pub id: [u8; 4],
    pub offset: usize,
    /// Extent up to the next greater offset, which may include format padding.
    pub bytes: &'a [u8],
}

#[derive(Debug)]
pub struct Archive<'a> {
    pub entries: Vec<Resource<'a>>,
}

impl<'a> Archive<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() < 6 {
            return Err("Resource archive is missing its six-byte header".into());
        }
        let declared = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        if declared != bytes.len() {
            return Err("Resource archive size does not match its header".into());
        }
        let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
        let base = 6 + count * 8;
        if base > bytes.len() {
            return Err("Truncated resource directory".into());
        }
        let payload = &bytes[base..];
        let mut offsets = Vec::with_capacity(count);
        for index in 0..count {
            let position = 6 + count * 4 + index * 4;
            let offset =
                u32::from_le_bytes(bytes[position..position + 4].try_into().unwrap()) as usize;
            if offset > payload.len() {
                return Err("Resource offset lies outside the payload".into());
            }
            offsets.push(offset);
        }
        // Directory order follows IDs, not payload locations. Sort a separate copy
        // to determine boundaries without changing first-match lookup semantics.
        let mut boundaries = offsets.clone();
        boundaries.push(payload.len());
        boundaries.sort_unstable();
        boundaries.dedup();
        let entries = offsets
            .iter()
            .enumerate()
            .map(|(index, &offset)| {
                let boundary = boundaries.binary_search(&offset).unwrap();
                let end = boundaries
                    .get(boundary + 1)
                    .copied()
                    .unwrap_or(payload.len());
                Resource {
                    id: bytes[6 + index * 4..10 + index * 4].try_into().unwrap(),
                    offset,
                    bytes: &payload[offset..end],
                }
            })
            .collect();
        Ok(Self { entries })
    }

    /// Exact four-byte lookup. Short-name padding rules are not implemented here.
    pub fn get(&self, id: &[u8; 4]) -> Option<&'a [u8]> {
        self.entries
            .iter()
            .find(|entry| &entry.id == id)
            .map(|entry| entry.bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsorted_offsets_aliases_and_invalid_directory_bounds() {
        // A and C alias the last byte, B points at the first two bytes.
        let mut bytes = vec![33, 0, 0, 0, 3, 0];
        bytes.extend(b"aaaa".iter().chain(b"bbbb").chain(b"cccc"));
        for offset in [2u32, 0, 2] {
            bytes.extend(offset.to_le_bytes());
        }
        bytes.extend([10, 20, 30]);
        let archive = Archive::parse(&bytes).unwrap();
        assert_eq!(archive.get(b"aaaa"), Some(&[30][..]));
        assert_eq!(archive.get(b"bbbb"), Some(&[10, 20][..]));
        assert_eq!(archive.get(b"cccc"), Some(&[30][..]));
        bytes[18..22].copy_from_slice(&4u32.to_le_bytes());
        assert!(Archive::parse(&bytes).is_err());
        assert!(Archive::parse(&bytes[..5]).is_err());
        assert!(Archive::parse(&bytes[..32]).is_err());
    }

    #[test]
    fn reads_original_plain_resource_archives() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let mut count = 0;
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("res"))
            {
                let bytes = std::fs::read(&path).unwrap();
                Archive::parse(&bytes)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                count += 1;
            }
        }
        assert_eq!(count, 13);
    }

    #[test]
    fn reads_decoded_resource_archives() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let mut count = 0;
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let extension = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_uppercase();
            if ["PRE", "PVS", "P3S", "PES"].contains(&extension.as_str()) {
                let decoded =
                    crate::compression::decompress(&std::fs::read(&path).unwrap()).unwrap();
                Archive::parse(&decoded)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                count += 1;
            }
        }
        assert_eq!(count, 71);
    }
}
