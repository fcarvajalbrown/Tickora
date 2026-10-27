use std::str::FromStr;
use std::sync::mpsc;

use wgpu::util::DeviceExt;

use crate::camera::{OrthoCamera, PerspectiveCamera};
use crate::image::Image;
use crate::scene::{Layout, PixelRect, Scene, ViewMode};
use crate::scene3::{self, Batch, Scene3};
use crate::{MAX_SIZE, MIN_SIZE};

pub const ADAPTER_ENV: &str = "TICKORA_ADAPTER";

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const VERTEX_SIZE: u64 = 12;
const FRAME_UNIFORM_SIZE: u64 = 64;

const SHADER: &str = "
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
}

@vertex
fn vs(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>) -> Out {
    return Out(vec4<f32>(position, 0.0, 1.0), color);
}

@fragment
fn fs(input: Out) -> @location(0) vec4<f32> {
    return input.color;
}
";

const MESH_SHADER: &str = "
struct Frame {
    view_projection: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
}

@vertex
fn vs(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
) -> Out {
    var out: Out;
    out.position = frame.view_projection * vec4<f32>(position, 1.0);
    out.world = position;
    out.normal = normal;
    out.uv = uv;
    out.color = color;
    return out;
}

@fragment
fn fs_unlit(input: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color.rgb, 1.0);
}
";

struct MeshPipelines {
    frame_layout: wgpu::BindGroupLayout,
    triangles: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterChoice {
    Auto,
    Hardware,
    Software,
}

impl FromStr for AdapterChoice {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        match text {
            "auto" => Ok(AdapterChoice::Auto),
            "hardware" => Ok(AdapterChoice::Hardware),
            "software" => Ok(AdapterChoice::Software),
            other => Err(format!(
                "{ADAPTER_ENV}={other:?}; expected \"auto\", \"hardware\" or \"software\""
            )),
        }
    }
}

impl AdapterChoice {
    pub fn from_env() -> Result<Self, String> {
        match std::env::var(ADAPTER_ENV) {
            Ok(text) if !text.is_empty() => text.parse(),
            _ => Ok(AdapterChoice::Auto),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterInfo {
    pub name: String,
    pub backend: String,
    pub device_type: String,
}

impl AdapterInfo {
    pub fn bucket(&self) -> String {
        format!("{}_{}", std::env::consts::OS, self.backend)
    }
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    mesh: MeshPipelines,
    info: AdapterInfo,
}

impl Renderer {
    pub fn new(choice: AdapterChoice) -> Result<Self, String> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = if cfg!(windows) {
            wgpu::Backends::DX12
        } else {
            wgpu::Backends::VULKAN
        };
        let instance = wgpu::Instance::new(descriptor);
        let adapter = match choice {
            AdapterChoice::Hardware => hardware(&instance),
            AdapterChoice::Software => software(&instance),
            AdapterChoice::Auto => hardware(&instance).or_else(|hardware_error| {
                software(&instance)
                    .map_err(|software_error| format!("{hardware_error}; {software_error}"))
            }),
        }?;
        let raw = adapter.get_info();
        let info = AdapterInfo {
            name: raw.name.clone(),
            backend: raw.backend.to_str().to_string(),
            device_type: device_type(raw.device_type).to_string(),
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("tickora"),
            required_limits: wgpu::Limits::downlevel_defaults(),
            ..Default::default()
        }))
        .map_err(|e| format!("adapter {} refused a device: {e}", info.name))?;
        let pipeline = pipeline(&device);
        let mesh = mesh_pipelines(&device);
        Ok(Renderer {
            device,
            queue,
            pipeline,
            mesh,
            info,
        })
    }

    pub fn info(&self) -> &AdapterInfo {
        &self.info
    }

    pub fn render(
        &self,
        scene: &Scene,
        camera: &OrthoCamera,
        width: u32,
        height: u32,
    ) -> Result<(Image, Layout), String> {
        check_size(width, height)?;
        let layout = scene.layout(camera, width, height);
        let vertices = vertex_bytes(&layout.rects, width, height);
        let vertex_buffer = (!vertices.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("tickora rects"),
                    contents: &vertices,
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        let image = self.frame(scene.background, width, height, false, |pass| {
            if let Some(buffer) = &vertex_buffer {
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..(vertices.len() as u64 / VERTEX_SIZE) as u32, 0..1);
            }
        })?;
        Ok((image, layout))
    }

    pub fn render3(
        &self,
        scene: &Scene3,
        camera: &PerspectiveCamera,
        view_mode: ViewMode,
        width: u32,
        height: u32,
    ) -> Result<(Image, usize), String> {
        check_size(width, height)?;
        let (batches, pipeline) = match view_mode {
            ViewMode::Unlit => (scene.triangles(), &self.mesh.triangles),
            ViewMode::Wireframe => (scene.lines(), &self.mesh.lines),
        };
        let uniforms: Vec<u8> = camera
            .view_projection(width, height)
            .column_major_f32()
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let frame_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tickora frame uniforms"),
                contents: &uniforms,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let frame_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tickora frame uniforms"),
            layout: &self.mesh.frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });
        let buffers: Vec<(&Batch, wgpu::Buffer)> = batches
            .iter()
            .map(|batch| {
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("tickora mesh batch"),
                        contents: &batch.vertices,
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                (batch, buffer)
            })
            .collect();
        let image = self.frame(scene.background, width, height, true, |pass| {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &frame_group, &[]);
            for (batch, buffer) in &buffers {
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..batch.vertex_count(), 0..1);
            }
        })?;
        Ok((image, scene.off_screen(camera, width, height)))
    }

    fn frame(
        &self,
        background: [u8; 4],
        width: u32,
        height: u32,
        depth: bool,
        record: impl FnOnce(&mut wgpu::RenderPass<'_>),
    ) -> Result<Image, String> {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let depth_view = depth.then(|| {
            self.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("tickora depth"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: DEPTH_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        });
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tickora frame"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let row_bytes = width * 4;
        let padded_row = row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tickora readback"),
            size: u64::from(padded_row) * u64::from(height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tickora frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tickora frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color(background)),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: depth_view.as_ref().map(|view| {
                    wgpu::RenderPassDepthStencilAttachment {
                        view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0.0),
                            store: wgpu::StoreOp::Discard,
                        }),
                        stencil_ops: None,
                    }
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            record(&mut pass);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (sender, receiver) = mpsc::channel();
        readback.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("waiting for the frame failed: {e}"))?;
        receiver
            .recv()
            .map_err(|e| format!("frame readback was dropped: {e}"))?
            .map_err(|e| format!("frame readback failed: {e}"))?;
        let mut pixels = Vec::with_capacity((row_bytes * height) as usize);
        {
            let mapped = readback
                .get_mapped_range(..)
                .map_err(|e| format!("frame readback could not be read: {e}"))?;
            for row in mapped.chunks(padded_row as usize) {
                pixels.extend_from_slice(&row[..row_bytes as usize]);
            }
        }
        readback.unmap();
        Image::new(width, height, pixels)
    }
}

fn check_size(width: u32, height: u32) -> Result<(), String> {
    for (name, size) in [("width", width), ("height", height)] {
        if !(MIN_SIZE..=MAX_SIZE).contains(&size) {
            return Err(format!("{name} must be from {MIN_SIZE} to {MAX_SIZE}"));
        }
    }
    Ok(())
}

fn hardware(instance: &wgpu::Instance) -> Result<wgpu::Adapter, String> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        ..Default::default()
    }))
    .map_err(|e| format!("no hardware adapter: {e}"))?;
    if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
        return Err(format!(
            "no hardware adapter: only the software adapter {} is available",
            adapter.get_info().name
        ));
    }
    Ok(adapter)
}

fn software(instance: &wgpu::Instance) -> Result<wgpu::Adapter, String> {
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        force_fallback_adapter: true,
        ..Default::default()
    }))
    .map_err(|e| format!("no software adapter: {e}"))
}

fn device_type(kind: wgpu::DeviceType) -> &'static str {
    match kind {
        wgpu::DeviceType::Cpu => "cpu",
        wgpu::DeviceType::DiscreteGpu => "discrete_gpu",
        wgpu::DeviceType::IntegratedGpu => "integrated_gpu",
        wgpu::DeviceType::VirtualGpu => "virtual_gpu",
        wgpu::DeviceType::Other => "other",
    }
}

fn clear_color(color: [u8; 4]) -> wgpu::Color {
    let channel = |value: u8| f64::from(value) / 255.0;
    wgpu::Color {
        r: channel(color[0]),
        g: channel(color[1]),
        b: channel(color[2]),
        a: channel(color[3]),
    }
}

fn vertex_bytes(rects: &[PixelRect], width: u32, height: u32) -> Vec<u8> {
    let ndc_x = |x: u32| (2.0 * f64::from(x) / f64::from(width) - 1.0) as f32;
    let ndc_y = |y: u32| (1.0 - 2.0 * f64::from(y) / f64::from(height)) as f32;
    let mut bytes = Vec::with_capacity(rects.len() * 6 * VERTEX_SIZE as usize);
    for rect in rects {
        let (left, right) = (ndc_x(rect.x0), ndc_x(rect.x1));
        let (top, bottom) = (ndc_y(rect.y0), ndc_y(rect.y1));
        for (x, y) in [
            (left, top),
            (left, bottom),
            (right, bottom),
            (left, top),
            (right, bottom),
            (right, top),
        ] {
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
            bytes.extend_from_slice(&rect.color);
        }
    }
    bytes
}

fn pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tickora rects"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Unorm8x4];
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tickora rects"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: VERTEX_SIZE,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            })],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn mesh_pipelines(device: &wgpu::Device) -> MeshPipelines {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tickora meshes"),
        source: wgpu::ShaderSource::Wgsl(MESH_SHADER.into()),
    });
    let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("tickora frame uniforms"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(FRAME_UNIFORM_SIZE),
            },
            count: None,
        }],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tickora meshes"),
        bind_group_layouts: &[Some(&frame_layout)],
        ..Default::default()
    });
    let attributes = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x2,
        3 => Float32x4
    ];
    let build = |topology: wgpu::PrimitiveTopology, fragment: &str| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tickora meshes"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: scene3::VERTEX_SIZE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(fragment),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    MeshPipelines {
        triangles: build(wgpu::PrimitiveTopology::TriangleList, "fs_unlit"),
        lines: build(wgpu::PrimitiveTopology::LineList, "fs_unlit"),
        frame_layout,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_choice_parses() {
        assert_eq!("auto".parse(), Ok(AdapterChoice::Auto));
        assert_eq!("hardware".parse(), Ok(AdapterChoice::Hardware));
        assert_eq!("software".parse(), Ok(AdapterChoice::Software));
        assert!("gpu".parse::<AdapterChoice>().is_err());
    }

    #[test]
    fn bucket_is_os_and_backend() {
        let info = AdapterInfo {
            name: "x".to_string(),
            backend: "dx12".to_string(),
            device_type: "cpu".to_string(),
        };
        assert_eq!(info.bucket(), format!("{}_dx12", std::env::consts::OS));
    }

    #[test]
    fn six_vertices_per_rect() {
        let rect = PixelRect {
            x0: 0,
            y0: 0,
            x1: 16,
            y1: 16,
            color: [1, 2, 3, 4],
        };
        let bytes = vertex_bytes(&[rect, rect], 16, 16);
        assert_eq!(bytes.len() as u64, 2 * 6 * VERTEX_SIZE);
        assert_eq!(&bytes[..8], &[0, 0, 128, 191, 0, 0, 128, 63]);
        assert_eq!(&bytes[8..12], &[1, 2, 3, 4]);
    }
}
