use super::bristles::disc_texel_count;
use super::{FULL_DIP, MAX_TIP_THICKNESS_UM, PIGMENT_CHANNELS, VOLUME_CHANNEL, WetOilEngine};
use crate::brush::{DEFAULT_LOAD, FULL_RESERVOIR_VOLUME_UM};
use crate::canvas::PaintTotals;
use crate::compute::read_buffer;
use crate::gpu::{GpuContext, GpuLost, new_encoder};
use crate::pigments::Mixture;

const DRAWN_LOADED_THICKNESS_UM: f32 = FULL_RESERVOIR_VOLUME_UM * DEFAULT_LOAD;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct DipRecord {
    fractions: [f32; PIGMENT_CHANNELS],
    tip_radius: f32,
    tip_add_um: f32,
    tip_limit_um: f32,
    body_add_um: f32,
    body_limit_um: f32,
    _pad: [f32; 3],
}

impl DipRecord {
    pub(super) fn measuring(tip_radius: f32) -> DipRecord {
        DipRecord {
            tip_radius,
            ..bytemuck::Zeroable::zeroed()
        }
    }
}

impl WetOilEngine {
    pub fn load_reservoir(&mut self, gpu: &GpuContext, mixture: &Mixture) {
        self.clean_brush(gpu);
        self.dip(gpu, mixture, FULL_DIP);
    }

    pub fn dip(&mut self, gpu: &GpuContext, mixture: &Mixture, amount: f32) {
        let Some(thickness) = dip_thickness(mixture) else {
            return;
        };
        if !amount.is_finite() || amount <= 0.0 {
            return;
        }
        let record = DipRecord {
            fractions: dip_fractions(mixture),
            tip_radius: self.tip_radius,
            tip_add_um: amount * thickness,
            tip_limit_um: MAX_TIP_THICKNESS_UM,
            body_add_um: amount * self.reserve_volume(thickness),
            body_limit_um: self.reserve_volume(MAX_TIP_THICKNESS_UM),
            _pad: [0.0; 3],
        };
        self.submit_fill(gpu, &record, true);
        self.loaded_thickness_um = thickness;
    }

    pub fn restore_reserve(&mut self, gpu: &GpuContext, mixture: &Mixture) {
        let Some(thickness) = dip_thickness(mixture) else {
            return;
        };
        let full_reserve = self.reserve_volume(thickness);
        let record = DipRecord {
            fractions: dip_fractions(mixture),
            tip_radius: self.tip_radius,
            tip_add_um: 0.0,
            tip_limit_um: 0.0,
            body_add_um: full_reserve,
            body_limit_um: full_reserve,
            _pad: [0.0; 3],
        };
        self.submit_fill(gpu, &record, false);
        self.loaded_thickness_um = thickness;
    }

    pub(crate) fn draw_up(
        &mut self,
        gpu: &GpuContext,
        into_knife: bool,
        take: impl FnOnce(f32) -> Result<PaintTotals, GpuLost>,
    ) -> Result<(PaintTotals, f32), GpuLost> {
        if into_knife {
            self.draw_up_into_knife(gpu, take)
        } else {
            self.draw_up_into_reserve(gpu, take)
        }
    }

    fn draw_up_into_reserve(
        &mut self,
        gpu: &GpuContext,
        take: impl FnOnce(f32) -> Result<PaintTotals, GpuLost>,
    ) -> Result<(PaintTotals, f32), GpuLost> {
        let held = self.reserve_totals(gpu)?.volume as f32;
        let capacity = self.full_reserve_volume();
        let taken = take(capacity - held)?;
        let drew_paint = holds_paint(&taken);
        if drew_paint {
            self.add_to_reserve(gpu, &taken);
        }
        if drew_paint && held <= 0.0 {
            self.loaded_thickness_um = self.loaded_thickness_um.max(DRAWN_LOADED_THICKNESS_UM);
        }
        Ok((taken, level_after(held, capacity, &taken)))
    }

    fn add_to_reserve(&self, gpu: &GpuContext, taken: &PaintTotals) {
        let record = DipRecord {
            fractions: std::array::from_fn(|slot| (taken.pigment[slot] / taken.volume) as f32),
            tip_radius: self.tip_radius,
            tip_add_um: 0.0,
            tip_limit_um: 0.0,
            body_add_um: taken.volume as f32,
            body_limit_um: f32::MAX,
            _pad: [0.0; 3],
        };
        self.submit_fill(gpu, &record, false);
    }

    fn submit_fill(&self, gpu: &GpuContext, record: &DipRecord, fills_tip: bool) {
        self.dip_pass
            .submit_fill(gpu, record, &self.buffers, fills_tip);
    }

    pub fn full_reserve_volume(&self) -> f32 {
        self.reserve_volume(FULL_RESERVOIR_VOLUME_UM)
    }

    pub(super) fn reserve_volume(&self, thickness_um: f32) -> f32 {
        self.params.get("reserve_factor") * thickness_um * disc_texel_count(self.tip_radius) as f32
    }

    pub fn clean_brush(&mut self, gpu: &GpuContext) {
        let mut encoder = new_encoder(gpu, "clean_brush");
        encoder.clear_buffer(&self.buffers.tip, 0, None);
        encoder.clear_buffer(&self.buffers.tip_delta, 0, None);
        encoder.clear_buffer(&self.buffers.body, 0, None);
        encoder.clear_buffer(&self.buffers.wanted, 0, None);
        encoder.clear_buffer(&self.buffers.capillary_pools, 0, None);
        gpu.queue.submit([encoder.finish()]);
        self.bristles.reset();
    }

    pub fn reservoir_mixture(&self, gpu: &GpuContext) -> Result<Mixture, GpuLost> {
        let body = read_buffer(gpu, &self.buffers.body)?;
        let volume = body[VOLUME_CHANNEL];
        let fractions = std::array::from_fn(|slot| share_of(body[slot], volume));
        Ok(Mixture {
            volume_um: self.loaded_thickness_um,
            fractions,
        })
    }
}

fn share_of(amount: f32, volume: f32) -> f32 {
    if volume > 0.0 { amount / volume } else { 0.0 }
}

pub(super) fn level_after(held_um: f32, capacity_um: f32, taken: &PaintTotals) -> f32 {
    if capacity_um > 0.0 {
        ((held_um + taken.volume as f32) / capacity_um).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

pub(super) fn holds_paint(totals: &PaintTotals) -> bool {
    totals.volume.is_finite() && totals.volume > 0.0
}

pub(super) fn dip_thickness(mixture: &Mixture) -> Option<f32> {
    let thickness = mixture.volume_um;
    (thickness.is_finite() && thickness > 0.0).then(|| thickness.min(MAX_TIP_THICKNESS_UM))
}

pub(super) fn dip_fractions(mixture: &Mixture) -> [f32; PIGMENT_CHANNELS] {
    mixture.fractions.map(|fraction| {
        if fraction.is_finite() {
            fraction.clamp(0.0, 1.0)
        } else {
            0.0
        }
    })
}
