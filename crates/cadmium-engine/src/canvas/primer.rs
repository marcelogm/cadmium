use wgpu::util::DeviceExt;

use crate::compute::{AutoLayoutStage, buffer_binding, texture_binding};
use crate::gpu::{GpuContext, new_encoder};
use crate::pigments::{MAX_SLOTS, PigmentDb, mixture_reflectance};
use crate::spectral::{BandValues, km_infinite, reflectance_srgb8};

use super::{
    BANDS_PER_VEC4, Canvas, CanvasSpec, DirtyTiles, MARGIN_REFLECTANCE, Primer, StateTexture,
    TILE_SIZE, TexelBlock, TileCoord, weave_heights,
};

const PRIMER_WHITE_ID: &str = "titanium_white";
const IMPRIMATURA_PIGMENT_ID: &str = "burnt_sienna";
const IMPRIMATURA_FRACTION: f32 = 0.5;
const IMPRIMATURA_THICKNESS_UM: f32 = 12.0;
/// The paper ground's brightest band: just under full reflectance, so layers composited
/// over it stay well clear of the `1 − R·R` pole.
const PAPER_TOP_REFLECTANCE: f32 = 0.99;

const INIT_WORKGROUP_SIZE: u32 = 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct InitParams {
    primer_bands_low: [f32; 4],
    primer_bands_high: [f32; 4],
    margin_bands_low: [f32; 4],
    margin_bands_high: [f32; 4],
    painting_origin: [u32; 2],
    painting_size: [u32; 2],
    margin_only: u32,
    padding: [u32; 3],
}

const _: () = assert!(std::mem::size_of::<InitParams>() == 96);

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
        Primer::Paper => {
            let top = gesso.iter().copied().fold(0.0, f32::max);
            gesso.map(|band| band / top * PAPER_TOP_REFLECTANCE)
        }
    }
}

pub fn primer_srgb8(db: &PigmentDb, primer: Primer) -> [u8; 3] {
    reflectance_srgb8(&primer_reflectance(db, primer))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InitScope {
    WholeCanvas,
    MarginOnly,
}

impl Canvas {
    pub(super) fn initialise(&self, gpu: &GpuContext, db: &PigmentDb) {
        self.encode_init(gpu, db, InitScope::WholeCanvas);
        self.write_f32(
            gpu,
            StateTexture::DryHeight,
            &weave_heights(self.spec.weave, self.spec.width, self.spec.height),
        );
    }

    /// Puts the margin back as a new canvas has it, wet and dry paint and graphite gone,
    /// and leaves every texel of the painting alone.
    pub(crate) fn reset_margin(&mut self, gpu: &GpuContext, db: &PigmentDb) {
        self.encode_init(gpu, db, InitScope::MarginOnly);
        let heights = weave_heights(self.spec.weave, self.spec.width, self.spec.height);
        for strip in self.spec.margin_strips() {
            let bytes = strip_bytes(&heights, self.spec.width, strip);
            self.write_bytes_at(gpu, StateTexture::DryHeight, &bytes, strip);
        }
        let margin_tiles = self.spec.margin_tiles();
        for tile in &margin_tiles {
            mark_tile(&mut self.dirty, *tile);
        }
        self.margin_touched.clear();
        let still_wet: Vec<TileCoord> = self
            .wet_tiles
            .take()
            .into_iter()
            .filter(|tile| !self.spec.is_tile_wholly_in_margin(*tile))
            .collect();
        for tile in still_wet {
            mark_tile(&mut self.wet_tiles, tile);
        }
    }

    fn encode_init(&self, gpu: &GpuContext, db: &PigmentDb, scope: InitScope) {
        let params_buffer = init_params_buffer(gpu, db, &self.spec, scope);
        let stage = AutoLayoutStage::new(
            gpu,
            "canvas_init",
            include_str!("../shaders/canvas_init.wgsl"),
        );
        let mut entries = vec![buffer_binding(0, &params_buffer)];
        entries.extend(
            (1..)
                .zip(StateTexture::ALL)
                .map(|(binding, which)| texture_binding(binding, self, which)),
        );
        let bind_group = stage.bind_group(gpu, &entries);
        let mut encoder = new_encoder(gpu, "canvas_init");
        stage.encode(
            &mut encoder,
            &bind_group,
            [
                self.spec.width.div_ceil(INIT_WORKGROUP_SIZE),
                self.spec.height.div_ceil(INIT_WORKGROUP_SIZE),
                1,
            ],
        );
        gpu.queue.submit([encoder.finish()]);
    }
}

fn strip_bytes(heights: &[f32], width: u32, strip: TexelBlock) -> Vec<u8> {
    let [x, y] = strip.origin.map(|value| value as usize);
    let [strip_width, strip_height] = strip.size.map(|value| value as usize);
    let width = width as usize;
    (y..y + strip_height)
        .flat_map(|row| &heights[row * width + x..][..strip_width])
        .flat_map(|height| height.to_le_bytes())
        .collect()
}

fn mark_tile(tiles: &mut DirtyTiles, tile: TileCoord) {
    let x0 = (tile.x * TILE_SIZE) as f32;
    let y0 = (tile.y * TILE_SIZE) as f32;
    let last = (TILE_SIZE - 1) as f32;
    tiles.mark_rect(x0, y0, x0 + last, y0 + last);
}

fn init_params_buffer(
    gpu: &GpuContext,
    db: &PigmentDb,
    spec: &CanvasSpec,
    scope: InitScope,
) -> wgpu::Buffer {
    let reflectance = primer_reflectance(db, spec.primer);
    let painting = spec.painting_rect();
    let params = InitParams {
        primer_bands_low: std::array::from_fn(|band| reflectance[band]),
        primer_bands_high: std::array::from_fn(|band| reflectance[BANDS_PER_VEC4 + band]),
        margin_bands_low: [MARGIN_REFLECTANCE; BANDS_PER_VEC4],
        margin_bands_high: [MARGIN_REFLECTANCE; BANDS_PER_VEC4],
        painting_origin: [painting.x, painting.y],
        painting_size: [painting.width, painting.height],
        margin_only: u32::from(scope == InitScope::MarginOnly),
        padding: [0; 3],
    };
    gpu.device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("canvas_init_params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        })
}
