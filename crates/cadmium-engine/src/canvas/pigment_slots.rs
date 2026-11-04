use crate::pigments::{MAX_SLOTS, PigmentDb};
use crate::spectral::BandValues;

use super::BANDS_PER_VEC4;

const SLOT_UNIFORM_VEC4S: usize = 2 * MAX_SLOTS;

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
