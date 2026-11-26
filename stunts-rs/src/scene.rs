//! Original scene metadata and meshes with experimental triangle-based contacts.
//! The metadata is extracted from the original MCGA image; contact queries here
//! are new geometry code, not the original build_track_object/plane solver.
use crate::{compression, matrix, prototype::TILE, resource::Archive, shape::Shape, track::Track};
use std::path::Path;
const DATA: &[u8; 3740] = include_bytes!("scene_metadata.bin");
pub const SCALE: f32 = TILE / 1024.0;

#[derive(Clone, Copy, Debug)]
pub struct Object {
    pub shape: usize,
    pub angle: u16,
    pub overlay: u8,
    pub paint: u8,
    pub multi: u8,
    pub physical: u8,
}
fn record(offset: usize) -> Object {
    let b = &DATA[offset..offset + 14];
    let pointer = u16::from_le_bytes([b[4], b[5]]);
    Object {
        shape: pointer.saturating_sub(0x764c) as usize / 22,
        angle: u16::from_le_bytes([b[2], b[3]]),
        overlay: b[8],
        paint: b[9],
        multi: b[11],
        physical: b[12],
    }
}
pub fn object(id: u8) -> Option<Object> {
    (id < 215).then(|| record(464 + id as usize * 14))
}
pub fn terrain(id: u8) -> Option<Object> {
    (id < 19).then(|| record(464 + 3010 + id as usize * 14))
}
pub fn shape_id(index: usize) -> Option<[u8; 4]> {
    (index < 116).then(|| DATA[index * 4..index * 4 + 4].try_into().unwrap())
}
pub fn start_heading(id: u8) -> Option<f32> {
    matches!(id,1|0x86..=0x89|0x93..=0x96|0xb3..=0xb5|0x6f..=0x72|0x46..=0x49).then(|| {
        std::f32::consts::PI - object(id).unwrap().angle as f32 * std::f32::consts::TAU / 1024.0
    })
}

/// Complete original subst_hillroad_track (1E1A:38BA). The renderer replaces
/// compatible straight roads/ramps on slopes with dedicated inclined meshes.
pub fn hillroad(terrain: u8, element: u8) -> u8 {
    let orientation = match terrain {
        7 => 0,
        8 => 1,
        9 => 2,
        10 => 3,
        _ => return 0,
    };
    let vertical = orientation % 2 == 0;
    match element {
        4 if vertical => 0xb6 + orientation,
        5 if !vertical => 0xb6 + orientation,
        0xe if vertical => 0xba + orientation,
        0xf if !vertical => 0xba + orientation,
        0x18 if vertical => 0xbe + orientation,
        0x19 if !vertical => 0xbe + orientation,
        0x27 | 0x3b | 0x62 if terrain == 7 => 0xc2,
        0x24 | 0x38 | 0x5f if terrain == 8 => 0xc3,
        0x26 | 0x3a | 0x61 if terrain == 9 => 0xc4,
        0x25 | 0x39 | 0x60 if terrain == 10 => 0xc5,
        _ => 0,
    }
}

#[derive(Clone, Debug)]
pub struct Triangle {
    pub points: [[f32; 3]; 3],
    pub color: u32,
    pub surface: u8,
    pub solid: bool,
}
pub struct Scene {
    pub triangles: Vec<Triangle>,
    buckets: Vec<Vec<usize>>,
}
#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub height: f32,
    pub surface: u8,
}

fn color(material: u8) -> u32 {
    match material {
        19 | 22 | 23 => 0x62656a,
        20 => 0x50545b,
        21 | 24 | 27 | 30 => 0xede7d2,
        25 | 26 => 0x9c7047,
        28 | 29 => 0x9ed0df,
        31 | 32 => 0xa4a8ac,
        33 => 0x74787c,
        101..=105 | 127 | 128 => 0x498147,
        106..=110 => 0x427c3f,
        _ => [0xa74532, 0xa48b62, 0x436e3c, 0xb5a68a, 0x526c80, 0xc1bca7][material as usize % 6],
    }
}

impl Scene {
    pub fn load(root: &Path, track: &Track<'_>) -> Result<Self, String> {
        let mut buffers = Vec::new();
        for file in ["GAME1.P3S", "GAME2.P3S"] {
            buffers.push(compression::decompress(
                &std::fs::read(root.join(file)).map_err(|e| format!("{file}: {e}"))?,
            )?);
        }
        let archives: Vec<_> = buffers
            .iter()
            .map(|b| Archive::parse(b))
            .collect::<Result<_, _>>()?;
        let mut shapes = Vec::new();
        for i in 0..116 {
            let id = shape_id(i).unwrap();
            let bytes = archives
                .iter()
                .find_map(|a| a.get(&id))
                .ok_or_else(|| format!("Missing scene shape {:?}", id))?;
            shapes.push(Shape::parse(bytes).map_err(|e| format!("Scene shape {:?}: {e}", id))?);
        }
        let mut scene = Self {
            triangles: Vec::new(),
            buckets: vec![Vec::new(); 900],
        };
        for y in 0..30 {
            for x in 0..30 {
                let tid = track.terrain(x, y).unwrap();
                let raw_id = track.element(x, y).unwrap();
                let slope_id = if (7..=10).contains(&tid) {
                    hillroad(tid, raw_id)
                } else {
                    0
                };
                let origin = [(x as f32 + 0.5) * TILE, 0.0, (y as f32 + 0.5) * TILE];
                // The original renderer uses terrain 6 as a 450-unit plateau;
                // other terrain records contain their own slope/coast geometry.
                // Record zero is a fence helper, not the empty terrain mesh.
                // frame.c only submits nonzero terrain IDs. Supply the game's
                // global grass background as per-tile ground triangles instead.
                let x0 = x as f32 * TILE;
                let z0 = y as f32 * TILE;
                for points in [
                    [[x0, 0.0, z0], [x0 + TILE, 0.0, z0], [x0, 0.0, z0 + TILE]],
                    [
                        [x0 + TILE, 0.0, z0],
                        [x0 + TILE, 0.0, z0 + TILE],
                        [x0, 0.0, z0 + TILE],
                    ],
                ] {
                    scene.triangles.push(Triangle {
                        points,
                        color: 0x498147,
                        surface: 0,
                        solid: true,
                    });
                }
                if tid != 0 && slope_id == 0 {
                    if let Some(meta) = terrain(tid) {
                        scene.place(&shapes, meta, origin, true);
                    }
                }
                let id = if slope_id != 0 { slope_id } else { raw_id };
                if id == 0 || id >= 0xfd {
                    continue;
                }
                if let Some(meta) = object(id) {
                    let origin = [
                        (x as f32 + if meta.multi & 2 != 0 { 1.0 } else { 0.5 }) * TILE,
                        if tid == 6 { 450.0 * SCALE } else { 0.0 },
                        (y as f32 + if meta.multi & 1 != 0 { 0.0 } else { 0.5 }) * TILE,
                    ];
                    scene.place(&shapes, meta, origin, false);
                    if meta.overlay != 0 {
                        if let Some(overlay) = object(meta.overlay) {
                            scene.place(&shapes, overlay, origin, false);
                        }
                    }
                }
            }
        }
        scene.index();
        Ok(scene)
    }

    fn place(&mut self, shapes: &[Shape], meta: Object, origin: [f32; 3], ground: bool) {
        let Some(shape) = shapes.get(meta.shape) else {
            return;
        };
        let rotation = matrix::rotation_y(meta.angle);
        let vertices: Vec<_> = shape
            .vertices
            .iter()
            .map(|v| {
                let v = matrix::transform(&rotation, v);
                [
                    origin[0] + v[0] as f32 * SCALE,
                    origin[1] + v[1] as f32 * SCALE,
                    origin[2] + v[2] as f32 * SCALE,
                ]
            })
            .collect();
        for primitive in &shape.primitives {
            if !(3..=10).contains(&primitive.kind) {
                continue;
            }
            let material = primitive
                .materials
                .get(meta.paint as usize)
                .or(primitive.materials.first())
                .copied()
                .unwrap_or(0);
            // Road paint indices were confirmed in the original road/ramp
            // meshes. Decoration and stripe polygons do not define a contact.
            let road = matches!(material, 19 | 20 | 22 | 23 | 25 | 26 | 28 | 29);
            let surface = if road {
                match meta.paint {
                    1 => 2,
                    2 => 3,
                    _ => 1,
                }
            } else {
                0
            };
            for i in 1..primitive.indices.len() - 1 {
                let points = [
                    vertices[primitive.indices[0] as usize],
                    vertices[primitive.indices[i] as usize],
                    vertices[primitive.indices[i + 1] as usize],
                ];
                self.triangles.push(Triangle {
                    points,
                    color: color(material),
                    surface,
                    solid: ground || road || !matches!(material, 21 | 24 | 27 | 30),
                });
            }
        }
    }

    fn index(&mut self) {
        for (i, t) in self.triangles.iter().enumerate() {
            let minx = t.points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
            let maxx = t
                .points
                .iter()
                .map(|p| p[0])
                .fold(f32::NEG_INFINITY, f32::max);
            let minz = t.points.iter().map(|p| p[2]).fold(f32::INFINITY, f32::min);
            let maxz = t
                .points
                .iter()
                .map(|p| p[2])
                .fold(f32::NEG_INFINITY, f32::max);
            for z in (minz / TILE).floor().clamp(0.0, 29.0) as usize
                ..=(maxz / TILE).floor().clamp(0.0, 29.0) as usize
            {
                for x in (minx / TILE).floor().clamp(0.0, 29.0) as usize
                    ..=(maxx / TILE).floor().clamp(0.0, 29.0) as usize
                {
                    self.buckets[z * 30 + x].push(i);
                }
            }
        }
    }

    pub fn nearby(&self, x: f32, z: f32, radius: usize) -> Vec<usize> {
        let cx = (x / TILE).floor().clamp(0.0, 29.0) as usize;
        let cz = (z / TILE).floor().clamp(0.0, 29.0) as usize;
        let mut indices = Vec::new();
        for row in cz.saturating_sub(radius)..=(cz + radius).min(29) {
            for col in cx.saturating_sub(radius)..=(cx + radius).min(29) {
                indices.extend_from_slice(&self.buckets[row * 30 + col]);
            }
        }
        indices.sort_unstable();
        indices.dedup();
        indices
    }

    /// Highest supporting triangle below the wheel's reach. Keeping the ceiling
    /// bounded allows a car under a bridge to stay on the lower road.
    pub fn contact(&self, x: f32, z: f32, ceiling: f32) -> Contact {
        let mut hit = Contact {
            height: 0.0,
            surface: 0,
        };
        for &i in &self.buckets[(z / TILE).floor().clamp(0.0, 29.0) as usize * 30
            + (x / TILE).floor().clamp(0.0, 29.0) as usize]
        {
            let t = &self.triangles[i];
            if !t.solid {
                continue;
            }
            let [a, b, c] = t.points;
            let area = (b[0] - a[0]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[0] - a[0]);
            if area.abs() < 0.0001 {
                continue;
            }
            let u = ((x - a[0]) * (c[2] - a[2]) - (z - a[2]) * (c[0] - a[0])) / area;
            let v = ((b[0] - a[0]) * (z - a[2]) - (b[2] - a[2]) * (x - a[0])) / area;
            if u >= -0.001 && v >= -0.001 && u + v <= 1.001 {
                let height = a[1] + u * (b[1] - a[1]) + v * (c[1] - a[1]);
                if height <= ceiling && height >= hit.height - 0.001 {
                    hit = Contact {
                        height,
                        surface: t.surface,
                    };
                }
            }
        }
        hit
    }

    /// Sweep three points across near-vertical mesh faces. This prevents simple
    /// wall tunnelling, but remains an approximation of the car's collision hull.
    pub fn obstructed(&self, from: [f32; 3], to: [f32; 3]) -> bool {
        let candidates = self.nearby((from[0] + to[0]) * 0.5, (from[2] + to[2]) * 0.5, 1);
        for lateral in [-0.7, 0.0, 0.7] {
            let dx = to[0] - from[0];
            let dz = to[2] - from[2];
            let length = dx.hypot(dz);
            if length < 0.0001 {
                return false;
            }
            let a = [
                from[0] - dz / length * lateral,
                from[1] + 0.15,
                from[2] + dx / length * lateral,
            ];
            let b = [
                to[0] - dz / length * lateral,
                to[1] + 0.15,
                to[2] + dx / length * lateral,
            ];
            for &i in &candidates {
                let t = &self.triangles[i];
                if !t.solid {
                    continue;
                }
                let normal = cross(sub(t.points[1], t.points[0]), sub(t.points[2], t.points[0]));
                if normal[1].abs() > 0.5 * dot(normal, normal).sqrt() {
                    continue;
                }
                if segment_triangle(a, b, t.points) {
                    return true;
                }
            }
        }
        false
    }
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn segment_triangle(a: [f32; 3], b: [f32; 3], t: [[f32; 3]; 3]) -> bool {
    let direction = sub(b, a);
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let h = cross(direction, e2);
    let det = dot(e1, h);
    if det.abs() < 0.00001 {
        return false;
    }
    let s = sub(a, t[0]);
    let u = dot(s, h) / det;
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = cross(s, e1);
    let v = dot(direction, q) / det;
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    let distance = dot(e2, q) / det;
    (0.0001..=1.0).contains(&distance)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slope_substitution_matches_every_original_byte_pair() {
        let expected = include_bytes!("../tests/fixtures/hillroad-dos.bin");
        assert_eq!(expected.len(), 65536);
        for terrain in 0..=255 {
            for element in 0..=255 {
                assert_eq!(
                    hillroad(terrain, element),
                    expected[terrain as usize * 256 + element as usize],
                    "terrain {terrain}, element {element}"
                );
            }
        }
    }
    #[test]
    fn loads_original_shapes_and_preserves_metadata() {
        assert_eq!(shape_id(object(4).unwrap().shape), Some(*b"road"));
        assert_eq!(object(5).unwrap().angle, 256);
        assert_eq!(shape_id(object(0x24).unwrap().shape), Some(*b"ramp"));
        let bytes = include_bytes!("../../stunts_dos/HELL5.TRK");
        let scene = Scene::load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos"),
            &Track::parse(bytes).unwrap(),
        )
        .unwrap();
        assert!(scene.triangles.len() > 1000);
        assert!(shape_id(usize::MAX).is_none());
        assert!(object(255).is_none());
        assert!(terrain(19).is_none());
    }
    #[test]
    fn original_ramp_contacts_have_height_and_empty_terrain_has_no_fence() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let mut bytes = [0u8; 1802];
        bytes[15 * 30 + 15] = 0x24;
        let scene = Scene::load(&root, &Track::parse(&bytes).unwrap()).unwrap();
        let low = scene.contact(15.0 * TILE + 10.0, 15.5 * TILE, 100.0);
        let high = scene.contact(15.0 * TILE + 50.0, 15.5 * TILE, 100.0);
        assert!(
            high.height > low.height + 10.0,
            "low {low:?}, high {high:?}"
        );
        assert_eq!(low.surface, 1);
        assert!(!scene.obstructed([100.0, 0.7, 100.0], [180.0, 0.7, 100.0]));
        bytes[15 * 30 + 15] = 4;
        bytes[901 + (29 - 15) * 30 + 15] = 7;
        let scene = Scene::load(&root, &Track::parse(&bytes).unwrap()).unwrap();
        let low = scene.contact(15.5 * TILE, 15.0 * TILE + 10.0, 100.0);
        let high = scene.contact(15.5 * TILE, 15.0 * TILE + 50.0, 100.0);
        assert!(
            high.height > low.height + 10.0,
            "inclined road: {low:?} {high:?}"
        );
        assert_eq!(high.surface, 1);
    }
    #[test]
    fn contact_selects_lower_road_under_bridge_and_sweep_catches_walls() {
        let mut scene = Scene {
            triangles: vec![],
            buckets: vec![vec![]; 900],
        };
        for height in [0.0, 10.0] {
            scene.triangles.push(Triangle {
                points: [[0.0, height, 0.0], [60.0, height, 0.0], [0.0, height, 60.0]],
                color: 0,
                surface: 1,
                solid: true,
            });
        }
        scene.triangles.push(Triangle {
            points: [[20.0, 0.0, 0.0], [20.0, 20.0, 0.0], [20.0, 0.0, 60.0]],
            color: 0,
            surface: 0,
            solid: true,
        });
        scene.index();
        assert_eq!(scene.contact(10.0, 10.0, 2.0).height, 0.0);
        assert_eq!(scene.contact(10.0, 10.0, 12.0).height, 10.0);
        assert!(scene.obstructed([10.0, 1.0, 10.0], [40.0, 1.0, 10.0]));
        assert!(!scene.obstructed([10.0, 30.0, 10.0], [40.0, 30.0, 10.0]));
    }
}
