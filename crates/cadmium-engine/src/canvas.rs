use wgpu::util::DeviceExt;

use crate::gpu::{GpuContext, GpuLost, TexelRows, new_encoder, read_mapped, staging_buffer};
use crate::pigments::PigmentDb;

mod changes;
mod pigment_slots;
mod primer;
mod spec;
mod state_texture;
mod texels;
mod tiles;
mod weave;

pub(crate) use changes::{TILE_CHANGE_HEADER_WORDS, TileChanges};
pub use pigment_slots::PigmentSlotsUniform;
pub(crate) use primer::TRAY_REFLECTANCE;
pub use primer::{primer_reflectance, primer_srgb8};
pub use spec::{CanvasSpec, MAX_CANVAS_SIZE, Primer, Weave};
pub use state_texture::StateTexture;
pub use texels::{f32_to_half, half_to_f32};
pub use tiles::{DirtyTiles, TileCoord, tiles_in_rect};
pub use weave::weave_heights;

use texels::{decode_texels, encode_texels};

pub const TILE_SIZE: u32 = 64;

const BANDS_PER_VEC4: usize = 4;
const CHANNELS_PER_PIGMENT_TEXTURE: usize = 4;

/// A rectangle of texels: its top-left corner and its width and height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TexelBlock {
    pub origin: [u32; 2],
    pub size: [u32; 2],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaintTotals {
    pub volume: f64,
    pub pigment: [f64; 8],
}

impl PaintTotals {
    fn add_texel<'a>(&mut self, volume: f32, concentrations: impl Iterator<Item = &'a f32>) {
        let volume = f64::from(volume);
        self.volume += volume;
        for (slot, &c) in concentrations.enumerate() {
            self.pigment[slot] += f64::from(c) * volume;
        }
    }
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
    pub possibly_dirty: DirtyTiles,
    pub wet_tiles: DirtyTiles,
    changes: TileChanges,
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
            possibly_dirty: DirtyTiles::new(spec.width, spec.height),
            wet_tiles: DirtyTiles::new(spec.width, spec.height),
            changes: TileChanges::new(gpu, spec.width, spec.height),
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

    pub fn tooth_height_um(&self) -> f32 {
        weave::tooth_height_um(self.spec.weave)
    }

    pub fn view(&self, which: StateTexture) -> &wgpu::TextureView {
        &self.surfaces[which.index()].view
    }

    pub(crate) fn tile_changes(&self) -> &TileChanges {
        &self.changes
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
        let mut encoder = new_encoder(gpu, "canvas_readback");
        self.encode_copy_out(which, &staging, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        read_mapped(gpu, &staging, |padded| rows.unpad(padded))
    }

    pub fn write_bytes(&self, gpu: &GpuContext, which: StateTexture, data: &[u8]) {
        let whole = TexelBlock {
            origin: [0, 0],
            size: [self.spec.width, self.spec.height],
        };
        self.write_bytes_at(gpu, which, data, whole);
    }

    /// Writes `data`, the texels of `block`, into that block of the texture.
    pub fn write_bytes_at(
        &self,
        gpu: &GpuContext,
        which: StateTexture,
        data: &[u8],
        block: TexelBlock,
    ) {
        let TexelBlock { origin, size } = block;
        let row_bytes = size[0] * which.bytes_per_texel();
        assert_eq!(
            data.len(),
            (row_bytes * size[1]) as usize,
            "texel data does not cover the {}×{} {which:?} block",
            size[0],
            size[1]
        );
        assert!(
            origin[0] + size[0] <= self.spec.width && origin[1] + size[1] <= self.spec.height,
            "the {which:?} block does not fit the texture"
        );
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                origin: wgpu::Origin3d {
                    x: origin[0],
                    y: origin[1],
                    z: 0,
                },
                ..self.texture(which).as_image_copy()
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(size[1]),
            },
            wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
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
            totals.add_texel(v, low.iter().chain(high));
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
