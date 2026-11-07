use wgpu::util::DeviceExt;

use crate::compute::{AutoLayoutStage, buffer_binding, texture_binding};
use crate::gpu::{GpuContext, new_encoder};
use crate::pigments::{MAX_SLOTS, PigmentDb, mixture_reflectance};
use crate::spectral::{BANDS, BandValues, km_infinite, reflectance_srgb8};

use super::{BANDS_PER_VEC4, Canvas, Primer, StateTexture, weave_heights};

const PRIMER_WHITE_ID: &str = "titanium_white";
const IMPRIMATURA_PIGMENT_ID: &str = "burnt_sienna";
const IMPRIMATURA_FRACTION: f32 = 0.5;
const IMPRIMATURA_THICKNESS_UM: f32 = 12.0;
/// The tray's mid grey under its glass: flat, so it has no tone, and shown near L* 52, the
/// middle value a painter judges light and dark against.
pub(crate) const TRAY_REFLECTANCE: f32 = 0.40;
/// The paper ground's brightest band: just under full reflectance, so layers composited
/// over it stay well clear of the `1 − R·R` pole.
const PAPER_TOP_REFLECTANCE: f32 = 0.99;

const INIT_WORKGROUP_SIZE: u32 = 8;

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
        Primer::Tray => [TRAY_REFLECTANCE; BANDS],
        Primer::Paper => {
            let top = gesso.iter().copied().fold(0.0, f32::max);
            gesso.map(|band| band / top * PAPER_TOP_REFLECTANCE)
        }
    }
}

pub fn primer_srgb8(db: &PigmentDb, primer: Primer) -> [u8; 3] {
    reflectance_srgb8(&primer_reflectance(db, primer))
}

impl Canvas {
    pub(super) fn initialise(&self, gpu: &GpuContext, db: &PigmentDb) {
        let params_buffer = init_params_buffer(gpu, db, self.spec.primer);
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
        self.write_f32(
            gpu,
            StateTexture::DryHeight,
            &weave_heights(self.spec.weave, self.spec.width, self.spec.height),
        );
    }
}

fn init_params_buffer(gpu: &GpuContext, db: &PigmentDb, primer: Primer) -> wgpu::Buffer {
    let reflectance = primer_reflectance(db, primer);
    let params = InitParams {
        primer_bands_low: std::array::from_fn(|band| reflectance[band]),
        primer_bands_high: std::array::from_fn(|band| reflectance[BANDS_PER_VEC4 + band]),
    };
    gpu.device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("canvas_init_params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM,
        })
}
