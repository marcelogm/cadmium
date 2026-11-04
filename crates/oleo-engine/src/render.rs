use wgpu::util::DeviceExt;

use crate::canvas::{Canvas, StateTexture, TILE_SIZE};
use crate::gpu::GpuContext;
use crate::params::SimParams;
use crate::spectral::{BANDS, band_xyz_weights};

const SHADE_SOURCE: &str = concat!(
    include_str!("shaders/pigment_slots.wgsl"),
    include_str!("shaders/shade.wgsl")
);
const SHADE_WORKGROUP_SIZE: u32 = 8;
const WORKGROUPS_PER_TILE_SIDE: u32 = TILE_SIZE / SHADE_WORKGROUP_SIZE;
const BANDS_PER_VEC4: usize = 4;
const XYZ_WEIGHT_VEC4S: usize = 3 * BANDS / BANDS_PER_VEC4;

const VIEW_COLOR: u32 = 0;
const VIEW_HEIGHT: u32 = 1;
const VIEW_WETNESS: u32 = 2;
const VIEW_PIGMENT: u32 = 3;

const SHADE_INPUTS: [StateTexture; 6] = [
    StateTexture::WetPig0,
    StateTexture::WetPig1,
    StateTexture::WetVol,
    StateTexture::DryRefl0,
    StateTexture::DryRefl1,
    StateTexture::DryHeight,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Color,
    Height,
    Wetness,
    Pigment(u8),
}

impl ViewMode {
    fn shader_selector(self) -> (u32, u32) {
        match self {
            ViewMode::Color => (VIEW_COLOR, 0),
            ViewMode::Height => (VIEW_HEIGHT, 0),
            ViewMode::Wetness => (VIEW_WETNESS, 0),
            ViewMode::Pigment(slot) => (VIEW_PIGMENT, u32::from(slot)),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadeParams {
    xyz_weights: [[f32; 4]; XYZ_WEIGHT_VEC4S],
    impasto_scale: f32,
    light_azimuth_deg: f32,
    light_elevation_deg: f32,
    wet_gloss: f32,
    dry_roughness: f32,
    view_mode: u32,
    view_slot: u32,
    _pad: u32,
}

impl ShadeParams {
    fn new(params: &SimParams, mode: ViewMode) -> Self {
        let weights = band_xyz_weights();
        let mut xyz_weights = [[0.0; 4]; XYZ_WEIGHT_VEC4S];
        for (channel, bands) in weights.iter().enumerate() {
            for (part, chunk) in bands.as_chunks::<BANDS_PER_VEC4>().0.iter().enumerate() {
                xyz_weights[channel * BANDS / BANDS_PER_VEC4 + part] = *chunk;
            }
        }
        let (view_mode, view_slot) = mode.shader_selector();
        ShadeParams {
            xyz_weights,
            impasto_scale: params.get("impasto_scale"),
            light_azimuth_deg: params.get("light_azimuth_deg"),
            light_elevation_deg: params.get("light_elevation_deg"),
            wet_gloss: params.get("wet_gloss"),
            dry_roughness: params.get("dry_roughness"),
            view_mode,
            view_slot,
            _pad: 0,
        }
    }
}

pub struct Renderer {
    pipeline: wgpu::ComputePipeline,
}

impl Renderer {
    pub fn new(gpu: &GpuContext) -> Self {
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("shade"),
                source: wgpu::ShaderSource::Wgsl(SHADE_SOURCE.into()),
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("shade"),
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        Renderer { pipeline }
    }

    pub fn encode_shade(
        &self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        params: &SimParams,
        mode: ViewMode,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let tiles = canvas.dirty.take();
        if tiles.is_empty() {
            return;
        }
        let tile_list: Vec<[u32; 2]> = tiles.iter().map(|tile| [tile.x, tile.y]).collect();
        let tile_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("shade_tiles"),
                contents: bytemuck::cast_slice(&tile_list),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let params_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("shade_params"),
                contents: bytemuck::bytes_of(&ShadeParams::new(params, mode)),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: canvas.slot_table().as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: params_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: tile_buffer.as_entire_binding(),
            },
        ];
        let textures = SHADE_INPUTS.iter().chain([&StateTexture::Display]);
        for (binding, which) in (3..).zip(textures) {
            entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(canvas.view(*which)),
            });
        }
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shade"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("shade"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(
            WORKGROUPS_PER_TILE_SIDE,
            WORKGROUPS_PER_TILE_SIDE,
            tile_list.len() as u32,
        );
    }
}
