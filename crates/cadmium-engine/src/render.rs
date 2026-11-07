use wgpu::util::DeviceExt;

use crate::canvas::{Canvas, Primer, StateTexture, TILE_CHANGE_HEADER_WORDS, TILE_SIZE, TileCoord};
use crate::compute::{AutoLayoutStage, buffer_binding, texture_binding};
use crate::gpu::GpuContext;
use crate::params::SimParams;
use crate::spectral::{BANDS, band_xyz_weights};

const SHADE_BODY: &str = concat!(
    include_str!("shaders/pigment_slots.wgsl"),
    include_str!("shaders/slot_lane.wgsl"),
    include_str!("shaders/kubelka_munk.wgsl"),
    include_str!("shaders/shade.wgsl")
);
const TILE_CHANGES_BINDING: u32 = 10;
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
    first_possible_tile: u32,
    /// Nonzero on the tray: its bare plate catches the glass's reflection.
    glass: u32,
    padding: [u32; 3],
}

impl ShadeParams {
    fn new(params: &SimParams, mode: ViewMode, first_possible_tile: u32, primer: Primer) -> Self {
        let (view_mode, view_slot) = mode.shader_selector();
        ShadeParams {
            xyz_weights: xyz_weight_vec4s(),
            impasto_scale: params.get("impasto_scale"),
            light_azimuth_deg: params.get("light_azimuth_deg"),
            light_elevation_deg: params.get("light_elevation_deg"),
            wet_gloss: params.get("wet_gloss"),
            dry_roughness: params.get("dry_roughness"),
            view_mode,
            view_slot,
            first_possible_tile,
            glass: u32::from(primer == Primer::Tray),
            padding: [0; 3],
        }
    }
}

fn xyz_weight_vec4s() -> [[f32; 4]; XYZ_WEIGHT_VEC4S] {
    let weights = band_xyz_weights();
    let mut xyz_weights = [[0.0; 4]; XYZ_WEIGHT_VEC4S];
    for (channel, bands) in weights.iter().enumerate() {
        for (part, chunk) in bands.as_chunks::<BANDS_PER_VEC4>().0.iter().enumerate() {
            xyz_weights[channel * BANDS / BANDS_PER_VEC4 + part] = *chunk;
        }
    }
    xyz_weights
}

pub struct Renderer {
    compute: AutoLayoutStage,
}

pub struct ShadeRequest<'a> {
    pub params: &'a SimParams,
    pub mode: ViewMode,
}

impl Renderer {
    pub fn new(gpu: &GpuContext) -> Self {
        Renderer {
            compute: AutoLayoutStage::new(gpu, "shade", &shade_source()),
        }
    }

    pub fn encode_shade(
        &self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        request: ShadeRequest<'_>,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let tiles = shaded_tiles(canvas);
        if tiles.list.is_empty() {
            return;
        }
        let tile_list: Vec<[u32; 2]> = tiles.list.iter().map(|tile| [tile.x, tile.y]).collect();
        let tile_buffer = shade_tile_buffer(gpu, &tile_list);
        let params = ShadeParams::new(
            request.params,
            request.mode,
            tiles.first_possible,
            canvas.spec.primer,
        );
        let params_buffer = shade_params_buffer(gpu, &params);
        let bind_group = self.shade_bind_group(gpu, canvas, &params_buffer, &tile_buffer);
        self.compute.encode(
            encoder,
            &bind_group,
            [
                WORKGROUPS_PER_TILE_SIDE,
                WORKGROUPS_PER_TILE_SIDE,
                tile_list.len() as u32,
            ],
        );
        if tiles.first_possible < tile_list.len() as u32 {
            canvas.tile_changes().encode_forget(encoder);
        }
    }

    fn shade_bind_group(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        params_buffer: &wgpu::Buffer,
        tile_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        let mut entries = vec![
            buffer_binding(0, canvas.slot_table()),
            buffer_binding(1, params_buffer),
            buffer_binding(2, tile_buffer),
        ];
        let textures = SHADE_INPUTS.iter().chain([&StateTexture::Display]);
        entries.extend(
            (3..)
                .zip(textures)
                .map(|(binding, which)| texture_binding(binding, canvas, *which)),
        );
        entries.push(buffer_binding(
            TILE_CHANGES_BINDING,
            canvas.tile_changes().buffer(),
        ));
        self.compute.bind_group(gpu, &entries)
    }
}

fn shade_tile_buffer(gpu: &GpuContext, tile_list: &[[u32; 2]]) -> wgpu::Buffer {
    gpu.device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("shade_tiles"),
            contents: bytemuck::cast_slice(tile_list),
            usage: wgpu::BufferUsages::STORAGE,
        })
}

struct ShadedTiles {
    list: Vec<TileCoord>,
    first_possible: u32,
}

fn shaded_tiles(canvas: &mut Canvas) -> ShadedTiles {
    let possible: Vec<TileCoord> = canvas
        .possibly_dirty
        .take()
        .into_iter()
        .filter(|tile| !canvas.dirty.contains(*tile))
        .collect();
    let mut list = canvas.dirty.take();
    let first_possible = list.len() as u32;
    list.extend(possible);
    ShadedTiles {
        list,
        first_possible,
    }
}

fn shade_source() -> String {
    format!("const TILE_CHANGE_HEADER_WORDS: u32 = {TILE_CHANGE_HEADER_WORDS}u;\n{SHADE_BODY}")
}

fn shade_params_buffer(gpu: &GpuContext, params: &ShadeParams) -> wgpu::Buffer {
    gpu.device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("shade_params"),
            contents: bytemuck::bytes_of(params),
            usage: wgpu::BufferUsages::UNIFORM,
        })
}
