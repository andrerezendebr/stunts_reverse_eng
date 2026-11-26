//! Original P3S shape records: vertices, visibility masks and indexed primitives.
//! Materials remain raw indices; the desktop prototype uses its own palette.
#[derive(Debug)]
pub struct Shape {
    pub vertices: Vec<[i16; 3]>,
    pub primitives: Vec<Primitive>,
}
#[derive(Debug)]
pub struct Primitive {
    pub kind: u8,
    pub flags: u8,
    pub materials: Vec<u8>,
    pub indices: Vec<u8>,
}
impl Shape {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 4 {
            return Err("Truncated shape header".into());
        }
        let (nv, np, nm) = (bytes[0] as usize, bytes[1] as usize, bytes[2] as usize);
        let mut pos = 4;
        let mut vertices = Vec::with_capacity(nv);
        for _ in 0..nv {
            let raw = bytes.get(pos..pos + 6).ok_or("Truncated shape vertices")?;
            vertices.push(std::array::from_fn(|i| {
                i16::from_le_bytes([raw[2 * i], raw[2 * i + 1]])
            }));
            pos += 6;
        }
        pos += np * 8; // Two original per-primitive visibility masks.
        if pos > bytes.len() {
            return Err("Truncated shape visibility masks".into());
        }
        let counts = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 2, 6, 3, 0, 0];
        let mut primitives = Vec::with_capacity(np);
        for _ in 0..np {
            let head = bytes.get(pos..pos + 2).ok_or("Truncated shape primitive")?;
            let kind = head[0];
            let flags = head[1];
            pos += 2;
            let count = *counts
                .get(kind as usize)
                .ok_or("Unknown shape primitive type")?;
            if count == 0 {
                return Err("Unsupported shape primitive type".into());
            }
            let materials = bytes
                .get(pos..pos + nm)
                .ok_or("Truncated shape materials")?
                .to_vec();
            pos += nm;
            let indices = bytes
                .get(pos..pos + count)
                .ok_or("Truncated shape indices")?
                .to_vec();
            pos += count;
            if indices.iter().any(|&i| i as usize >= nv) {
                return Err("Shape vertex index out of bounds".into());
            }
            primitives.push(Primitive {
                kind,
                flags,
                materials,
                indices,
            });
        }
        Ok(Self {
            vertices,
            primitives,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_original_indy_mesh_and_rejects_every_truncated_prefix() {
        let decoded =
            crate::compression::decompress(include_bytes!("../../stunts_dos/STPMIN.P3S")).unwrap();
        let archive = crate::resource::Archive::parse(&decoded).unwrap();
        let raw = archive.get(b"car0").unwrap();
        let shape = Shape::parse(raw).unwrap();
        assert!(!shape.vertices.is_empty());
        assert!(!shape.primitives.is_empty());
        // Some archives pad records, so locate the minimal valid record first.
        let length = (4..=raw.len())
            .find(|&n| Shape::parse(&raw[..n]).is_ok())
            .unwrap();
        for n in 0..length {
            assert!(Shape::parse(&raw[..n]).is_err(), "prefix {n}");
        }
    }
}
