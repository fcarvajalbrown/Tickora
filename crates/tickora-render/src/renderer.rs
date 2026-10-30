use std::str::FromStr;
use std::sync::{Mutex, MutexGuard, mpsc};

use wgpu::util::DeviceExt;

use crate::camera::{OrthoCamera, PerspectiveCamera};
use crate::image::Image;
use crate::light::{self, ShadowQuality};
use crate::scene::{Layout, PixelRect, Scene, ViewMode};
use crate::scene3::{self, Batch, Scene3};
use crate::tone::{self, Display};
use crate::{MAX_SIZE, MIN_SIZE};

pub const ADAPTER_ENV: &str = "TICKORA_ADAPTER";

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const LIGHT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const TONE_UNIFORM_SIZE: u64 = 16;
const VERTEX_SIZE: u64 = 12;
const FRAME_UNIFORM_SIZE: u64 = 64 + scene3::LIGHT_BYTES_SIZE as u64;

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
const CONTENT_GAMMA: f32 = 2.2;

const JITTER_TILE: u32 = 32u;

struct Light {
    position_radius: vec4<f32>,
    color_intensity: vec4<f32>,
    falloff: vec4<f32>,
    specular: vec4<f32>,
}

struct Spot {
    view_projection: mat4x4<f32>,
    position_radius: vec4<f32>,
    color_intensity: vec4<f32>,
    forward_cos_half: vec4<f32>,
    falloff_cone_layer_near: vec4<f32>,
    specular: vec4<f32>,
    shadow_layer_side_bias_offset: vec4<f32>,
}

struct Frame {
    view_projection: mat4x4<f32>,
    camera_blur: vec4<f32>,
    ambient: vec4<f32>,
    counts: vec4<u32>,
    lights: array<Light, 16>,
    spots: array<Spot, 16>,
}

struct Material {
    specular_power: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var gobos: texture_2d_array<f32>;
@group(0) @binding(2) var shadow_maps: texture_depth_2d_array;
@group(0) @binding(3) var jitter: texture_2d<f32>;
@group(1) @binding(0) var albedo: texture_2d<f32>;
@group(1) @binding(1) var albedo_sampler: sampler;
@group(1) @binding(2) var<uniform> material: Material;

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

fn decoded(uv: vec2<f32>) -> vec3<f32> {
    let texel = textureSample(albedo, albedo_sampler, uv);
    return pow(texel.rgb, vec3<f32>(CONTENT_GAMMA));
}

@fragment
fn fs_unlit(input: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color.rgb * decoded(input.uv), 1.0);
}

fn raised(base: f32, power: f32) -> f32 {
    if base <= 0.0 {
        return 0.0;
    }
    return pow(base, power);
}

fn distance_falloff(distance: f32, radius: f32, power: f32) -> f32 {
    return raised(1.0 - distance / radius, power);
}

fn spot_cone(spot: Spot, from_light: vec3<f32>) -> vec3<f32> {
    let cos_theta = dot(from_light, spot.forward_cos_half.xyz);
    let t = saturate((1.0 - cos_theta) / (1.0 - spot.forward_cos_half.w));
    return vec3<f32>(raised(1.0 - sqrt(t), spot.falloff_cone_layer_near.y));
}

fn shadow_tap(layer: i32, side: f32, base: vec2<f32>, offset: vec2<f32>, depth: f32, near: f32) -> f32 {
    let texel = clamp(floor(base + offset * frame.camera_blur.w), vec2<f32>(0.0), vec2<f32>(side - 1.0));
    let stored = textureLoad(shadow_maps, vec2<i32>(texel), layer, 0);
    return select(0.0, 1.0, stored * depth <= near);
}

fn jitter_offset(pixel: vec2<u32>, tap: u32) -> vec2<f32> {
    let texel = vec2<u32>(pixel.x % JITTER_TILE, pixel.y % JITTER_TILE + JITTER_TILE * (tap / 2u));
    let pair = textureLoad(jitter, texel, 0) * 2.0 - 1.0;
    if tap % 2u == 0u {
        return pair.xy;
    }
    return pair.zw;
}

fn spot_shadow(spot: Spot, world: vec3<f32>, normal: vec3<f32>, direction: vec3<f32>, pixel: vec2<u32>) -> f32 {
    let shadow = spot.shadow_layer_side_bias_offset;
    let receiver = world + normal * ((1.0 - dot(normal, direction)) * shadow.w);
    let clip = spot.view_projection * vec4<f32>(receiver, 1.0);
    if clip.w <= 0.0 {
        return 1.0;
    }
    let ndc = clip.xy / clip.w;
    if abs(ndc.x) > 1.0 || abs(ndc.y) > 1.0 {
        return 1.0;
    }
    let uv = vec2<f32>(0.5 + 0.5 * ndc.x, 0.5 - 0.5 * ndc.y);
    let base = uv * shadow.y;
    let depth = dot(receiver - spot.position_radius.xyz, spot.forward_cos_half.xyz) - shadow.z;
    let near = spot.falloff_cone_layer_near.w;
    let layer = i32(shadow.x);
    let taps = frame.counts.z;
    if taps <= 1u {
        return shadow_tap(layer, shadow.y, base, vec2<f32>(0.0), depth, near);
    }
    var first = 0.0;
    for (var tap = 0u; tap < 4u; tap = tap + 1u) {
        first = first + shadow_tap(layer, shadow.y, base, jitter_offset(pixel, tap), depth, near);
    }
    if first == 0.0 || first == 4.0 {
        return first / 4.0;
    }
    var rest = 0.0;
    for (var tap = 4u; tap < taps; tap = tap + 1u) {
        rest = rest + shadow_tap(layer, shadow.y, base, jitter_offset(pixel, tap), depth, near);
    }
    return first / f32(taps) + rest / f32(taps);
}

fn blinn_phong(normal: vec3<f32>, direction: vec3<f32>, to_camera: vec3<f32>, power: f32) -> f32 {
    let half = normalize(direction + to_camera);
    return raised(saturate(dot(normal, half)), power);
}

fn spot_gobo(spot: Spot, world: vec3<f32>) -> vec3<f32> {
    let clip = spot.view_projection * vec4<f32>(world, 1.0);
    if clip.w <= 0.0 {
        return vec3<f32>(0.0);
    }
    let ndc = clip.xy / clip.w;
    if abs(ndc.x) > 1.0 || abs(ndc.y) > 1.0 {
        return vec3<f32>(0.0);
    }
    let size = textureDimensions(gobos);
    let uv = vec2<f32>(0.5 + 0.5 * ndc.x, 0.5 - 0.5 * ndc.y);
    let texel = min(vec2<u32>(uv * vec2<f32>(size)), size - vec2<u32>(1u));
    let layer = i32(spot.falloff_cone_layer_near.z);
    return pow(textureLoad(gobos, texel, layer, 0).rgb, vec3<f32>(CONTENT_GAMMA));
}

@fragment
fn fs_lit(input: Out) -> @location(0) vec4<f32> {
    let base = decoded(input.uv);
    let normal = normalize(input.normal);
    let shine = material.specular_power.rgb;
    let shiny = any(shine > vec3<f32>(0.0));
    let power = exp2(material.specular_power.w * 10.0) + 1.0;
    let to_camera = normalize(frame.camera_blur.xyz - input.world);
    let pixel = vec2<u32>(input.position.xy);
    var light = frame.ambient.rgb;
    var specular = vec3<f32>(0.0);
    for (var index = 0u; index < frame.counts.x; index = index + 1u) {
        let source = frame.lights[index];
        let to_light = source.position_radius.xyz - input.world;
        let distance = length(to_light);
        let direction = to_light / max(distance, 0.0001);
        let falloff = distance_falloff(distance, source.position_radius.w, source.falloff.x);
        let lambert = max(dot(normal, direction), 0.0);
        light = light + source.color_intensity.rgb * source.color_intensity.w * lambert * falloff;
        if shiny {
            let highlight = blinn_phong(normal, direction, to_camera, power) * falloff;
            specular = specular + source.specular.rgb * source.color_intensity.w * shine * highlight;
        }
    }
    for (var index = 0u; index < frame.counts.y; index = index + 1u) {
        let spot = frame.spots[index];
        let to_light = spot.position_radius.xyz - input.world;
        let distance = length(to_light);
        let direction = to_light / max(distance, 0.0001);
        let falloff = distance_falloff(distance, spot.position_radius.w, spot.falloff_cone_layer_near.x);
        let lambert = max(dot(normal, direction), 0.0);
        var shape: vec3<f32>;
        if spot.falloff_cone_layer_near.z < 0.0 {
            shape = spot_cone(spot, -direction);
        } else {
            shape = spot_gobo(spot, input.world);
        }
        if spot.shadow_layer_side_bias_offset.x >= 0.0 {
            shape = shape * spot_shadow(spot, input.world, normal, direction, pixel);
        }
        light = light + spot.color_intensity.rgb * spot.color_intensity.w * lambert * falloff * shape;
        if shiny {
            let highlight = blinn_phong(normal, direction, to_camera, power) * falloff;
            specular = specular + spot.specular.rgb * spot.color_intensity.w * shine * highlight * shape;
        }
    }
    var colour = input.color.rgb * base * light;
    if shiny {
        colour = colour + specular;
    }
    return vec4<f32>(colour, 1.0);
}
";

const SHADOW_SHADER: &str = "
@group(0) @binding(0) var<uniform> view_projection: mat4x4<f32>;

@vertex
fn vs(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return view_projection * vec4<f32>(position, 1.0);
}
";

const TONE_SHADER: &str = "
const A: f32 = 0.15;
const B: f32 = 0.50;
const C: f32 = 0.10;
const D: f32 = 0.20;
const E: f32 = 0.02;
const F: f32 = 0.30;

struct Tone {
    exposure_white_gamma_curve: vec4<f32>,
}

@group(0) @binding(0) var light_buffer: texture_2d<f32>;
@group(0) @binding(1) var<uniform> tone: Tone;

@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index & 1u) * 4u) - 1.0;
    let y = f32((index >> 1u) * 4u) - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

fn hable(x: vec3<f32>) -> vec3<f32> {
    return (x * (A * x + C * B) + D * E) / (x * (A * x + B) + D * F) - E / F;
}

@fragment
fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let settings = tone.exposure_white_gamma_curve;
    var colour = textureLoad(light_buffer, vec2<i32>(position.xy), 0).rgb;
    if settings.w > 0.5 {
        colour = hable(colour * settings.x) / hable(vec3<f32>(settings.y));
    }
    return vec4<f32>(pow(saturate(colour), vec3<f32>(1.0 / settings.z)), 1.0);
}
";

struct TonePipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

struct MeshPipelines {
    lit: wgpu::RenderPipeline,
    frame_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    white: Image,
    triangles: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    shadow: wgpu::RenderPipeline,
    shadow_layout: wgpu::BindGroupLayout,
}

struct JitterMaps {
    low: wgpu::TextureView,
    medium: wgpu::TextureView,
    high: wgpu::TextureView,
}

impl JitterMaps {
    fn get(&self, quality: ShadowQuality) -> &wgpu::TextureView {
        match quality {
            ShadowQuality::Low => &self.low,
            ShadowQuality::Medium => &self.medium,
            ShadowQuality::High => &self.high,
        }
    }
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
    jitter: JitterMaps,
    tone: TonePipeline,
    info: AdapterInfo,
    one_frame_at_a_time: Mutex<()>,
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
        let tone = tone_pipeline(&device);
        let jitter = JitterMaps {
            low: rgba_view(&device, &queue, "tickora jitter", 1, 1, &[128; 4]),
            medium: jitter_view(
                &device,
                &queue,
                &light::jitter_map(ShadowQuality::Medium.taps()),
            ),
            high: jitter_view(
                &device,
                &queue,
                &light::jitter_map(ShadowQuality::High.taps()),
            ),
        };
        Ok(Renderer {
            device,
            queue,
            pipeline,
            mesh,
            jitter,
            tone,
            info,
            one_frame_at_a_time: Mutex::new(()),
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
        let _frame = self.lock_frame();
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
        let image = self.frame(scene.background, width, height, |pass| {
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
        display: &Display,
        width: u32,
        height: u32,
    ) -> Result<(Image, usize), String> {
        check_size(width, height)?;
        tone::check_positive("exposure", f64::from(scene.exposure))?;
        tone::check_positive("white_point", f64::from(scene.white_point))?;
        tone::check_positive("gamma", f64::from(display.gamma))?;
        let _frame = self.lock_frame();
        let (batches, pipeline) = match view_mode {
            ViewMode::Lit => (scene.triangles(), &self.mesh.lit),
            ViewMode::Unlit => (scene.triangles(), &self.mesh.triangles),
            ViewMode::Wireframe => (scene.lines(), &self.mesh.lines),
        };
        let mut uniforms: Vec<u8> = camera
            .view_projection(width, height)
            .column_major_f32()
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        uniforms.extend(scene.light_bytes(camera, display.shadow_quality)?);
        let gobo_view = self.gobo_view(&scene.gobo_layers()?);
        let shadow_layers = scene.shadow_layers()?;
        let lit = view_mode == ViewMode::Lit;
        let shadow_side = if lit { shadow_layers.side.max(1) } else { 1 };
        let shadow_count = if lit { shadow_layers.count.max(1) } else { 1 };
        let shadow_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tickora shadow maps"),
            size: wgpu::Extent3d {
                width: shadow_side,
                height: shadow_side,
                depth_or_array_layers: shadow_count as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let casting: Vec<&scene3::SpotLight> = if lit {
            scene
                .spots
                .iter()
                .filter(|spot| spot.cast_shadows)
                .collect()
        } else {
            Vec::new()
        };
        let shadow_draws: Vec<(wgpu::TextureView, wgpu::BindGroup, u32)> = casting
            .iter()
            .enumerate()
            .map(|(layer, spot)| {
                let bytes: Vec<u8> = spot
                    .view_projection()
                    .column_major_f32()
                    .iter()
                    .flat_map(|value| value.to_le_bytes())
                    .collect();
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("tickora shadow view"),
                        contents: &bytes,
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("tickora shadow view"),
                    layout: &self.mesh.shadow_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    }],
                });
                let view = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer as u32,
                    array_layer_count: Some(1),
                    ..Default::default()
                });
                (view, group, spot.shadow_resolution.side())
            })
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
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&gobo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        self.jitter.get(display.shadow_quality),
                    ),
                },
            ],
        });
        let draws: Vec<(&Batch, wgpu::Buffer, wgpu::BindGroup)> = batches
            .iter()
            .map(|batch| {
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("tickora mesh batch"),
                        contents: &batch.vertices,
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                let image = match (view_mode, scene.textures.get(&batch.material)) {
                    (ViewMode::Wireframe, _) | (_, None) => &self.mesh.white,
                    (_, Some(image)) => image.as_ref(),
                };
                let material = if lit {
                    scene.material(&batch.material)
                } else {
                    scene3::Material::default()
                };
                (batch, buffer, self.texture_group(image, &material))
            })
            .collect();
        let curve = if view_mode == ViewMode::Lit { 1.0 } else { 0.0 };
        let tone_bytes: Vec<u8> = [scene.exposure, scene.white_point, display.gamma, curve]
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let tone_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tickora tone uniforms"),
                contents: &tone_bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let light_view = self
            .target(
                "tickora light buffer",
                LIGHT_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                width,
                height,
            )
            .create_view(&wgpu::TextureViewDescriptor::default());
        let tone_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tickora tone"),
            layout: &self.tone.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&light_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: tone_buffer.as_entire_binding(),
                },
            ],
        });
        let depth_view = self
            .target(
                "tickora depth",
                DEPTH_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
                width,
                height,
            )
            .create_view(&wgpu::TextureViewDescriptor::default());
        let frame = self.target(
            "tickora frame",
            FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            width,
            height,
        );
        let frame_view = frame.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tickora frame"),
            });
        for (view, group, side) in &shadow_draws {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tickora shadow map"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(0.0, 0.0, *side as f32, *side as f32, 0.0, 1.0);
            pass.set_pipeline(&self.mesh.shadow);
            pass.set_bind_group(0, group, &[]);
            for (batch, buffer, _) in &draws {
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..batch.vertex_count(), 0..1);
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tickora geometry"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &light_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(linear_clear_color(scene.background)),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &frame_group, &[]);
            for (batch, buffer, texture) in &draws {
                pass.set_bind_group(1, texture, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..batch.vertex_count(), 0..1);
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tickora tone"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.tone.pipeline);
            pass.set_bind_group(0, &tone_group, &[]);
            pass.draw(0..3, 0..1);
        }
        let image = self.read_back(encoder, &frame, width, height)?;
        Ok((image, scene.off_screen(camera, width, height)))
    }

    fn lock_frame(&self) -> MutexGuard<'_, ()> {
        self.one_frame_at_a_time
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn gobo_view(&self, layers: &scene3::GoboLayers<'_>) -> wgpu::TextureView {
        let frames = if layers.frames.is_empty() {
            vec![&self.mesh.white]
        } else {
            layers.frames.clone()
        };
        let pixels: Vec<u8> = frames
            .iter()
            .flat_map(|image| image.pixels.iter().copied())
            .collect();
        let texture = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("tickora gobos"),
                size: wgpu::Extent3d {
                    width: frames[0].width,
                    height: frames[0].height,
                    depth_or_array_layers: frames.len() as u32,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &pixels,
        );
        texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        })
    }

    fn texture_group(&self, image: &Image, material: &scene3::Material) -> wgpu::BindGroup {
        let material_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tickora material"),
                contents: &material.bytes(),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let texture = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("tickora albedo"),
                size: wgpu::Extent3d {
                    width: image.width,
                    height: image.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &image.pixels,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tickora albedo"),
            layout: &self.mesh.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.mesh.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: material_buffer.as_entire_binding(),
                },
            ],
        })
    }

    fn target(
        &self,
        label: &str,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        width: u32,
        height: u32,
    ) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }

    fn frame(
        &self,
        background: [u8; 4],
        width: u32,
        height: u32,
        record: impl FnOnce(&mut wgpu::RenderPass<'_>),
    ) -> Result<Image, String> {
        let texture = self.target(
            "tickora frame",
            FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            width,
            height,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
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
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            record(&mut pass);
        }
        self.read_back(encoder, &texture, width, height)
    }

    fn read_back(
        &self,
        mut encoder: wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
    ) -> Result<Image, String> {
        let row_bytes = width * 4;
        let padded_row = row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tickora readback"),
            size: u64::from(padded_row) * u64::from(height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
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

fn rgba_view(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> wgpu::TextureView {
    device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            pixels,
        )
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn jitter_view(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    map: &light::JitterMap,
) -> wgpu::TextureView {
    let pixels: Vec<u8> = map.texels.iter().flatten().copied().collect();
    rgba_view(
        device,
        queue,
        "tickora jitter",
        map.width() as u32,
        map.height() as u32,
        &pixels,
    )
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

fn linear_clear_color(color: [u8; 4]) -> wgpu::Color {
    wgpu::Color {
        r: tone::decode_byte(color[0]),
        g: tone::decode_byte(color[1]),
        b: tone::decode_byte(color[2]),
        a: 1.0,
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
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(FRAME_UNIFORM_SIZE),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });
    let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("tickora albedo"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(scene3::MATERIAL_BYTES_SIZE as u64),
                },
                count: None,
            },
        ],
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("tickora nearest repeat"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tickora meshes"),
        bind_group_layouts: &[Some(&frame_layout), Some(&texture_layout)],
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
                    format: LIGHT_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let shadow_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tickora shadow maps"),
        source: wgpu::ShaderSource::Wgsl(SHADOW_SHADER.into()),
    });
    let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("tickora shadow view"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(64),
            },
            count: None,
        }],
    });
    let shadow_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tickora shadow maps"),
        bind_group_layouts: &[Some(&shadow_layout)],
        ..Default::default()
    });
    let shadow = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tickora shadow maps"),
        layout: Some(&shadow_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shadow_module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: scene3::VERTEX_SIZE,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3],
            })],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
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
        fragment: None,
        multiview_mask: None,
        cache: None,
    });
    MeshPipelines {
        shadow,
        shadow_layout,
        lit: build(wgpu::PrimitiveTopology::TriangleList, "fs_lit"),
        triangles: build(wgpu::PrimitiveTopology::TriangleList, "fs_unlit"),
        lines: build(wgpu::PrimitiveTopology::LineList, "fs_unlit"),
        frame_layout,
        texture_layout,
        sampler,
        white: Image {
            width: 1,
            height: 1,
            pixels: vec![255; 4],
        },
    }
}

fn tone_pipeline(device: &wgpu::Device) -> TonePipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tickora tone"),
        source: wgpu::ShaderSource::Wgsl(TONE_SHADER.into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("tickora tone"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(TONE_UNIFORM_SIZE),
                },
                count: None,
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tickora tone"),
        bind_group_layouts: &[Some(&layout)],
        ..Default::default()
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tickora tone"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
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
    });
    TonePipeline { pipeline, layout }
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
