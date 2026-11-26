//! Stunts resource compression: bounded RLE and canonical variable-length codes.
//! Format reference: 4d-stunts/restunts, src/restunts/c/fileio.c.

type Result<T> = std::result::Result<T, String>;
// Tool-side memory bound, not a field or limit imposed by the original game.
const LIMIT: usize = 16 * 1024 * 1024;

/// Cursor over one compressed buffer, with checked reads for truncated input.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Result<u8> {
        let value = *self
            .bytes
            .get(self.pos)
            .ok_or("Truncated compressed input")?;
        self.pos += 1;
        Ok(value)
    }

    /// Decode the three-byte little-endian sizes used by the resource headers.
    fn size24(&mut self) -> Result<usize> {
        Ok(self.byte()? as usize | (self.byte()? as usize) << 8 | (self.byte()? as usize) << 16)
    }

    /// Borrow a bounded region and advance only when the whole region exists.
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(len).ok_or("Input size overflow")?;
        let value = self
            .bytes
            .get(self.pos..end)
            .ok_or("Truncated compressed input")?;
        self.pos = end;
        Ok(value)
    }
}

/// Append a repeated sequence after checking multiplication and remaining capacity.
/// Final runs must fit the declared output length; they are never silently clipped.
fn repeat(out: &mut Vec<u8>, sequence: &[u8], count: usize, limit: usize) -> Result<()> {
    let extra = sequence
        .len()
        .checked_mul(count)
        .ok_or("Run size overflow")?;
    if extra > limit.saturating_sub(out.len()) {
        return Err("Run exceeds output size limit".into());
    }
    for _ in 0..count {
        out.extend_from_slice(sequence);
    }
    Ok(())
}

/// Decode an RLE pass: optional sequence expansion, then single-byte runs.
/// The reader is already past the pass type and its decoded-size field.
fn rle(reader: &mut Reader<'_>, length: usize) -> Result<Vec<u8>> {
    let source_length = reader.size24()?;
    let _reserved = reader.byte()?;
    let flags = reader.byte()?;
    let escapes = reader.take((flags & 0x7f) as usize)?;
    if escapes.is_empty() || escapes.len() > 16 {
        return Err("Invalid RLE escape table".into());
    }
    let mut lookup = [0usize; 256];
    // Zero denotes a literal byte. Escape indices are one-based because the
    // original format uses the index itself to choose the repetition encoding.
    for (index, &value) in escapes.iter().enumerate() {
        // Real resources can repeat an escape byte; the loader keeps its last index.
        lookup[value as usize] = index + 1;
    }
    let expanded;
    let input = if flags & 0x80 == 0 {
        // Bit 7 disables this stage. Otherwise escape-table entry 1 delimits a
        // sequence: <escape><sequence><escape><total repetition count>.
        let &escape = escapes.get(1).ok_or("Missing RLE sequence escape")?;
        let mut source = Reader {
            bytes: reader.take(source_length)?,
            pos: 0,
        };
        let mut output = Vec::new();
        while source.pos < source.bytes.len() {
            let value = source.byte()?;
            if value == escape {
                let start = source.pos;
                while source.byte()? != escape {}
                let end = source.pos - 1;
                let count = source.byte()? as usize;
                if count == 0 {
                    return Err("Zero RLE sequence repetition count".into());
                }
                repeat(&mut output, &source.bytes[start..end], count, LIMIT)?;
            } else {
                repeat(&mut output, &[value], 1, LIMIT)?;
            }
        }
        expanded = output;
        // Retain the intermediate buffer until the single-byte stage finishes.
        expanded.as_slice()
    } else {
        reader.take(source_length)?
    };
    let mut source = Reader {
        bytes: input,
        pos: 0,
    };
    let mut output = Vec::with_capacity(length);
    // The final size determines when to stop; decoding may leave padding unread.
    while output.len() < length {
        let value = source.byte()?;
        let code = lookup[value as usize];
        let (value, count) = match code {
            0 => (value, 1),
            1 => {
                // First escape: an explicit 8-bit count followed by its byte.
                let count = source.byte()? as usize;
                (source.byte()?, count)
            }
            3 => {
                // Third escape: a little-endian 16-bit count followed by its byte.
                let count = source.byte()? as usize | (source.byte()? as usize) << 8;
                (source.byte()?, count)
            }
            // Other escapes encode the run length directly in their table index.
            _ => (source.byte()?, code - 1),
        };
        repeat(&mut output, &[value], count, length)?;
    }
    Ok(output)
}

/// Decode canonical variable-length prefix codes, optionally as byte deltas.
fn vle(reader: &mut Reader<'_>, length: usize) -> Result<Vec<u8>> {
    let flags = reader.byte()?;
    let widths = (flags & 0x7f) as usize;
    if !(1..=16).contains(&widths) {
        return Err("Invalid VLE maximum code width".into());
    }
    let counts = reader.take(widths)?;
    // counts[0] is the number of one-bit codes, counts[1] the two-bit codes,
    // and so on. Symbols follow in increasing code-width order.
    let alphabet_size: usize = counts.iter().map(|&n| n as usize).sum();
    if alphabet_size == 0 || alphabet_size > 256 {
        return Err("Invalid VLE alphabet size".into());
    }
    let alphabet = reader.take(alphabet_size)?;
    let mut first_code = vec![0usize; widths];
    let mut first_symbol = vec![0usize; widths];
    let mut code = 0;
    let mut symbol = 0;
    for (index, &count) in counts.iter().enumerate() {
        // Canonical codes are consecutive within each width. Advancing a width
        // shifts the first unused code left, adding room for one more bit.
        first_code[index] = code;
        first_symbol[index] = symbol;
        if code + count as usize > 1usize << (index + 1) {
            // More codes than the width can hold would create overlapping prefixes.
            return Err("Oversubscribed VLE code table".into());
        }
        code = (code + count as usize) << 1;
        symbol += count as usize;
    }
    let mut output = Vec::with_capacity(length);
    let mut remaining = 0;
    let mut bits = 0u8;
    let mut previous = 0u8;
    for _ in 0..length {
        let mut code = 0;
        let mut decoded = None;
        for index in 0..widths {
            if remaining == 0 {
                bits = reader.byte()?;
                remaining = 8;
            }
            code = (code << 1) | (bits >> 7) as usize;
            // Bits arrive most significant first, including across byte boundaries.
            bits <<= 1;
            remaining -= 1;
            if code >= first_code[index] && code - first_code[index] < counts[index] as usize {
                // The code's position within this width selects its alphabet entry.
                decoded = Some(alphabet[first_symbol[index] + code - first_code[index]]);
                break;
            }
        }
        let value = decoded.ok_or("Invalid VLE bit sequence")?;
        previous = if flags & 0x80 != 0 {
            // Additive VLE stores deltas. Explicit wrapping preserves DOS u8 overflow.
            previous.wrapping_add(value)
        } else {
            value
        };
        output.push(previous);
    }
    Ok(output)
}

/// Decode a resource and return its bytes without changing the source buffer.
/// A normal file starts with its sole pass header. Multi-pass files prepend a
/// pass count and final size; each decoded intermediate buffer is the next input.
pub fn decompress(input: &[u8]) -> Result<Vec<u8>> {
    let mut source = Reader {
        bytes: input,
        pos: 0,
    };
    let flags = *input.first().ok_or("Empty compressed input")?;
    let (passes, expected) = if flags & 0x80 != 0 {
        source.byte()?;
        ((flags & 0x7f) as usize, Some(source.size24()?))
    } else {
        // Do not consume the first byte: it is the compression type of this pass.
        (1, None)
    };
    if passes == 0 || passes > 16 {
        return Err("Invalid compression pass count".into());
    }
    let mut current = source.bytes[source.pos..].to_vec();
    for _ in 0..passes {
        let mut reader = Reader {
            bytes: &current,
            pos: 0,
        };
        let kind = reader.byte()?;
        let length = reader.size24()?;
        if length > LIMIT {
            return Err("Decompressed size exceeds limit".into());
        }
        current = match kind {
            1 => rle(&mut reader, length)?,
            2 => vle(&mut reader, length)?,
            _ => return Err(format!("Unknown compression type {kind}")),
        };
    }
    if expected.is_some_and(|length| length != current.len()) {
        // Intermediate sizes may differ, but the final outer size must agree.
        return Err("Multi-pass output size does not match header".into());
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::decompress;

    #[test]
    fn canonical_and_additive_vle() {
        // A=0, B=10, C=11; stream A B C A.
        let mut data = vec![2, 4, 0, 0, 2, 1, 2, b'A', b'B', b'C', 0b01011000];
        assert_eq!(decompress(&data).unwrap(), b"ABCA");
        data[4] |= 0x80;
        data[7..10].copy_from_slice(&[1, 2, 255]);
        assert_eq!(decompress(&data).unwrap(), [1, 3, 2, 3]);
    }

    #[test]
    fn rle_and_nested_passes() {
        let data = [1, 5, 0, 0, 4, 0, 0, 0, 0x81, 0xff, b'A', 0xff, 4, b'B'];
        assert_eq!(decompress(&data).unwrap(), b"ABBBB");
        let mut nested = vec![0x81, 5, 0, 0];
        nested.extend(data);
        assert_eq!(decompress(&nested).unwrap(), b"ABBBB");
    }

    #[test]
    fn rejects_truncation_and_output_overrun() {
        assert!(decompress(&[]).is_err());
        assert!(decompress(&[2, 1, 0, 0, 1, 2, b'A']).is_err());
        assert!(decompress(&[1, 1, 0, 0, 3, 0, 0, 0, 0x81, 0xff, 0xff, 2, b'A']).is_err());
    }

    #[test]
    fn sequence_rle_and_long_runs() {
        // Sequence pass expands AB twice before the single-byte pass.
        assert_eq!(
            decompress(&[1, 4, 0, 0, 5, 0, 0, 0, 2, 0xfe, 0xff, 0xff, b'A', b'B', 0xff, 2])
                .unwrap(),
            b"ABAB"
        );
        // Third escape selects a 16-bit run count.
        let data = [
            1, 0, 1, 0, 4, 0, 0, 0, 0x83, 0xfd, 0xfe, 0xff, 0xff, 0, 1, b'X',
        ];
        assert_eq!(decompress(&data).unwrap(), vec![b'X'; 256]);
        // SDGAME2.PVS uses repeated escape bytes: the last table index wins.
        let duplicate = [
            1, 2, 0, 0, 4, 0, 0, 0, 0x83, 0xff, 0xfe, 0xff, 0xff, 2, 0, b'Y',
        ];
        assert_eq!(decompress(&duplicate).unwrap(), b"YY");
    }

    #[test]
    fn long_vle_codes_and_two_passes() {
        // One nine-bit symbol exercises decoding beyond the eight-bit boundary.
        let mut data = vec![2, 2, 0, 0, 9];
        data.extend([0; 8]);
        data.extend([1, b'Z', 0, 0, 0]);
        assert_eq!(decompress(&data).unwrap(), b"ZZ");
        // The first RLE pass emits another complete RLE stream, not final bytes.
        let inner = [1, 2, 0, 0, 2, 0, 0, 0, 0x81, 0xff, b'A', b'B'];
        let mut nested = vec![0x82, 2, 0, 0, 1, 12, 0, 0, 12, 0, 0, 0, 0x81, 0xfe];
        nested.extend(inner);
        assert_eq!(decompress(&nested).unwrap(), b"AB");
    }

    #[test]
    fn decodes_original_resource_corpus() {
        // Real-file coverage catches format quirks missing from synthetic examples.
        // These extensions are compressed resources; plain RES/TRK files are excluded.
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let mut count = 0;
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let extension = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_uppercase();
            if ["CMN", "COD", "DIF", "PRE", "PVS", "P3S", "PES"].contains(&extension.as_str()) {
                let input = std::fs::read(&path).unwrap();
                let expected =
                    input[1] as usize | (input[2] as usize) << 8 | (input[3] as usize) << 16;
                let output = decompress(&input)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                assert_eq!(output.len(), expected, "{}", path.display());
                count += 1;
            }
        }
        assert!(
            count >= 70,
            "Expected the original compressed game resources"
        );
        println!("Decoded {count} original compressed resources");
    }
}
