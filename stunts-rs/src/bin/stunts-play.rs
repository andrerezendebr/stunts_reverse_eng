//! Native software-rendered driving experiment, separate from verified DOS physics.
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use stunts_rs::{
    car::Car,
    compression, drivetrain,
    prototype::{World, STEP, TILE},
    resource::Archive,
    scene::Scene,
    shape::Shape,
    track::Track,
};
const W: usize = 960;
const H: usize = 600;

struct Renderer {
    pixels: Vec<u32>,
    depth: Vec<f32>,
    camera: [f32; 3],
    heading: f32,
}
impl Renderer {
    fn new() -> Self {
        Self {
            pixels: vec![0; W * H],
            depth: vec![f32::INFINITY; W * H],
            camera: [0.0; 3],
            heading: 0.0,
        }
    }
    fn camera_point(&self, p: [f32; 3]) -> [f32; 3] {
        let dx = p[0] - self.camera[0];
        let dy = p[1] - self.camera[1];
        let dz = p[2] - self.camera[2];
        let x = dx * self.heading.cos() + dz * self.heading.sin();
        let forward = dx * self.heading.sin() - dz * self.heading.cos();
        // A fixed downward chase-camera pitch keeps the road in view.
        let y = dy * 0.96 + forward * 0.28;
        let z = forward * 0.96 - dy * 0.28;
        [x, y, z]
    }
    fn triangle(&mut self, pts: [[f32; 3]; 3], color: u32) {
        let camera = pts.map(|p| self.camera_point(p));
        let mut clipped = Vec::with_capacity(4);
        // Clip in camera space before perspective division. A triangle crossing
        // the near plane becomes a triangle or quad instead of disappearing.
        for i in 0..3 {
            let a = camera[i];
            let b = camera[(i + 1) % 3];
            if a[2] >= 0.25 {
                clipped.push(a);
            }
            if (a[2] >= 0.25) != (b[2] >= 0.25) {
                let t = (0.25 - a[2]) / (b[2] - a[2]);
                clipped.push(std::array::from_fn(|axis| {
                    a[axis] + t * (b[axis] - a[axis])
                }));
            }
        }
        let project = |p: [f32; 3]| {
            [
                W as f32 / 2.0 + p[0] * 650.0 / p[2],
                H as f32 * 0.47 - p[1] * 650.0 / p[2],
                1.0 / p[2],
            ]
        };
        for i in 1..clipped.len().saturating_sub(1) {
            self.raster(
                [
                    project(clipped[0]),
                    project(clipped[i]),
                    project(clipped[i + 1]),
                ],
                color,
            );
        }
    }
    fn raster(&mut self, [a, b, c]: [[f32; 3]; 3], color: u32) {
        let edge = |p: [f32; 3], q: [f32; 3], x: f32, y: f32| {
            (x - p[0]) * (q[1] - p[1]) - (y - p[1]) * (q[0] - p[0])
        };
        let area = edge(a, b, c[0], c[1]);
        if area.abs() < 0.01 {
            return;
        }
        let xmin = a[0].min(b[0]).min(c[0]).floor().clamp(0.0, (W - 1) as f32) as usize;
        let xmax = a[0].max(b[0]).max(c[0]).ceil().clamp(0.0, (W - 1) as f32) as usize;
        let ymin = a[1].min(b[1]).min(c[1]).floor().clamp(0.0, (H - 1) as f32) as usize;
        let ymax = a[1].max(b[1]).max(c[1]).ceil().clamp(0.0, (H - 1) as f32) as usize;
        for y in ymin..=ymax {
            for x in xmin..=xmax {
                let u = edge(b, c, x as f32 + 0.5, y as f32 + 0.5) / area;
                let v = edge(c, a, x as f32 + 0.5, y as f32 + 0.5) / area;
                let t = 1.0 - u - v;
                if u >= 0.0 && v >= 0.0 && t >= 0.0 {
                    let z = 1.0 / (u * a[2] + v * b[2] + t * c[2]);
                    let i = y * W + x;
                    // Original roads and markings are coplanar with terrain.
                    // A small depth tolerance lets their later draw order win
                    // without changing geometry used by suspension contacts.
                    if z < self.depth[i] + 0.01 {
                        self.depth[i] = z;
                        self.pixels[i] = color;
                    }
                }
            }
        }
    }
    fn quad(&mut self, p: [[f32; 3]; 4], color: u32) {
        self.triangle([p[0], p[1], p[2]], color);
        self.triangle([p[0], p[2], p[3]], color);
    }
    fn render(&mut self, world: &World, track: &Track<'_>, shape: &Shape, scene: &Scene) {
        self.pixels.fill(0x82b8d8);
        self.depth.fill(f32::INFINITY);
        self.heading = world.heading;
        self.camera = [
            world.position[0] - world.heading.sin() * 11.0,
            world.position[1] + 4.5,
            world.position[2] + world.heading.cos() * 11.0,
        ];
        for index in scene.nearby(world.position[0], world.position[2], 6) {
            let triangle = &scene.triangles[index];
            self.triangle(triangle.points, triangle.color);
        }
        // Mesh vertices are original signed DOS coordinates. Only model scale,
        // palette and the chase-camera transform are provisional.
        let vertices: Vec<_> = shape
            .vertices
            .iter()
            .map(|v| {
                let x = v[0] as f32 * 0.0025;
                let y = v[1] as f32 * 0.0025;
                let z = v[2] as f32 * 0.0025;
                let py = y * world.pitch.cos() - z * world.pitch.sin();
                let pz = y * world.pitch.sin() + z * world.pitch.cos();
                let px = x * world.roll.cos() - py * world.roll.sin();
                let py = x * world.roll.sin() + py * world.roll.cos();
                [
                    world.position[0] + px * world.heading.cos() - pz * world.heading.sin(),
                    world.position[1] - 0.65 + py,
                    world.position[2] + px * world.heading.sin() + pz * world.heading.cos(),
                ]
            })
            .collect();
        let palette = [
            0xdf3030, 0xf8d143, 0xd7e2e9, 0x252733, 0xd94732, 0x4d5361, 0xeee8da, 0x1e2838,
        ];
        for p in &shape.primitives {
            if p.kind == 12 {
                // The original renderer turns this six-point primitive into a
                // specialized wheel. Show its two defining faces provisionally.
                let v = |i: usize| vertices[p.indices[i] as usize];
                self.quad([v(0), v(1), v(2), v(3)], 0x20212a);
                self.quad([v(3), v(4), v(5), v(0)], 0x292b35);
            }
            if (3..=10).contains(&p.kind) {
                let color =
                    palette[p.materials.first().copied().unwrap_or(0) as usize % palette.len()];
                for i in 1..p.indices.len() - 1 {
                    self.triangle(
                        [
                            vertices[p.indices[0] as usize],
                            vertices[p.indices[i] as usize],
                            vertices[p.indices[i + 1] as usize],
                        ],
                        color,
                    );
                }
            }
        }
        // Schematic minimap and a speed bar remain legible without a font asset.
        for y in 0..30 {
            for x in 0..30 {
                let color = if track.element(x, y).unwrap() != 0 {
                    0xc0c0c0
                } else {
                    0x203d28
                };
                for dy in 0..4 {
                    for dx in 0..4 {
                        self.pixels[(12 + y * 4 + dy) * W + 12 + x * 4 + dx] = color;
                    }
                }
            }
        }
        let mx = (world.position[0] / TILE * 4.0).clamp(0.0, 119.0) as usize + 12;
        let my = (world.position[2] / TILE * 4.0).clamp(0.0, 119.0) as usize + 12;
        for y in my.saturating_sub(2)..=my + 2 {
            for x in mx.saturating_sub(2)..=mx + 2 {
                self.pixels[y * W + x] = 0xff3030;
            }
        }
        let speed = (world.state.word(drivetrain::SPEED2) as usize / 128).min(500);
        for y in H - 24..H - 14 {
            for x in 20..20 + speed {
                self.pixels[y * W + x] = 0xf2c94c;
            }
        }
    }
    fn save(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        write!(file, "P6\n{W} {H}\n255\n")?;
        let bytes: Vec<_> = self
            .pixels
            .iter()
            .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
            .collect();
        file.write_all(&bytes)?;
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let value = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
    };
    if args.iter().any(|a| a == "--help") {
        println!("stunts-play [--data PATH] [--track NAME.TRK] [--frames N] [--headless] [--screenshot FILE.ppm]\nArrows/WASD: drive and brake; R: reset; Escape: exit.\nOriginal track/car meshes; experimental triangle contacts and world physics.");
        return Ok(());
    }
    let root = value("--data")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos"));
    let track_bytes =
        std::fs::read(root.join(value("--track").map(String::as_str).unwrap_or("HELL5.TRK")))?;
    let track = Track::parse(&track_bytes)?;
    let car_bytes = std::fs::read(root.join("CARPMIN.RES"))?;
    let car = Car::parse(&car_bytes)?;
    let model_bytes = compression::decompress(&std::fs::read(root.join("STPMIN.P3S"))?)?;
    let archive = Archive::parse(&model_bytes)?;
    let shape = Shape::parse(archive.get(b"car0").ok_or("Missing car0 mesh")?)?;
    let mut world = World::new(&car, &track)?;
    let scene = Scene::load(&root, &track)?;
    world.place_on_scene(&scene);
    let mut renderer = Renderer::new();
    let limit = value("--frames").map(|n| n.parse::<usize>()).transpose()?;
    let headless = args.iter().any(|a| a == "--headless");
    if headless && limit.is_none() {
        return Err("--headless requires --frames N".into());
    }
    let mut window = if headless {
        None
    } else {
        Some(Window::new(
            "Stunts Rust experimental driving | WASD/arrows, R reset, Esc exit",
            W,
            H,
            WindowOptions {
                resize: true,
                ..WindowOptions::default()
            },
        )?)
    };
    if let Some(w) = window.as_mut() {
        w.set_target_fps(60);
    }
    let mut previous = Instant::now();
    let mut accumulator = Duration::ZERO;
    let mut frames = 0;
    loop {
        if limit.is_some_and(|n| frames >= n) {
            break;
        }
        let mut input = if headless { 1 } else { 0 };
        if let Some(w) = window.as_mut() {
            if !w.is_open() || w.is_key_down(Key::Escape) {
                break;
            }
            if w.is_key_down(Key::Up) || w.is_key_down(Key::W) {
                input |= 1;
            }
            if w.is_key_down(Key::Down) || w.is_key_down(Key::S) {
                input |= 2;
            }
            if w.is_key_down(Key::Left) || w.is_key_down(Key::A) {
                input |= 8;
            }
            if w.is_key_down(Key::Right) || w.is_key_down(Key::D) {
                input |= 4;
            }
            if w.is_key_pressed(Key::R, KeyRepeat::No) {
                world.reset(&car)?;
            }
        }
        let now = Instant::now();
        accumulator += (now - previous).min(Duration::from_millis(250));
        previous = now;
        if headless {
            world.tick_scene(&car, &track, &scene, input)?;
        } else {
            while accumulator >= Duration::from_secs_f32(STEP) {
                world.tick_scene(&car, &track, &scene, input)?;
                accumulator -= Duration::from_secs_f32(STEP);
            }
        }
        renderer.render(&world, &track, &shape, &scene);
        if let Some(w) = window.as_mut() {
            w.set_title(&format!("Stunts Rust EXPERIMENTAL | speed {:.1} raw/256 | RPM {} | gear {} | contacts {} | hits {} | WASD / R / Esc",world.state.word(drivetrain::SPEED2)as f32/256.0,world.state.word(0x22),world.state.0[0xbe],world.contacts,world.collisions));
            w.update_with_buffer(&renderer.pixels, W, H)?;
        }
        frames += 1;
    }
    if let Some(path) = value("--screenshot") {
        renderer.save(path)?;
    }
    println!(
        "Rendered {frames} frames; position {:?}; contacts {}; collisions {}",
        world.position, world.contacts, world.collisions
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn near_plane_clips_crossing_triangles_and_rejects_geometry_behind_camera() {
        let mut renderer = Renderer::new();
        renderer.triangle(
            [[-0.5, 0.0, -0.5], [0.5, 0.0, -0.5], [0.0, -0.3, 0.5]],
            0x123456,
        );
        assert!(renderer.pixels.iter().any(|&p| p == 0x123456));
        renderer.pixels.fill(0);
        renderer.depth.fill(f32::INFINITY);
        renderer.triangle(
            [[-0.5, 0.0, 1.0], [0.5, 0.0, 1.0], [0.0, 0.5, 1.0]],
            0x123456,
        );
        assert!(renderer.pixels.iter().all(|&p| p == 0));
    }
    #[test]
    fn framebuffer_contains_ground_and_original_car_and_changes_when_driving() {
        let car = Car::parse(include_bytes!("../../../stunts_dos/CARPMIN.RES")).unwrap();
        let track = Track::parse(include_bytes!("../../../stunts_dos/HELL5.TRK")).unwrap();
        let model =
            compression::decompress(include_bytes!("../../../stunts_dos/STPMIN.P3S")).unwrap();
        let archive = Archive::parse(&model).unwrap();
        let shape = Shape::parse(archive.get(b"car0").unwrap()).unwrap();
        let mut world = World::new(&car, &track).unwrap();
        let scene = Scene::load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos"),
            &track,
        )
        .unwrap();
        world.place_on_scene(&scene);
        let mut renderer = Renderer::new();
        renderer.render(&world, &track, &shape, &scene);
        assert!(renderer.pixels.iter().filter(|&&p| p == 0xdf3030).count() > 100);
        assert!(renderer.depth.iter().filter(|d| d.is_finite()).count() > 100000);
        let first = renderer.pixels.clone();
        for _ in 0..80 {
            world.tick_scene(&car, &track, &scene, 1).unwrap();
        }
        renderer.render(&world, &track, &shape, &scene);
        assert!(
            first
                .iter()
                .zip(&renderer.pixels)
                .filter(|(a, b)| a != b)
                .count()
                > 1000
        );
    }
}
