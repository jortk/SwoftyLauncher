use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};

/// The most frames playback may skip between two renders and still count as continuing.
const STEP: usize = 8;

use swofty_shaders::{ActiveInstance, TextureDecl, mip_levels};

use crate::{ClipInfo, CompiledEffect, Frame};

const UNIFORM_DWORDS: usize = 56;
const UNIFORM_STRIDE: u64 = 256;
const UNIFORM_BYTES: u64 = (UNIFORM_DWORDS * 4) as u64;
const INPUTS: usize = 8;

const PICTURE: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const BYTE: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

const HELPERS: &str = "
@group(0) @binding(0) var up_src: texture_2d<f32>;
@group(0) @binding(1) var up_dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var down_src: texture_2d<f32>;
@group(0) @binding(3) var down_dst: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn up(@builtin(global_invocation_id) id: vec3<u32>) {
    let d = textureDimensions(up_src);
    if (id.x >= d.x || id.y >= d.y) { return; }
    textureStore(up_dst, vec2<i32>(id.xy), textureLoad(up_src, vec2<i32>(id.xy), 0));
}

@group(0) @binding(6) var mip_src: texture_2d<f32>;
@group(0) @binding(7) var mip_dst: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn mip(@builtin(global_invocation_id) id: vec3<u32>) {
    let d = textureDimensions(mip_dst);
    if (id.x >= d.x || id.y >= d.y) { return; }
    let m = vec2<i32>(textureDimensions(mip_src)) - vec2<i32>(1);
    let b = vec2<i32>(id.xy) * 2;
    let sum = textureLoad(mip_src, min(b, m), 0)
        + textureLoad(mip_src, min(b + vec2<i32>(1, 0), m), 0)
        + textureLoad(mip_src, min(b + vec2<i32>(0, 1), m), 0)
        + textureLoad(mip_src, min(b + vec2<i32>(1, 1), m), 0);
    textureStore(mip_dst, vec2<i32>(id.xy), sum * 0.25);
}

@group(0) @binding(4) var narrow_src: texture_2d<f32>;
@group(0) @binding(5) var narrow_dst: texture_storage_2d<rgba16float, write>;

fn narrow_in(id: vec3<u32>) -> bool {
    let d = textureDimensions(narrow_src);
    return id.x < d.x && id.y < d.y;
}

fn narrow_at(id: vec3<u32>) -> vec4<f32> {
    return textureLoad(narrow_src, vec2<i32>(id.xy), 0);
}

@compute @workgroup_size(8, 8)
fn as_rgba8(@builtin(global_invocation_id) id: vec3<u32>) {
    if (!narrow_in(id)) { return; }
    let c = clamp(narrow_at(id), vec4<f32>(0.0), vec4<f32>(1.0));
    textureStore(narrow_dst, vec2<i32>(id.xy), round(c * 255.0) / 255.0);
}

@compute @workgroup_size(8, 8)
fn as_rgb10a2(@builtin(global_invocation_id) id: vec3<u32>) {
    if (!narrow_in(id)) { return; }
    let c = clamp(narrow_at(id), vec4<f32>(0.0), vec4<f32>(1.0));
    textureStore(narrow_dst, vec2<i32>(id.xy), vec4<f32>(round(c.rgb * 1023.0) / 1023.0, round(c.a * 3.0) / 3.0));
}

@compute @workgroup_size(8, 8)
fn as_rg(@builtin(global_invocation_id) id: vec3<u32>) {
    if (!narrow_in(id)) { return; }
    let c = narrow_at(id);
    textureStore(narrow_dst, vec2<i32>(id.xy), vec4<f32>(c.r, c.g, 0.0, 1.0));
}

@compute @workgroup_size(8, 8)
fn as_r(@builtin(global_invocation_id) id: vec3<u32>) {
    if (!narrow_in(id)) { return; }
    textureStore(narrow_dst, vec2<i32>(id.xy), vec4<f32>(narrow_at(id).r, 0.0, 0.0, 1.0));
}

@compute @workgroup_size(8, 8)
fn as_r11g11b10f(@builtin(global_invocation_id) id: vec3<u32>) {
    if (!narrow_in(id)) { return; }
    textureStore(narrow_dst, vec2<i32>(id.xy), vec4<f32>(max(narrow_at(id).rgb, vec3<f32>(0.0)), 1.0));
}

@compute @workgroup_size(8, 8)
fn down(@builtin(global_invocation_id) id: vec3<u32>) {
    let d = textureDimensions(down_src);
    if (id.x >= d.x || id.y >= d.y) { return; }
    let c = textureLoad(down_src, vec2<i32>(id.xy), 0);
    textureStore(down_dst, vec2<i32>(id.xy), clamp(c, vec4<f32>(0.0), vec4<f32>(1.0)));
}
";

struct Tex {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    store: wgpu::TextureView,
    levels: Vec<wgpu::TextureView>,
    width: u32,
    height: u32,
    mips: u32,
}

impl Tex {
    fn new(device: &wgpu::Device, label: &str, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        Self::with_mips(device, label, width, height, format, 1)
    }

    fn with_mips(
        device: &wgpu::Device,
        label: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        mips: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mips,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let level = |mip: u32| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: mip,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let store = level(0);
        let levels = if mips > 1 {
            (0..mips).map(level).collect()
        } else {
            Vec::new()
        };
        Self {
            texture,
            view,
            store,
            levels,
            width,
            height,
            mips,
        }
    }

    fn copy_to(&self, encoder: &mut wgpu::CommandEncoder, to: &Tex) {
        for mip in 0..self.mips.min(to.mips) {
            encoder.copy_texture_to_texture(
                level_of(&self.texture, mip),
                level_of(&to.texture, mip),
                wgpu::Extent3d {
                    width: (self.width >> mip).max(1),
                    height: (self.height >> mip).max(1),
                    depth_or_array_layers: 1,
                },
            );
        }
    }
}

fn level_of(texture: &wgpu::Texture, mip_level: u32) -> wgpu::TexelCopyTextureInfo<'_> {
    wgpu::TexelCopyTextureInfo {
        texture,
        mip_level,
        origin: wgpu::Origin3d::ZERO,
        aspect: wgpu::TextureAspect::All,
    }
}

fn clear(encoder: &mut wgpu::CommandEncoder, tex: &Tex) {
    encoder.clear_texture(&tex.texture, &wgpu::ImageSubresourceRange::default());
}

fn format_of(_decl: &TextureDecl) -> wgpu::TextureFormat {
    PICTURE
}

/// Declared textures are all rgba16float here. After a pass writes one declared with another format, the
/// helper this names gives it the values the game's texture of that format holds: clamped and rounded
/// for the unorm formats, missing channels as 0 (alpha 1), no negatives for r11g11b10f.
const NARROW: [&str; 5] = ["as_rgba8", "as_rgb10a2", "as_rg", "as_r", "as_r11g11b10f"];

fn narrow_of(decl: &TextureDecl) -> Option<&'static str> {
    match decl.format.as_deref()? {
        "rgba8" => Some(NARROW[0]),
        "rgb10a2" => Some(NARROW[1]),
        "rg16f" => Some(NARROW[2]),
        "r16f" | "r32f" => Some(NARROW[3]),
        "r11g11b10f" => Some(NARROW[4]),
        _ => None,
    }
}

struct PassPipe {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    threads: (u32, u32),
}

struct EffectGpu {
    effect: swofty_shaders::Effect,
    passes: Vec<PassPipe>,
    images: HashMap<String, Tex>,
}

struct Slot {
    name: String,
    tex: Tex,
    shadow: Option<Tex>,
    narrow: Option<(&'static str, Tex)>,
    persistent: bool,
}

struct Inst {
    effect: String,
    params: Vec<f32>,
    slots: Vec<Slot>,
}

#[derive(Clone, Copy, PartialEq)]
enum Picture {
    Original,
    A,
    B,
}

struct Targets {
    width: u32,
    height: u32,
    upload: Tex,
    original: Tex,
    previous: Tex,
    pictures: [Tex; 2],
    out: Tex,
    readback: wgpu::Buffer,
    row_bytes: u32,
}

/// The renderer: effects and instances in, frames in, frames out.
pub struct Preview {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_name: String,
    errors: Arc<Mutex<Vec<String>>>,
    samplers: [wgpu::Sampler; 4],
    dummy: Tex,
    depth: Tex,
    layout: wgpu::BindGroupLayout,
    up: wgpu::ComputePipeline,
    down: wgpu::ComputePipeline,
    mip: wgpu::ComputePipeline,
    narrow: HashMap<&'static str, wgpu::ComputePipeline>,
    uniforms: wgpu::Buffer,
    uniform_slots: u64,
    sources: HashMap<String, CompiledEffect>,
    effects: HashMap<String, EffectGpu>,
    requested: Vec<ActiveInstance>,
    instances: Vec<Inst>,
    instances_dirty: bool,
    targets: Option<Targets>,
    last_frame: Option<usize>,
    fps: f32,
    view: u32,
}

fn sampler(device: &wgpu::Device, filter: wgpu::FilterMode, address: wgpu::AddressMode) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: address,
        address_mode_v: address,
        address_mode_w: address,
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: match filter {
            wgpu::FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
            _ => wgpu::MipmapFilterMode::Nearest,
        },
        ..Default::default()
    })
}

fn layout_for(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let sampled = |binding: u32, filterable: bool| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let sampler = |binding: u32, filtering: bool| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Sampler(if filtering {
            wgpu::SamplerBindingType::Filtering
        } else {
            wgpu::SamplerBindingType::NonFiltering
        }),
        count: None,
    };
    let mut entries = vec![
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        sampled(10, true),
        sampled(11, false),
    ];
    for i in 0..INPUTS as u32 {
        entries.push(sampled(12 + i, true));
    }
    entries.push(wgpu::BindGroupLayoutEntry {
        binding: 30,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::ReadWrite,
            format: PICTURE,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    });
    entries.push(sampler(40, true));
    entries.push(sampler(41, false));
    entries.push(sampler(42, true));
    entries.push(sampler(43, false));
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("swofty pass"),
        entries: &entries,
    })
}

fn words(bytes: &[u8]) -> Result<Vec<u32>, String> {
    if !bytes.len().is_multiple_of(4) || bytes.is_empty() {
        return Err("a SPIR-V blob is not a whole number of words".into());
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn scaled(base: u32, scale: f64) -> u32 {
    ((f64::from(base) * scale).round() as u32).max(1)
}

impl Preview {
    /// Opens a device. Blocks on the adapter and device requests; call it off the UI thread.
    pub fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| format!("no graphics adapter for the preview: {e}"))?;
        let adapter_name = adapter.get_info().name;
        let needed = wgpu::Features::CLEAR_TEXTURE | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        if !adapter.features().contains(needed) {
            return Err(format!(
                "{adapter_name} cannot run the preview: it lacks {:?}",
                needed - adapter.features()
            ));
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("swofty preview"),
            required_features: needed,
            ..Default::default()
        }))
        .map_err(|e| format!("preview device: {e}"))?;
        let errors = Arc::new(Mutex::new(Vec::new()));
        {
            let errors = errors.clone();
            device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
                if let Ok(mut list) = errors.lock() {
                    list.push(e.to_string());
                }
            }));
        }
        let samplers = [
            sampler(&device, wgpu::FilterMode::Linear, wgpu::AddressMode::ClampToEdge),
            sampler(&device, wgpu::FilterMode::Nearest, wgpu::AddressMode::ClampToEdge),
            sampler(&device, wgpu::FilterMode::Linear, wgpu::AddressMode::Repeat),
            sampler(&device, wgpu::FilterMode::Nearest, wgpu::AddressMode::Repeat),
        ];
        let dummy = Tex::new(&device, "dummy", 1, 1, PICTURE);
        let depth = Tex::new(&device, "depth", 1, 1, wgpu::TextureFormat::R32Float);
        let layout = layout_for(&device);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("preview helpers"),
            source: wgpu::ShaderSource::Wgsl(HELPERS.into()),
        });
        let helper = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let up = helper("up");
        let down = helper("down");
        let mip = helper("mip");
        let narrow = NARROW.iter().map(|&entry| (entry, helper(entry))).collect();
        let uniforms = Self::uniform_buffer(&device, 32);
        let preview = Self {
            device,
            queue,
            adapter_name,
            errors,
            samplers,
            dummy,
            depth,
            layout,
            up,
            down,
            mip,
            narrow,
            uniforms,
            uniform_slots: 32,
            sources: HashMap::new(),
            effects: HashMap::new(),
            requested: Vec::new(),
            instances: Vec::new(),
            instances_dirty: false,
            targets: None,
            last_frame: None,
            fps: 30.0,
            view: 0,
        };
        preview.take_errors()?;
        Ok(preview)
    }

    fn uniform_buffer(device: &wgpu::Device, slots: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("swofty cbuffer"),
            size: slots * UNIFORM_STRIDE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn take_errors(&self) -> Result<(), String> {
        let mut list = self.errors.lock().map_err(|e| e.to_string())?;
        if list.is_empty() {
            return Ok(());
        }
        let message = list.join("; ");
        list.clear();
        Err(message)
    }

    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    /// The clip's frame rate and view feed `swofty_time`, `swofty_frametime` and `swofty_view`.
    pub fn set_clip(&mut self, info: &ClipInfo) {
        self.fps = if info.fps > 0.0 { info.fps } else { 30.0 };
        self.view = match info.view.as_str() {
            "cockpit" => 1,
            "tcam" => 2,
            "tcam_offset" => 3,
            "chase_near" => 4,
            "chase_far" => 5,
            "other" => 6,
            _ => 0,
        };
        self.last_frame = None;
    }

    /// Replaces the effect library. Pipelines are built the first time an instance uses an effect.
    pub fn set_effects(&mut self, effects: Vec<CompiledEffect>) {
        self.sources = effects.into_iter().map(|e| (e.effect.id.clone(), e)).collect();
        self.effects.clear();
        self.instances_dirty = true;
    }

    pub fn set_instances(&mut self, instances: Vec<ActiveInstance>) {
        let same = instances.len() == self.requested.len()
            && instances
                .iter()
                .zip(&self.requested)
                .all(|(a, b)| a.effect == b.effect && a.label == b.label && a.params.len() == b.params.len());
        if same && !self.instances_dirty {
            for (inst, new) in self.instances.iter_mut().zip(&instances) {
                inst.params.clone_from(&new.params);
            }
        } else {
            self.instances_dirty = true;
        }
        self.requested = instances;
    }

    /// The frame as it is, for the left side of the compare wipe.
    pub fn render_original(&self, frame: &Frame) -> Frame {
        frame.clone()
    }

    fn build_effect(&self, id: &str) -> Result<EffectGpu, String> {
        let source = self
            .sources
            .get(id)
            .ok_or_else(|| format!("effect {id} is not loaded in the preview"))?;
        let effect = &source.effect;
        if source.blobs.len() != effect.passes.len() {
            return Err(format!(
                "effect {id}: {} blobs for {} passes",
                source.blobs.len(),
                effect.passes.len()
            ));
        }
        let mut passes = Vec::new();
        for (decl, blob) in effect.passes.iter().zip(&source.blobs) {
            let spirv = words(blob).map_err(|e| format!("effect {id}, pass {}: {e}", decl.name))?;
            let threads = decl.threads.map_or((8, 8), |t| (t[0].max(1), t[1].max(1)));
            let label = format!("{id}/{}", decl.name);
            let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&label),
                source: wgpu::ShaderSource::SpirV(spirv.into()),
            });
            if let Some(name) = &decl.output
                && !effect.textures.iter().any(|t| &t.name == name)
            {
                return Err(format!("effect {id}, pass {}: no texture {name}", decl.name));
            }
            let layout = self.layout.clone();
            let pipeline_layout = self
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[Some(&layout)],
                    immediate_size: 0,
                });
            let pipeline = self
                .device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(&format!("{id}/{}", decl.name)),
                    layout: Some(&pipeline_layout),
                    module: &module,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                });
            self.take_errors()
                .map_err(|e| format!("effect {id}, pass {}: {e}", decl.name))?;
            passes.push(PassPipe {
                pipeline,
                layout,
                threads,
            });
        }
        let mut images = HashMap::new();
        for (name, w, h, rgba) in &source.images {
            let tex = Tex::new(&self.device, name, *w, *h, BYTE);
            self.queue.write_texture(
                tex.texture.as_image_copy(),
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(*h),
                },
                wgpu::Extent3d {
                    width: *w,
                    height: *h,
                    depth_or_array_layers: 1,
                },
            );
            images.insert(name.clone(), tex);
        }
        Ok(EffectGpu {
            effect: effect.clone(),
            passes,
            images,
        })
    }

    fn build_instances(&mut self, width: u32, height: u32) -> Result<(), String> {
        for request in &self.requested {
            if !self.effects.contains_key(&request.effect) {
                let built = self.build_effect(&request.effect)?;
                self.effects.insert(request.effect.clone(), built);
            }
        }
        let mut instances = Vec::new();
        for request in &self.requested {
            let fx = &self.effects[&request.effect];
            let mut slots = Vec::new();
            for decl in fx.effect.textures.iter().filter(|t| t.file.is_none()) {
                let (w, h) = match (decl.width, decl.height, decl.scale) {
                    (Some(w), Some(h), _) => (w.max(1), h.max(1)),
                    (_, _, Some(s)) => (scaled(width, s), scaled(height, s)),
                    _ => (width, height),
                };
                let format = format_of(decl);
                let mips = mip_levels(w, h, decl.mips.unwrap_or(1));
                let shadow = fx.effect.passes.iter().any(|p| {
                    p.output.as_deref() == Some(decl.name.as_str()) && p.inputs.contains(&decl.name)
                });
                slots.push(Slot {
                    name: decl.name.clone(),
                    tex: Tex::with_mips(&self.device, &decl.name, w, h, format, mips),
                    shadow: shadow.then(|| Tex::with_mips(&self.device, &decl.name, w, h, format, mips)),
                    narrow: narrow_of(decl)
                        .map(|entry| (entry, Tex::new(&self.device, &decl.name, w, h, format))),
                    persistent: decl.persistent,
                });
            }
            instances.push(Inst {
                effect: request.effect.clone(),
                params: request.params.clone(),
                slots,
            });
        }
        self.instances = instances;
        self.instances_dirty = false;
        Ok(())
    }

    fn ensure_targets(&mut self, width: u32, height: u32) {
        if self
            .targets
            .as_ref()
            .is_some_and(|t| t.width == width && t.height == height)
        {
            return;
        }
        let row_bytes = (width * 4).div_ceil(256) * 256;
        self.targets = Some(Targets {
            width,
            height,
            upload: Tex::new(&self.device, "upload", width, height, BYTE),
            original: Tex::new(&self.device, "original", width, height, PICTURE),
            previous: Tex::new(&self.device, "previous", width, height, PICTURE),
            pictures: [
                Tex::new(&self.device, "picture a", width, height, PICTURE),
                Tex::new(&self.device, "picture b", width, height, PICTURE),
            ],
            out: Tex::new(&self.device, "out", width, height, BYTE),
            readback: self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback"),
                size: u64::from(row_bytes) * u64::from(height),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            row_bytes,
        });
        self.instances_dirty = true;
        self.last_frame = None;
    }

    /// The frame with the instances applied. A frame up to `STEP` frames after the last one rendered
    /// continues it, as playback that drops frames does; any other frame (a scrub back or a long jump)
    /// resets `previous` and the persistent textures.
    pub fn render(&mut self, frame: &Frame) -> Result<Frame, String> {
        if frame.rgba.len() != (frame.width * frame.height * 4) as usize {
            return Err("the frame's pixel data does not match its size".into());
        }
        self.ensure_targets(frame.width, frame.height);
        if self.instances_dirty {
            self.build_instances(frame.width, frame.height)?;
        }
        let step = self
            .last_frame
            .filter(|&l| frame.index > l && frame.index - l <= STEP)
            .map(|l| frame.index - l);
        let consecutive = step.is_some();

        let total_passes: usize = self
            .instances
            .iter()
            .map(|i| self.effects[&i.effect].passes.len())
            .sum();
        if total_passes as u64 > self.uniform_slots {
            self.uniform_slots = (total_passes as u64).next_power_of_two();
            self.uniforms = Self::uniform_buffer(&self.device, self.uniform_slots);
        }

        let targets = self.targets.as_ref().ok_or("no render targets")?;
        let (width, height) = (targets.width, targets.height);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("preview"),
            });

        self.queue.write_texture(
            targets.upload.texture.as_image_copy(),
            &frame.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        if !consecutive {
            clear(&mut encoder, &targets.previous);
            for inst in &self.instances {
                for slot in &inst.slots {
                    if slot.persistent {
                        clear(&mut encoder, &slot.tex);
                        if let Some(shadow) = &slot.shadow {
                            clear(&mut encoder, shadow);
                        }
                    }
                }
            }
        }

        let group = |pipeline: &wgpu::ComputePipeline, entries: &[wgpu::BindGroupEntry]| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipeline.get_bind_group_layout(0),
                entries,
            })
        };
        let dispatch = |encoder: &mut wgpu::CommandEncoder,
                        pipeline: &wgpu::ComputePipeline,
                        bind: &wgpu::BindGroup,
                        w: u32,
                        h: u32,
                        threads: (u32, u32)| {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.dispatch_workgroups(w.div_ceil(threads.0), h.div_ceil(threads.1), 1);
        };

        let up = group(
            &self.up,
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&targets.upload.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&targets.original.view),
                },
            ],
        );
        dispatch(&mut encoder, &self.up, &up, width, height, (8, 8));

        let picture_tex = |p: Picture| match p {
            Picture::Original => &targets.original,
            Picture::A => &targets.pictures[0],
            Picture::B => &targets.pictures[1],
        };

        let mut current = Picture::Original;
        let mut global_pass = 0u32;
        let mut uniform_writes: Vec<(u64, [u32; UNIFORM_DWORDS])> = Vec::new();
        let mut bind_groups: Vec<(usize, usize, wgpu::BindGroup, u32, u32)> = Vec::new();
        let mut steps: Vec<Step> = Vec::new();

        let time = frame.index as f32 / self.fps;
        let frametime = step.unwrap_or(1) as f32 / self.fps;
        let random = (frame.index as u32)
            .wrapping_mul(2_654_435_761)
            .wrapping_add(0x9e37_79b9);

        for (ii, inst) in self.instances.iter().enumerate() {
            let fx = &self.effects[&inst.effect];
            for (pi, (decl, pipe)) in fx.effect.passes.iter().zip(&fx.passes).enumerate() {
                let slot_of = |name: &str| inst.slots.iter().find(|s| s.name == name);
                let writes_picture = decl.output.is_none();
                let output = match &decl.output {
                    None => {
                        let target = if current == Picture::A {
                            Picture::B
                        } else {
                            Picture::A
                        };
                        &picture_tex(target).view
                    }
                    Some(name) => {
                        &slot_of(name)
                            .ok_or_else(|| format!("effect {}: no texture {name}", inst.effect))?
                            .tex
                            .store
                    }
                };
                let (out_w, out_h) = match &decl.output {
                    None => (width, height),
                    Some(name) => {
                        let s = slot_of(name).ok_or("missing texture")?;
                        (s.tex.width, s.tex.height)
                    }
                };
                let mut inputs: Vec<&wgpu::TextureView> = Vec::new();
                for name in &decl.inputs {
                    let view = match name.as_str() {
                        "original" => &targets.original.view,
                        "previous" => &targets.previous.view,
                        other => {
                            if let Some(s) = slot_of(other) {
                                if decl.output.as_deref() == Some(other) {
                                    match &s.shadow {
                                        Some(shadow) => {
                                            steps.push(Step::Shadow(ii, other.to_string()));
                                            &shadow.view
                                        }
                                        None => &s.tex.view,
                                    }
                                } else {
                                    &s.tex.view
                                }
                            } else if let Some(image) = fx.images.get(other) {
                                &image.view
                            } else {
                                return Err(format!(
                                    "effect {}: pass {} reads unknown texture {other}",
                                    inst.effect, decl.name
                                ));
                            }
                        }
                    };
                    inputs.push(view);
                }
                if inputs.len() > INPUTS {
                    return Err(format!(
                        "effect {}: pass {} has more than {INPUTS} inputs",
                        inst.effect, decl.name
                    ));
                }

                let scene = picture_tex(current);
                let mut u = [0u32; UNIFORM_DWORDS];
                u[0] = out_w;
                u[1] = out_h;
                u[2] = (1.0 / out_w as f32).to_bits();
                u[3] = (1.0 / out_h as f32).to_bits();
                u[4] = time.to_bits();
                u[5] = frame.index as u32;
                u[6] = 0;
                u[7] = global_pass;
                for (i, v) in inst.params.iter().take(32).enumerate() {
                    if i < 16 {
                        u[8 + i] = v.to_bits();
                    } else {
                        u[36 + (i - 16)] = v.to_bits();
                    }
                }
                u[24] = 1;
                u[25] = 1;
                u[26] = 0;
                u[27] = frametime.to_bits();
                u[28] = scene.width;
                u[29] = scene.height;
                u[30] = random;
                u[31] = self.view;
                u[32] = 0;
                u[33] = u32::from(self.view != 0);
                u[34] = u32::from(consecutive);
                let offset = u64::from(global_pass) * UNIFORM_STRIDE;
                uniform_writes.push((offset, u));

                let mut entries = vec![
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.uniforms,
                            offset,
                            size: NonZeroU64::new(UNIFORM_BYTES),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 10,
                        resource: wgpu::BindingResource::TextureView(&scene.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 11,
                        resource: wgpu::BindingResource::TextureView(&self.depth.view),
                    },
                ];
                for i in 0..INPUTS {
                    entries.push(wgpu::BindGroupEntry {
                        binding: 12 + i as u32,
                        resource: wgpu::BindingResource::TextureView(
                            inputs.get(i).copied().unwrap_or(&self.dummy.view),
                        ),
                    });
                }
                entries.push(wgpu::BindGroupEntry {
                    binding: 30,
                    resource: wgpu::BindingResource::TextureView(output),
                });
                for (i, s) in self.samplers.iter().enumerate() {
                    entries.push(wgpu::BindGroupEntry {
                        binding: 40 + i as u32,
                        resource: wgpu::BindingResource::Sampler(s),
                    });
                }
                let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&decl.name),
                    layout: &pipe.layout,
                    entries: &entries,
                });
                bind_groups.push((ii, pi, bind, out_w, out_h));
                steps.push(Step::Pass(bind_groups.len() - 1));
                if let Some(name) = &decl.output
                    && slot_of(name).is_some_and(|s| s.narrow.is_some())
                {
                    steps.push(Step::Narrow(ii, name.clone()));
                }
                if let Some(name) = &decl.output
                    && slot_of(name).is_some_and(|s| s.tex.mips > 1)
                {
                    steps.push(Step::Mips(ii, name.clone()));
                }
                if writes_picture {
                    current = if current == Picture::A {
                        Picture::B
                    } else {
                        Picture::A
                    };
                }
                global_pass += 1;
            }
        }

        for (offset, words) in &uniform_writes {
            let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
            self.queue.write_buffer(&self.uniforms, *offset, &bytes);
        }
        for step in &steps {
            match step {
                Step::Shadow(ii, name) => {
                    let slot = self.instances[*ii]
                        .slots
                        .iter()
                        .find(|s| &s.name == name)
                        .ok_or("missing texture")?;
                    if let Some(shadow) = &slot.shadow {
                        slot.tex.copy_to(&mut encoder, shadow);
                    }
                }
                Step::Pass(bi) => {
                    let (ii, pi, bind, w, h) = &bind_groups[*bi];
                    let fx = &self.effects[&self.instances[*ii].effect];
                    let pipe = &fx.passes[*pi];
                    dispatch(&mut encoder, &pipe.pipeline, bind, *w, *h, pipe.threads);
                }
                Step::Mips(ii, name) => {
                    let slot = self.instances[*ii]
                        .slots
                        .iter()
                        .find(|s| &s.name == name)
                        .ok_or("missing texture")?;
                    for level in 1..slot.tex.mips as usize {
                        let bind = group(
                            &self.mip,
                            &[
                                wgpu::BindGroupEntry {
                                    binding: 6,
                                    resource: wgpu::BindingResource::TextureView(&slot.tex.levels[level - 1]),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 7,
                                    resource: wgpu::BindingResource::TextureView(&slot.tex.levels[level]),
                                },
                            ],
                        );
                        dispatch(
                            &mut encoder,
                            &self.mip,
                            &bind,
                            (slot.tex.width >> level).max(1),
                            (slot.tex.height >> level).max(1),
                            (8, 8),
                        );
                    }
                }
                Step::Narrow(ii, name) => {
                    let slot = self.instances[*ii]
                        .slots
                        .iter()
                        .find(|s| &s.name == name)
                        .ok_or("missing texture")?;
                    if let Some((entry, scratch)) = &slot.narrow {
                        slot.tex.copy_to(&mut encoder, scratch);
                        let pipe = &self.narrow[entry];
                        let bind = group(
                            pipe,
                            &[
                                wgpu::BindGroupEntry {
                                    binding: 4,
                                    resource: wgpu::BindingResource::TextureView(&scratch.view),
                                },
                                wgpu::BindGroupEntry {
                                    binding: 5,
                                    resource: wgpu::BindingResource::TextureView(&slot.tex.store),
                                },
                            ],
                        );
                        dispatch(&mut encoder, pipe, &bind, slot.tex.width, slot.tex.height, (8, 8));
                    }
                }
            }
        }

        let final_picture = picture_tex(current);
        let down = group(
            &self.down,
            &[
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&final_picture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&targets.out.view),
                },
            ],
        );
        dispatch(&mut encoder, &self.down, &down, width, height, (8, 8));
        final_picture.copy_to(&mut encoder, &targets.previous);
        encoder.copy_texture_to_buffer(
            targets.out.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &targets.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(targets.row_bytes),
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

        let slice = targets.readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        {
            let data = slice.get_mapped_range().map_err(|e| e.to_string())?;
            for row in 0..height as usize {
                let start = row * targets.row_bytes as usize;
                rgba.extend_from_slice(&data[start..start + (width * 4) as usize]);
            }
        }
        targets.readback.unmap();
        self.take_errors()?;
        self.last_frame = Some(frame.index);
        Ok(Frame {
            index: frame.index,
            width,
            height,
            rgba,
        })
    }
}

enum Step {
    Mips(usize, String),
    Shadow(usize, String),
    Pass(usize),
    Narrow(usize, String),
}
