//! GPU triangle rendering. Original fixed-point simulation remains CPU-side.
use bytemuck::{Pod, Zeroable};
use eframe::{
    egui,
    egui_wgpu::RenderState,
    wgpu::{self, util::DeviceExt},
};
use glam::{Mat4, Vec3};
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
}
struct Mesh {
    buffer: wgpu::Buffer,
    count: u32,
}

pub struct Gpu {
    state: RenderState,
    pipeline: wgpu::RenderPipeline,
    scene_uniform: wgpu::Buffer,
    car_uniform: wgpu::Buffer,
    scene_group: wgpu::BindGroup,
    car_group: wgpu::BindGroup,
    scene: Option<Mesh>,
    car: Option<Mesh>,
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
            scene_uniform,
            car_uniform,
            scene_group,
            car_group,
            scene: None,
            car: None,
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
                    usage: wgpu::BufferUsages::VERTEX,
                }),
            count: vertices.len() as u32,
        };
        self.scene = Some(mesh(&scene, "Stunts static scene"));
        self.car = Some(mesh(&car, "Stunts local car"));
    }
    pub fn render(&mut self, world: &World, size: [u32; 2], lighting: bool) {
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
        let uniforms = |model: Mat4| Uniforms {
            view_projection: view_projection.to_cols_array_2d(),
            model: model.to_cols_array_2d(),
            lighting: [u8::from(lighting) as f32, 0.0, 0.0, 0.0],
        };
        self.state.queue.write_buffer(
            &self.scene_uniform,
            0,
            bytemuck::bytes_of(&uniforms(Mat4::IDENTITY)),
        );
        self.state
            .queue
            .write_buffer(&self.car_uniform, 0, bytemuck::bytes_of(&uniforms(model)));
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
            pass.set_pipeline(&self.pipeline);
            for (mesh, group) in [
                (&self.scene, &self.scene_group),
                (&self.car, &self.car_group),
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
        if p.kind == 12 {
            for q in [[0, 1, 2, 3], [3, 4, 5, 0]] {
                triangle(&mut vertices, [v(q[0]), v(q[1]), v(q[2])], 0x20212a);
                triangle(&mut vertices, [v(q[0]), v(q[2]), v(q[3])], 0x20212a);
            }
        }
        if (3..=10).contains(&p.kind) {
            let color = palette[p.materials.first().copied().unwrap_or(0) as usize % palette.len()];
            for i in 1..p.indices.len() - 1 {
                triangle(&mut vertices, [v(0), v(i), v(i + 1)], color);
            }
        }
    }
    vertices
}
#[cfg(test)]
mod tests {
    use super::*;
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
