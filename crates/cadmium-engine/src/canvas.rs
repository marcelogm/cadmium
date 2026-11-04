use std::collections::HashSet;

use wgpu::util::DeviceExt;

use crate::gpu::{GpuContext, GpuLost, TexelRows, read_mapped, staging_buffer};
use crate::pigments::{MAX_SLOTS, PigmentDb, mixture_reflectance};
use crate::spectral::{BandValues, km_infinite, reflectance_srgb8};

pub const TILE_SIZE: u32 = 64;
pub const MAX_CANVAS_SIZE: u32 = 4096;

const PRIMER_WHITE_ID: &str = "titanium_white";
const IMPRIMATURA_PIGMENT_ID: &str = "burnt_sienna";
const IMPRIMATURA_FRACTION: f32 = 0.5;
const IMPRIMATURA_THICKNESS_UM: f32 = 12.0;

const LINEN_AMPLITUDE_UM: f32 = 12.0;
const LINEN_PERIOD_PX: f32 = 6.0;
const COTTON_AMPLITUDE_UM: f32 = 6.0;
const COTTON_PERIOD_PX: f32 = 4.0;
const SMOOTH_PERIOD_PX: f32 = 1.0;
const WEAVE_BASE_SHARE: f32 = 0.85;
const WEAVE_JITTER_SHARE: f32 = 0.15;
const WEAVE_THREAD_SHARE: f32 = 0.5;
const WEAVE_MID_SHARE: f32 = 0.5;
const WEAVE_HEIGHT_STEP_UM: f32 = 1.0 / 64.0;
const PIXEL_CENTRE: f32 = 0.5;
const PCG_MULTIPLIER: u32 = 747796405;
const PCG_INCREMENT: u32 = 2891336453;
const PCG_OUTPUT_MULTIPLIER: u32 = 277803737;

const INIT_WORKGROUP_SIZE: u32 = 8;
const SLOT_UNIFORM_VEC4S: usize = 2 * MAX_SLOTS;
const BANDS_PER_VEC4: usize = 4;
const CHANNELS_PER_PIGMENT_TEXTURE: usize = 4;
const BYTES_PER_HALF: usize = 2;
const BYTES_PER_FLOAT: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Primer {
    WhiteGesso,
    SiennaImprimatura,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Weave {
    Linen,
    Cotton,
    Smooth,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CanvasSpec {
    pub width: u32,
    pub height: u32,
    pub primer: Primer,
    pub weave: Weave,
    pub pigment_slots: Vec<String>,
}

impl CanvasSpec {
    pub fn validate(&self, db: &PigmentDb) -> Result<(), String> {
        let allowed = 1..=MAX_CANVAS_SIZE;
        if !allowed.contains(&self.width) || !allowed.contains(&self.height) {
            return Err(format!(
                "canvas size {}×{} is outside 1..={MAX_CANVAS_SIZE}",
                self.width, self.height
            ));
        }
        if !(1..=MAX_SLOTS).contains(&self.pigment_slots.len()) {
            return Err(format!(
                "canvas has {} pigment slots, expected 1..={MAX_SLOTS}",
                self.pigment_slots.len()
            ));
        }
        let mut seen = HashSet::new();
        for id in &self.pigment_slots {
            if db.get(id).is_none() {
                return Err(format!("unknown pigment {id:?}"));
            }
            if !seen.insert(id.as_str()) {
                return Err(format!("pigment {id:?} appears in more than one slot"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StateTexture {
    WetPig0,
    WetPig1,
    WetVol,
    DryRefl0,
    DryRefl1,
    DryHeight,
    DryTopPig0,
    DryTopPig1,
    Display,
}

impl StateTexture {
    pub const ALL: [StateTexture; 9] = [
        StateTexture::WetPig0,
        StateTexture::WetPig1,
        StateTexture::WetVol,
        StateTexture::DryRefl0,
        StateTexture::DryRefl1,
        StateTexture::DryHeight,
        StateTexture::DryTopPig0,
        StateTexture::DryTopPig1,
        StateTexture::Display,
    ];

    pub const PAINT_STATE: [StateTexture; 8] = [
        StateTexture::WetPig0,
        StateTexture::WetPig1,
        StateTexture::WetVol,
        StateTexture::DryRefl0,
        StateTexture::DryRefl1,
        StateTexture::DryHeight,
        StateTexture::DryTopPig0,
        StateTexture::DryTopPig1,
    ];

    pub fn format(self) -> wgpu::TextureFormat {
        match self {
            StateTexture::WetVol | StateTexture::DryHeight => wgpu::TextureFormat::R32Float,
            _ => wgpu::TextureFormat::Rgba16Float,
        }
    }

    pub fn channels(self) -> u32 {
        match self.format() {
            wgpu::TextureFormat::R32Float => 1,
            _ => 4,
        }
    }

    pub fn bytes_per_texel(self) -> u32 {
        self.format()
            .block_copy_size(None)
            .expect("state textures are uncompressed colour formats")
    }

    fn label(self) -> &'static str {
        match self {
            StateTexture::WetPig0 => "wet_pig0",
            StateTexture::WetPig1 => "wet_pig1",
            StateTexture::WetVol => "wet_vol",
            StateTexture::DryRefl0 => "dry_refl0",
            StateTexture::DryRefl1 => "dry_refl1",
            StateTexture::DryHeight => "dry_height",
            StateTexture::DryTopPig0 => "dry_top_pig0",
            StateTexture::DryTopPig1 => "dry_top_pig1",
            StateTexture::Display => "display",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileCoord {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Debug)]
pub struct DirtyTiles {
    width: u32,
    height: u32,
    columns: u32,
    rows: u32,
    marked: Vec<bool>,
}

impl DirtyTiles {
    pub fn new(width: u32, height: u32) -> Self {
        let columns = width.div_ceil(TILE_SIZE);
        let rows = height.div_ceil(TILE_SIZE);
        DirtyTiles {
            width,
            height,
            columns,
            rows,
            marked: vec![false; (columns * rows) as usize],
        }
    }

    pub fn mark_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        for tile in tiles_in_rect(self.width, self.height, x0, y0, x1, y1) {
            let index = self.index(tile);
            self.marked[index] = true;
        }
    }

    pub fn mark_all(&mut self) {
        self.marked.fill(true);
    }

    pub fn take(&mut self) -> Vec<TileCoord> {
        let tiles = self.tiles();
        self.clear();
        tiles
    }

    pub fn tiles(&self) -> Vec<TileCoord> {
        let mut tiles = Vec::new();
        for x in 0..self.columns {
            for y in 0..self.rows {
                let tile = TileCoord { x, y };
                if self.marked[self.index(tile)] {
                    tiles.push(tile);
                }
            }
        }
        tiles
    }

    pub fn clear(&mut self) {
        self.marked.fill(false);
    }

    pub fn is_empty(&self) -> bool {
        !self.marked.contains(&true)
    }

    fn index(&self, tile: TileCoord) -> usize {
        (tile.y * self.columns + tile.x) as usize
    }
}

pub fn tiles_in_rect(
    width: u32,
    height: u32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
) -> Vec<TileCoord> {
    let (Some(columns), Some(rows)) = (tile_span(width, x0, x1), tile_span(height, y0, y1)) else {
        return Vec::new();
    };
    columns
        .flat_map(|x| rows.clone().map(move |y| TileCoord { x, y }))
        .collect()
}

fn tile_span(length: u32, a: f32, b: f32) -> Option<std::ops::RangeInclusive<u32>> {
    if length == 0 || a.is_nan() || b.is_nan() {
        return None;
    }
    let last_pixel = (length - 1) as f32;
    let first_pixel = a.min(b).floor();
    let final_pixel = a.max(b).floor();
    if final_pixel < 0.0 || first_pixel > last_pixel {
        return None;
    }
    let first = first_pixel.max(0.0) as u32 / TILE_SIZE;
    let last = final_pixel.min(last_pixel) as u32 / TILE_SIZE;
    Some(first..=last)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaintTotals {
    pub volume: f64,
    pub pigment: [f64; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PigmentSlotsUniform {
    pub absorption: [[f32; 4]; SLOT_UNIFORM_VEC4S],
    pub scattering: [[f32; 4]; SLOT_UNIFORM_VEC4S],
    pub count: u32,
    _pad: [u32; 3],
}

const _: () = assert!(std::mem::size_of::<PigmentSlotsUniform>() == 528);

impl PigmentSlotsUniform {
    pub fn new(db: &PigmentDb, slots: &[String]) -> Self {
        let mut uniform: PigmentSlotsUniform = bytemuck::Zeroable::zeroed();
        for (slot, id) in slots.iter().enumerate().take(MAX_SLOTS) {
            let pigment = db
                .get(id)
                .unwrap_or_else(|| panic!("unknown pigment id {id:?}"));
            write_bands(&mut uniform.absorption, slot, &pigment.absorption);
            write_bands(&mut uniform.scattering, slot, &pigment.scattering);
        }
        uniform.count = slots.len().min(MAX_SLOTS) as u32;
        uniform
    }
}

fn write_bands(table: &mut [[f32; 4]; SLOT_UNIFORM_VEC4S], slot: usize, values: &BandValues) {
    for (half, chunk) in values.as_chunks::<BANDS_PER_VEC4>().0.iter().enumerate() {
        table[2 * slot + half] = *chunk;
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct InitParams {
    primer_bands_low: [f32; 4],
    primer_bands_high: [f32; 4],
}

pub fn primer_reflectance(db: &PigmentDb, primer: Primer) -> BandValues {
    let white = db
        .get(PRIMER_WHITE_ID)
        .expect("titanium white is in the builtin pigment database");
    let gesso: BandValues =
        std::array::from_fn(|band| km_infinite(white.absorption[band], white.scattering[band]));
    match primer {
        Primer::WhiteGesso => gesso,
        Primer::SiennaImprimatura => {
            let mut fractions = [0.0; MAX_SLOTS];
            fractions[0] = IMPRIMATURA_FRACTION;
            mixture_reflectance(
                db,
                &[IMPRIMATURA_PIGMENT_ID.to_string()],
                &fractions,
                IMPRIMATURA_THICKNESS_UM,
                &gesso,
            )
        }
    }
}

pub fn primer_srgb8(db: &PigmentDb, primer: Primer) -> [u8; 3] {
    reflectance_srgb8(&primer_reflectance(db, primer))
}

fn weave_profile(weave: Weave) -> (f32, f32) {
    match weave {
        Weave::Linen => (LINEN_AMPLITUDE_UM, LINEN_PERIOD_PX),
        Weave::Cotton => (COTTON_AMPLITUDE_UM, COTTON_PERIOD_PX),
        Weave::Smooth => (0.0, SMOOTH_PERIOD_PX),
    }
}

fn pcg(seed: u32) -> u32 {
    let state = seed
        .wrapping_mul(PCG_MULTIPLIER)
        .wrapping_add(PCG_INCREMENT);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(PCG_OUTPUT_MULTIPLIER);
    (word >> 22) ^ word
}

fn cell_jitter(cell_x: i32, cell_y: i32) -> f32 {
    let mixed = pcg(cell_x.cast_unsigned() ^ pcg(cell_y.cast_unsigned()));
    WEAVE_BASE_SHARE + WEAVE_JITTER_SHARE * (mixed as f32 / u32::MAX as f32)
}

fn on_weave_height_grid(height: f32) -> f32 {
    (height / WEAVE_HEIGHT_STEP_UM).round() * WEAVE_HEIGHT_STEP_UM
}

pub fn weave_heights(weave: Weave, width: u32, height: u32) -> Vec<f32> {
    let (amplitude, period) = weave_profile(weave);
    let thread = |index: u32| {
        let position = index as f32 + PIXEL_CENTRE;
        (
            (std::f32::consts::TAU * position / period).sin(),
            (position / period).floor() as i32,
        )
    };
    let columns: Vec<(f32, i32)> = (0..width).map(thread).collect();
    let mut heights = Vec::with_capacity(width as usize * height as usize);
    for (row_thread, cell_y) in (0..height).map(thread) {
        heights.extend(columns.iter().map(|&(column_thread, cell_x)| {
            let threads = column_thread + row_thread;
            let jitter = cell_jitter(cell_x, cell_y);
            let raised =
                amplitude * WEAVE_MID_SHARE * (1.0 + WEAVE_THREAD_SHARE * threads * jitter);
            on_weave_height_grid(raised.clamp(0.0, amplitude))
        }));
    }
    heights
}

struct StateSurface {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasError {
    OutOfVideoMemory,
}

impl std::fmt::Display for CanvasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CanvasError::OutOfVideoMemory => write!(f, "not enough video memory for the canvas"),
        }
    }
}

impl std::error::Error for CanvasError {}

pub struct Canvas {
    pub spec: CanvasSpec,
    pub dirty: DirtyTiles,
    pub wet_tiles: DirtyTiles,
    surfaces: Vec<StateSurface>,
    slot_table: wgpu::Buffer,
}

impl Canvas {
    pub fn new(gpu: &GpuContext, db: &PigmentDb, spec: CanvasSpec) -> Result<Canvas, CanvasError> {
        if let Err(reason) = spec.validate(db) {
            panic!("invalid canvas spec: {reason}");
        }
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let canvas = Canvas::allocate(gpu, db, spec);
        match pollster::block_on(scope.pop()) {
            None => Ok(canvas),
            Some(_) => Err(CanvasError::OutOfVideoMemory),
        }
    }

    fn allocate(gpu: &GpuContext, db: &PigmentDb, spec: CanvasSpec) -> Canvas {
        let surfaces = StateTexture::ALL
            .iter()
            .map(|which| create_surface(gpu, &spec, *which))
            .collect();
        let slot_table = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("pigment_slots"),
                contents: bytemuck::bytes_of(&PigmentSlotsUniform::new(db, &spec.pigment_slots)),
                usage: wgpu::BufferUsages::UNIFORM
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            });
        let mut canvas = Canvas {
            dirty: DirtyTiles::new(spec.width, spec.height),
            wet_tiles: DirtyTiles::new(spec.width, spec.height),
            spec,
            surfaces,
            slot_table,
        };
        canvas.initialise(gpu, db);
        canvas.dirty.mark_all();
        canvas
    }

    pub fn texture(&self, which: StateTexture) -> &wgpu::Texture {
        &self.surfaces[which.index()].texture
    }

    pub fn view(&self, which: StateTexture) -> &wgpu::TextureView {
        &self.surfaces[which.index()].view
    }

    pub fn slot_table(&self) -> &wgpu::Buffer {
        &self.slot_table
    }

    pub fn add_slot(
        &mut self,
        gpu: &GpuContext,
        db: &PigmentDb,
        id: &str,
    ) -> Result<usize, String> {
        let mut grown = self.spec.clone();
        grown.pigment_slots.push(id.to_string());
        grown.validate(db)?;
        self.spec = grown;
        gpu.queue.write_buffer(
            &self.slot_table,
            0,
            bytemuck::bytes_of(&PigmentSlotsUniform::new(db, &self.spec.pigment_slots)),
        );
        Ok(self.spec.pigment_slots.len() - 1)
    }

    pub fn read_f32(&self, gpu: &GpuContext, which: StateTexture) -> Result<Vec<f32>, GpuLost> {
        Ok(decode_texels(which, &self.read_bytes(gpu, which)?))
    }

    pub fn write_f32(&self, gpu: &GpuContext, which: StateTexture, data: &[f32]) {
        self.write_bytes(gpu, which, &encode_texels(which, data));
    }

    pub fn texel_rows(&self, which: StateTexture) -> TexelRows {
        TexelRows::new(self.spec.width * which.bytes_per_texel(), self.spec.height)
    }

    pub fn encode_copy_out(
        &self,
        which: StateTexture,
        staging: &wgpu::Buffer,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        encoder.copy_texture_to_buffer(
            self.texture(which).as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: staging,
                layout: self.texel_rows(which).layout(),
            },
            self.extent(),
        );
    }

    pub fn read_bytes(&self, gpu: &GpuContext, which: StateTexture) -> Result<Vec<u8>, GpuLost> {
        let rows = self.texel_rows(which);
        let staging = staging_buffer(gpu, "canvas_readback", rows.padded_size());
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("canvas_readback"),
            });
        self.encode_copy_out(which, &staging, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        read_mapped(gpu, &staging, |padded| rows.unpad(padded))
    }

    pub fn write_bytes(&self, gpu: &GpuContext, which: StateTexture, data: &[u8]) {
        let row_bytes = self.spec.width * which.bytes_per_texel();
        assert_eq!(
            data.len(),
            (row_bytes * self.spec.height) as usize,
            "texel data does not cover the {which:?} texture"
        );
        gpu.queue.write_texture(
            self.texture(which).as_image_copy(),
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(self.spec.height),
            },
            self.extent(),
        );
    }

    pub fn wet_totals(&self, gpu: &GpuContext) -> Result<PaintTotals, GpuLost> {
        let volume = self.read_f32(gpu, StateTexture::WetVol)?;
        let low = self.read_f32(gpu, StateTexture::WetPig0)?;
        let high = self.read_f32(gpu, StateTexture::WetPig1)?;
        let mut totals = PaintTotals::default();
        let texels = volume
            .iter()
            .zip(low.as_chunks::<CHANNELS_PER_PIGMENT_TEXTURE>().0)
            .zip(high.as_chunks::<CHANNELS_PER_PIGMENT_TEXTURE>().0);
        for ((&v, low), high) in texels {
            let v = f64::from(v);
            totals.volume += v;
            for (slot, &c) in low.iter().chain(high).enumerate() {
                totals.pigment[slot] += f64::from(c) * v;
            }
        }
        Ok(totals)
    }

    fn extent(&self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.spec.width,
            height: self.spec.height,
            depth_or_array_layers: 1,
        }
    }

    fn initialise(&self, gpu: &GpuContext, db: &PigmentDb) {
        let primer = primer_reflectance(db, self.spec.primer);
        let params = InitParams {
            primer_bands_low: std::array::from_fn(|band| primer[band]),
            primer_bands_high: std::array::from_fn(|band| primer[BANDS_PER_VEC4 + band]),
        };
        let params_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("canvas_init_params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("canvas_init"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/canvas_init.wgsl").into()),
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("canvas_init"),
                layout: None,
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: params_buffer.as_entire_binding(),
        }];
        for (binding, which) in (1..).zip(StateTexture::ALL) {
            entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(self.view(which)),
            });
        }
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("canvas_init"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("canvas_init"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("canvas_init"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(
                self.spec.width.div_ceil(INIT_WORKGROUP_SIZE),
                self.spec.height.div_ceil(INIT_WORKGROUP_SIZE),
                1,
            );
        }
        gpu.queue.submit([encoder.finish()]);
        self.write_f32(
            gpu,
            StateTexture::DryHeight,
            &weave_heights(self.spec.weave, self.spec.width, self.spec.height),
        );
    }
}

fn create_surface(gpu: &GpuContext, spec: &CanvasSpec, which: StateTexture) -> StateSurface {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(which.label()),
        size: wgpu::Extent3d {
            width: spec.width,
            height: spec.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: which.format(),
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    StateSurface { texture, view }
}

fn decode_texels(which: StateTexture, bytes: &[u8]) -> Vec<f32> {
    match which.format() {
        wgpu::TextureFormat::R32Float => bytes
            .as_chunks::<BYTES_PER_FLOAT>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect(),
        _ => bytes
            .as_chunks::<BYTES_PER_HALF>()
            .0
            .iter()
            .map(|b| half_to_f32(u16::from_le_bytes(*b)))
            .collect(),
    }
}

fn encode_texels(which: StateTexture, values: &[f32]) -> Vec<u8> {
    match which.format() {
        wgpu::TextureFormat::R32Float => values.iter().flat_map(|v| v.to_le_bytes()).collect(),
        _ => values
            .iter()
            .flat_map(|v| f32_to_half(*v).to_le_bytes())
            .collect(),
    }
}

const HALF_SIGN_SHIFT: u32 = 16;
const HALF_SIGN_MASK: u16 = 0x8000;
const HALF_EXPONENT_ALL_ONES: i32 = 0x1f;
const HALF_EXPONENT_MASK: u16 = 0x7c00;
const HALF_MANTISSA_MASK: u16 = 0x03ff;
const HALF_MANTISSA_BITS: u32 = 10;
const HALF_EXPONENT_BIAS: i32 = 15;
const HALF_INFINITY: u16 = 0x7c00;
const HALF_QUIET_NAN: u16 = 0x7e00;
const HALF_MIN_SUBNORMAL_EXPONENT: i32 = -24;
const FLOAT_EXPONENT_BIAS: i32 = 127;
const FLOAT_MANTISSA_BITS: u32 = 23;
const FLOAT_EXPONENT_MASK: u32 = 0xff;
const FLOAT_MANTISSA_MASK: u32 = 0x007f_ffff;
const FLOAT_IMPLICIT_ONE: u32 = 0x0080_0000;
const MANTISSA_SHIFT: u32 = FLOAT_MANTISSA_BITS - HALF_MANTISSA_BITS;

pub fn half_to_f32(bits: u16) -> f32 {
    let sign = if bits & HALF_SIGN_MASK != 0 {
        -1.0
    } else {
        1.0
    };
    let exponent = i32::from((bits & HALF_EXPONENT_MASK) >> HALF_MANTISSA_BITS);
    let mantissa = f32::from(bits & HALF_MANTISSA_MASK);
    let mantissa_scale = (1u32 << HALF_MANTISSA_BITS) as f32;
    match exponent {
        0 => sign * mantissa * 2f32.powi(HALF_MIN_SUBNORMAL_EXPONENT),
        HALF_EXPONENT_ALL_ONES if mantissa == 0.0 => sign * f32::INFINITY,
        HALF_EXPONENT_ALL_ONES => f32::NAN,
        _ => sign * (1.0 + mantissa / mantissa_scale) * 2f32.powi(exponent - HALF_EXPONENT_BIAS),
    }
}

pub fn f32_to_half(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = (bits >> HALF_SIGN_SHIFT) as u16 & HALF_SIGN_MASK;
    let float_exponent = (bits >> FLOAT_MANTISSA_BITS) & FLOAT_EXPONENT_MASK;
    let mantissa = bits & FLOAT_MANTISSA_MASK;
    if float_exponent == FLOAT_EXPONENT_MASK {
        return sign
            | if mantissa == 0 {
                HALF_INFINITY
            } else {
                HALF_QUIET_NAN
            };
    }
    let exponent = float_exponent as i32 - FLOAT_EXPONENT_BIAS + HALF_EXPONENT_BIAS;
    if exponent >= HALF_EXPONENT_ALL_ONES {
        return sign | HALF_INFINITY;
    }
    if exponent <= 0 {
        let shift = (1 - exponent) as u32 + MANTISSA_SHIFT;
        if shift > FLOAT_MANTISSA_BITS + 1 {
            return sign;
        }
        let full = mantissa | FLOAT_IMPLICIT_ONE;
        return sign | round_shift_to_nearest_even(full, shift) as u16;
    }
    let combined = ((exponent as u32) << FLOAT_MANTISSA_BITS) | mantissa;
    sign | round_shift_to_nearest_even(combined, MANTISSA_SHIFT) as u16
}

fn round_shift_to_nearest_even(value: u32, shift: u32) -> u32 {
    let kept = value >> shift;
    let remainder = value & ((1 << shift) - 1);
    let halfway = 1 << (shift - 1);
    if remainder > halfway || (remainder == halfway && kept & 1 == 1) {
        kept + 1
    } else {
        kept
    }
}
