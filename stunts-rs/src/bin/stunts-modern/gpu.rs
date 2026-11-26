//! GPU triangle rendering. Original fixed-point simulation remains CPU-side.
use bytemuck::{Pod, Zeroable};
use eframe::{
    egui,
    egui_wgpu::RenderState,
    wgpu::{self, util::DeviceExt},
};
use glam::{Mat4, Quat, Vec3};
use stunts_rs::{prototype::World, session::Session};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    view_projection: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    lighting: [f32; 4],
    inverse_view_projection: [[f32; 4]; 4],
    atmosphere: [f32; 4],
}
struct Mesh {
    buffer: wgpu::Buffer,
    count: u32,
}

pub struct Gpu {
    state: RenderState,
    pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    clock: (f32, std::time::Instant),
    scene_uniform: wgpu::Buffer,
    car_uniform: wgpu::Buffer,
    scene_group: wgpu::BindGroup,
    car_group: wgpu::BindGroup,
    scene: Option<Mesh>,
    car: Option<Mesh>,
    wheels: Option<Mesh>,
    wheel_specs: Vec<Wheel>,
    color: wgpu::Texture,
    depth: wgpu::TextureView,
    size: [u32; 2],
    pub texture: egui::TextureId,
    pub adapter: String,
}
impl Gpu {
    pub fn new(state: RenderState) -> Self {
        let device = &state.device;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Stunts camera/model layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform = |label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: std::mem::size_of::<Uniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let scene_uniform = uniform("Stunts scene uniforms");
        let car_uniform = uniform("Stunts car uniforms");
        let group = |buffer: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        };
        let scene_group = group(&scene_uniform);
        let car_group = group(&car_uniform);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Stunts pipeline"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Stunts flat shading"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu.wgsl").into()),
        });
        let attributes = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x3];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Stunts original triangle meshes"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Time of day sky and clouds"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("sky_vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("sky_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let (color, depth) = targets(device, [1, 1]);
        let texture = state.renderer.write().register_native_texture(
            device,
            &color.create_view(&Default::default()),
            wgpu::FilterMode::Linear,
        );
        let info = state.adapter.get_info();
        let adapter = format!("{} ({:?})", info.name, info.backend);
        Self {
            state,
            pipeline,
            sky_pipeline,
            clock: (local_seconds(), std::time::Instant::now()),
            scene_uniform,
            car_uniform,
            scene_group,
            car_group,
            scene: None,
            car: None,
            wheels: None,
            wheel_specs: Vec::new(),
            color,
            depth,
            size: [1, 1],
            texture,
            adapter,
        }
    }
    pub fn load(&mut self, session: &Session) {
        let scene = scene_vertices(session);
        let car = car_vertices(session);
        let mesh = |vertices: &[Vertex], label| Mesh {
            buffer: self
                .state
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: bytemuck::cast_slice(vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                }),
            count: vertices.len() as u32,
        };
        self.scene = Some(mesh(&scene, "Stunts static scene"));
        self.car = Some(mesh(&car, "Stunts local car"));
        self.wheel_specs = wheels(session);
        self.wheels = Some(mesh(
            &wheel_vertices(&self.wheel_specs, &session.world),
            "Animated tires and rims",
        ));
    }
    pub fn render(
        &mut self,
        world: &World,
        size: [u32; 2],
        lighting: bool,
        time_of_day: Option<f32>,
    ) {
        let size = size.map(|n| n.clamp(1, 4096));
        if size != self.size {
            let (color, depth) = targets(&self.state.device, size);
            self.color = color;
            self.depth = depth;
            self.size = size;
            self.state
                .renderer
                .write()
                .update_egui_texture_from_wgpu_texture(
                    &self.state.device,
                    &self.color.create_view(&Default::default()),
                    wgpu::FilterMode::Linear,
                    self.texture,
                );
        }
        let (view_projection, model) = camera(world, size[0] as f32 / size[1] as f32);
        let seconds = (self.clock.0 + self.clock.1.elapsed().as_secs_f32()).rem_euclid(86400.0);
        let seconds = time_of_day
            .map(|h| h * 3600.0 + self.clock.1.elapsed().as_secs_f32())
            .unwrap_or(seconds)
            .rem_euclid(86400.0);
        let uniforms = |model: Mat4| Uniforms {
            view_projection: view_projection.to_cols_array_2d(),
            model: model.to_cols_array_2d(),
            lighting: [u8::from(lighting) as f32, 0.0, 0.0, 0.0],
            inverse_view_projection: view_projection.inverse().to_cols_array_2d(),
            atmosphere: [seconds / 3600.0, seconds, 0.0, 0.0],
        };
        self.state.queue.write_buffer(
            &self.scene_uniform,
            0,
            bytemuck::bytes_of(&uniforms(Mat4::IDENTITY)),
        );
        self.state
            .queue
            .write_buffer(&self.car_uniform, 0, bytemuck::bytes_of(&uniforms(model)));
        if let Some(mesh) = &self.wheels {
            self.state.queue.write_buffer(
                &mesh.buffer,
                0,
                bytemuck::cast_slice(&wheel_vertices(&self.wheel_specs, world)),
            );
        }
        let mut encoder =
            self.state
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Stunts GPU frame"),
                });
        {
            let view = self.color.create_view(&Default::default());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Stunts scene and car"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.51,
                            g: 0.72,
                            b: 0.85,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.sky_pipeline);
            pass.set_bind_group(0, &self.scene_group, &[]);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&self.pipeline);
            for (mesh, group) in [
                (&self.scene, &self.scene_group),
                (&self.car, &self.car_group),
                (&self.wheels, &self.car_group),
            ] {
                if let Some(mesh) = mesh {
                    pass.set_bind_group(0, group, &[]);
                    pass.set_vertex_buffer(0, mesh.buffer.slice(..));
                    pass.draw(0..mesh.count, 0..1);
                }
            }
        }
        self.state.queue.submit([encoder.finish()]);
    }
}
fn targets(device: &wgpu::Device, size: [u32; 2]) -> (wgpu::Texture, wgpu::TextureView) {
    let create = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Stunts offscreen target"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let color = create(
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let depth = create(
        wgpu::TextureFormat::Depth32Float,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    )
    .create_view(&Default::default());
    (color, depth)
}
fn camera(world: &World, aspect: f32) -> (Mat4, Mat4) {
    let position = Vec3::from_array(world.position);
    let forward = Vec3::new(world.heading.sin(), 0.0, -world.heading.cos());
    let view = Mat4::look_at_rh(
        position - forward * 8.0 + Vec3::Y * 2.8,
        position + Vec3::Y * 0.6,
        Vec3::Y,
    );
    let projection = Mat4::perspective_rh(50.0_f32.to_radians(), aspect, 0.1, 2500.0);
    let model = Mat4::from_translation(position - Vec3::Y * 0.65)
        * Mat4::from_rotation_y(-world.heading)
        * Mat4::from_rotation_z(world.roll)
        * Mat4::from_rotation_x(-world.pitch);
    (projection * view, model)
}
fn triangle(vertices: &mut Vec<Vertex>, points: [[f32; 3]; 3], color: u32) {
    let [a, b, c] = points.map(Vec3::from_array);
    let normal = (b - a).cross(c - a).normalize_or_zero();
    if normal.length_squared() < 0.5 {
        return;
    }
    let color = [
        ((color >> 16) & 255) as f32 / 255.0,
        ((color >> 8) & 255) as f32 / 255.0,
        (color & 255) as f32 / 255.0,
    ];
    for position in points {
        vertices.push(Vertex {
            position,
            normal: normal.to_array(),
            color,
        });
    }
}
fn scene_vertices(session: &Session) -> Vec<Vertex> {
    let mut vertices = Vec::with_capacity(session.scene.triangles.len() * 3);
    for t in &session.scene.triangles {
        // Graphics-only separation prevents coplanar grass/road/stripe fighting;
        // original geometry used for physical contacts remains unchanged.
        let lift = if !t.solid {
            0.012
        } else if t.surface != 0 {
            0.006
        } else {
            0.0
        };
        let points = t.points.map(|mut p| {
            p[1] += lift;
            p
        });
        triangle(&mut vertices, points, t.color);
    }
    vertices
}
fn car_vertices(session: &Session) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let points: Vec<_> = session
        .shape
        .vertices
        .iter()
        .map(|v| {
            [
                -v[0] as f32 * 0.0025,
                v[1] as f32 * 0.0025,
                -v[2] as f32 * 0.0025,
            ]
        })
        .collect();
    let palette = [
        0xdf3030, 0xf8d143, 0xd7e2e9, 0x252733, 0xd94732, 0x4d5361, 0xeee8da, 0x1e2838,
    ];
    for p in &session.shape.primitives {
        let v = |i: usize| points[p.indices[i] as usize];
        if (3..=10).contains(&p.kind) {
            let color = palette[p.materials.first().copied().unwrap_or(0) as usize % palette.len()];
            for i in 1..p.indices.len() - 1 {
                triangle(&mut vertices, [v(0), v(i), v(i + 1)], color);
            }
        }
    }
    vertices
}

struct Wheel {
    center: Vec3,
    axis: Vec3,
    radius: f32,
    width: f32,
    front: bool,
    rolling: bool,
}
fn wheels(session: &Session) -> Vec<Wheel> {
    session
        .shape
        .primitives
        .iter()
        .filter(|p| p.kind == 12)
        .map(|p| {
            let v = |i: usize| {
                let v = session.shape.vertices[p.indices[i] as usize];
                Vec3::new(-v[0] as f32, v[1] as f32, -v[2] as f32) * 0.0025
            };
            // Original type 12 stores each axle endpoint, radial-up point and radial-side point.
            let a = v(0);
            let b = v(3);
            let center = (a + b) * 0.5;
            let axis = (b - a).normalize();
            Wheel {
                center,
                axis,
                radius: (v(1) - a).length(),
                width: (b - a).length(),
                front: center.z < 0.0,
                rolling: axis.x.abs() > 0.8,
            }
        })
        .collect()
}
fn wheel_transform(wheel: &Wheel, world: &World) -> Mat4 {
    let steer = world.state.word(stunts_rs::grip::STEERING) as i16 as f32 / 240.0 * 0.48;
    // Use a consistent positive-X axle so both sides roll in the same direction.
    let axis = if wheel.axis.x < 0.0 {
        -wheel.axis
    } else {
        wheel.axis
    };
    let turn = if wheel.front && wheel.rolling {
        -steer
    } else {
        0.0
    };
    let spin = if wheel.rolling {
        -world.wheel_travel / wheel.radius
    } else {
        0.0
    };
    Mat4::from_translation(wheel.center)
        * Mat4::from_rotation_y(turn)
        * Mat4::from_quat(Quat::from_rotation_arc(Vec3::X, axis))
        * Mat4::from_rotation_x(spin)
}
fn wheel_vertices(wheels: &[Wheel], world: &World) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    for wheel in wheels {
        let transform = wheel_transform(wheel, world);
        let r = wheel.radius;
        let w = wheel.width;
        let point = |x: f32, radius: f32, angle: f32| {
            transform
                .transform_point3(Vec3::new(x, radius * angle.cos(), radius * angle.sin()))
                .to_array()
        };
        // Rounded shoulders, sidewalls and recessed circumferential tread grooves.
        let rings: [(f32, f32); 14] = [
            (-0.5, 0.57),
            (-0.5, 0.84),
            (-0.43, 0.96),
            (-0.32, 1.0),
            (-0.16, 1.0),
            (-0.12, 0.98),
            (-0.08, 1.0),
            (0.08, 1.0),
            (0.12, 0.98),
            (0.16, 1.0),
            (0.32, 1.0),
            (0.43, 0.96),
            (0.5, 0.84),
            (0.5, 0.57),
        ];
        for i in 0..40 {
            let a = i as f32 * std::f32::consts::TAU / 40.0;
            let b = (i + 1) as f32 * std::f32::consts::TAU / 40.0;
            for band in rings.windows(2) {
                let [(x0, r0), (x1, r1)] = [band[0], band[1]];
                let tread = x0.abs() < 0.33 && x1.abs() < 0.33;
                let color = if tread && (i % 5 == 0 || r0 < 0.99 || r1 < 0.99) {
                    0x151619
                } else if tread {
                    0x303136
                } else {
                    0x222327
                };
                let q = [
                    point(x0 * w, r0 * r, a),
                    point(x1 * w, r1 * r, a),
                    point(x1 * w, r1 * r, b),
                    point(x0 * w, r0 * r, b),
                ];
                triangle(&mut vertices, [q[0], q[1], q[2]], color);
                triangle(&mut vertices, [q[0], q[2], q[3]], color);
            }
            for side in [-1.0, 1.0] {
                let x = side * w * 0.505;
                // Five alloy spokes over a dark brake recess; bright rim lip and central hub.
                for (inner, outer, color) in [
                    (0.52, 0.57, 0xb4bac3),
                    (0.17, 0.52, if i % 8 < 2 { 0xa3abb7 } else { 0x343840 }),
                    (0.0, 0.17, 0x737c8b),
                ] {
                    let q = [
                        point(x, inner * r, a),
                        point(x, outer * r, a),
                        point(x, outer * r, b),
                        point(x, inner * r, b),
                    ];
                    triangle(&mut vertices, [q[0], q[1], q[2]], color);
                    triangle(&mut vertices, [q[0], q[2], q[3]], color);
                }
            }
        }
    }
    vertices
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tires_fit_all_cars_and_follow_simulated_travel_and_steering() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let catalog = stunts_rs::frontend::Catalog::load(&root).unwrap();
        for i in 0..catalog.cars.len() {
            let mut session = Session::load(&root, &catalog, 0, i).unwrap();
            let specs = wheels(&session);
            assert_eq!(specs.iter().filter(|w| w.rolling).count(), 4);
            assert_eq!(specs.iter().filter(|w| w.rolling && w.front).count(), 2);
            let vertices = wheel_vertices(&specs, &session.world);
            assert!(vertices.len() > 10000);
            assert!(vertices.iter().all(|v| v
                .position
                .iter()
                .chain(&v.normal)
                .all(|x| x.is_finite())));
            let front = specs.iter().find(|w| w.front && w.rolling).unwrap();
            let rear = specs.iter().find(|w| !w.front && w.rolling).unwrap();
            let initial = wheel_transform(front, &session.world);
            session.world.wheel_travel = front.radius;
            let moved = wheel_transform(front, &session.world);
            assert!(
                (initial.transform_vector3(Vec3::Y) - moved.transform_vector3(Vec3::Y)).length()
                    > 0.5
            );
            let rear_before = wheel_transform(rear, &session.world);
            session.world.state.set_word(stunts_rs::grip::STEERING, 120);
            let turned = wheel_transform(front, &session.world);
            assert!(
                (turned.transform_vector3(Vec3::X) - moved.transform_vector3(Vec3::X)).length()
                    > 0.1
            );
            assert_eq!(rear_before, wheel_transform(rear, &session.world));
            assert!((turned.transform_point3(Vec3::ZERO) - front.center).length() < 0.0001);
            session.reset().unwrap();
            assert_eq!(session.world.wheel_travel, 0.0);
            assert_eq!(initial, wheel_transform(front, &session.world));
            if i == 0 {
                let mut distance = 0.0;
                for _ in 0..40 {
                    let before = session.world.position;
                    session.tick(1).unwrap();
                    distance += (session.world.position[0] - before[0])
                        .hypot(session.world.position[2] - before[2]);
                }
                assert!(distance > 0.0);
                assert!((session.world.wheel_travel - distance).abs() < 0.0001);
                session.reset().unwrap();
                assert_eq!(session.world.wheel_travel, 0.0);
            }
        }
    }
    #[test]
    fn original_meshes_upload_as_finite_triangles_and_camera_follows_heading() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../stunts_dos");
        let catalog = stunts_rs::frontend::Catalog::load(&root).unwrap();
        let mut session = Session::load(&root, &catalog, 0, 0).unwrap();
        let scene = scene_vertices(&session);
        let car = car_vertices(&session);
        assert!(scene.len() > 3000 && car.len() > 30);
        assert!(scene.iter().chain(&car).all(|v| v
            .position
            .iter()
            .chain(&v.normal)
            .all(|x| x.is_finite())));
        for angle in [0.0, 1.0, 3.0, 5.0] {
            session.world.heading = angle;
            let (vp, model) = camera(&session.world, 1.6);
            let point = vp.project_point3(Vec3::from_array(session.world.position));
            assert!(point.x.abs() < 0.001 && point.z > 0.0 && point.z < 1.0);
            assert!(model.is_finite());
        }
    }
}

// Anchor the animation to the computer's local civil clock.
#[cfg(windows)]
fn local_seconds() -> f32 {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        weekday: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        millis: u16,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocalTime(time: *mut SystemTime);
    }
    let mut time = SystemTime::default();
    // The OS writes one SYSTEMTIME to this valid, correctly sized pointer.
    unsafe {
        GetLocalTime(&mut time);
    }
    time.hour as f32 * 3600.0 + time.minute as f32 * 60.0 + time.second as f32
}
#[cfg(not(windows))]
fn local_seconds() -> f32 {
    let local = std::process::Command::new("date")
        .arg("+%H %M %S")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok());
    if let Some(local) = local {
        let parts: Vec<f32> = local
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        if parts.len() == 3 {
            return parts[0] * 3600.0 + parts[1] * 60.0 + parts[2];
        }
    }
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        % 86400) as f32
}
