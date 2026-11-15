use cadmium_engine::brush::FULL_RESERVOIR_VOLUME_UM;
use cadmium_engine::canvas::PaintTotals;
use cadmium_engine::engine::BrushReadout;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::palette::{Recipe, normalised};
use cadmium_engine::pigments::Mixture;

use super::recipe::recipe_from_fractions;
use super::{AppState, EMPTY_MIXTURE, Easel, EaselId};

pub(super) fn fill_level(volume: f32, capacity: f32) -> f32 {
    if capacity > 0.0 && volume.is_finite() {
        (volume / capacity).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

pub(super) fn mean_mixture(totals: &PaintTotals) -> Mixture {
    if !totals.volume.is_finite() || totals.volume <= 0.0 {
        return EMPTY_MIXTURE;
    }
    Mixture {
        volume_um: totals.volume as f32,
        fractions: std::array::from_fn(|slot| (totals.pigment[slot] / totals.volume) as f32),
    }
}

impl Easel {
    pub fn palette_mixture(&self) -> Mixture {
        self.session.brush.reservoir_mixture(self.slot_count())
    }

    pub fn reserve_level(&self) -> f32 {
        fill_level(
            self.reserve_volume,
            self.session.engine.full_reserve_volume(),
        )
    }

    pub fn brush_is_clean(&self) -> bool {
        self.tip_mixture.is_empty() && self.reserve_volume <= 0.0
    }

    pub fn knife_level(&self) -> f32 {
        let capacity = self
            .session
            .engine
            .knife_full_dip_volume(FULL_RESERVOIR_VOLUME_UM);
        fill_level(self.knife_volume, capacity)
    }

    pub fn knife_is_clean(&self) -> bool {
        self.knife_volume <= 0.0
    }

    pub(super) fn clean_and_load(&mut self, gpu: &GpuContext) {
        let _ = self.session.clean_and_load(gpu);
    }

    pub(super) fn dip(&mut self, gpu: &GpuContext, mixture: &Mixture, amount: f32) {
        let _ = self.session.dip(gpu, mixture, amount);
    }

    pub(super) fn top_up(&mut self, gpu: &GpuContext) {
        if !self.holds_knife() {
            self.session.restore_reserve(gpu);
            return;
        }
        let _ = self.session.top_up_knife(gpu);
    }

    pub fn clean_tool(&mut self, gpu: &GpuContext) {
        self.put_down_stroke_tool(gpu);
        self.session.clean_brush(gpu);
        self.measure_brush_once_painted(gpu);
    }

    /// Loads the tool with the paint under `(x, y)`, giving that paint's mixture with the
    /// medium left out; `None` where there is none to pick.
    pub fn pick_colour(&mut self, gpu: &GpuContext, x: f32, y: f32) -> Option<Recipe> {
        let fractions = self.paint_fractions_at(gpu, x, y)?;
        self.session.brush.recipe = recipe_from_fractions(&fractions);
        self.clean_and_load(gpu);
        self.measure_brush(gpu);
        normalised(&fractions)
    }

    /// The mixture of the paint under `(x, y)` with the medium left out, leaving the tool as
    /// it is; `None` where there is none to pick.
    pub fn colour_at(&mut self, gpu: &GpuContext, x: f32, y: f32) -> Option<Recipe> {
        normalised(&self.paint_fractions_at(gpu, x, y)?)
    }

    fn paint_fractions_at(&mut self, gpu: &GpuContext, x: f32, y: f32) -> Option<Recipe> {
        self.put_down_stroke_tool(gpu);
        let Ok(Some(picked)) = self.session.pick_colour(gpu, x, y) else {
            return None;
        };
        Some(picked.fractions)
    }

    /// Reads the brush now, or once the frames have painted the dabs queued ahead of the
    /// change: reading it sooner would push the whole backlog to the GPU in one submission.
    pub(super) fn measure_brush_once_painted(&mut self, gpu: &GpuContext) {
        if self.session.has_queued_dabs() {
            self.brush_measure_waits = true;
        } else {
            self.measure_brush(gpu);
        }
    }

    /// Leaves the brush read to the next frame, once the dabs queued ahead are painted.
    pub(super) fn measure_brush_next_frame(&mut self) {
        self.brush_measure_waits = true;
    }

    /// Whether a deferred brush read is due on the next frame.
    pub fn brush_measure_due(&self) -> bool {
        self.brush_measure_waits && !self.session.has_queued_dabs()
    }

    /// Whether a deferred brush read landed since the last call, so the panels can refresh.
    pub fn take_brush_measured(&mut self) -> bool {
        std::mem::take(&mut self.brush_measured)
    }

    pub(super) fn measure_brush(&mut self, gpu: &GpuContext) -> Option<BrushReadout> {
        let readout = self.session.read_brush(gpu).ok()?;
        self.tip_mixture = readout.tip_mean;
        self.reserve_volume = readout.reserve.volume as f32;
        self.reserve_mixture = mean_mixture(&readout.reserve);
        self.knife_mixture = mean_mixture(&readout.knife);
        self.knife_volume = readout.knife.volume as f32;
        Some(readout)
    }
}

impl AppState {
    pub fn palette_mixture(&self) -> Mixture {
        self.painting.palette_mixture()
    }

    pub fn reserve_level(&self) -> f32 {
        self.painting.reserve_level()
    }

    pub fn brush_is_clean(&self) -> bool {
        self.painting.brush_is_clean()
    }

    pub fn knife_level(&self) -> f32 {
        self.painting.knife_level()
    }

    pub fn knife_is_clean(&self) -> bool {
        self.painting.knife_is_clean()
    }

    pub fn reload_brush(&mut self) {
        self.release(EaselId::Painting);
        let gpu = self.gpu.clone();
        self.painting.session.reload_brush(&gpu);
        self.painting.measure_brush_once_painted(&gpu);
        self.wake();
    }

    pub fn clean_brush(&mut self) {
        self.release(EaselId::Painting);
        let gpu = self.gpu.clone();
        self.painting.clean_tool(&gpu);
        self.wake();
    }

    /// Loads the tool on `id` with the paint under `(x, y)`; on the sheet with a pick armed,
    /// chooses that paint's colour instead and disarms. True when there was paint there.
    pub fn pick_colour(&mut self, id: EaselId, x: f32, y: f32) -> bool {
        if self.sheet_pick_is_armed(id) {
            return self.choose_from_sheet(x, y);
        }
        self.release(id);
        let gpu = self.gpu.clone();
        let picked = self.easel_mut(id).pick_colour(&gpu, x, y);
        self.wake();
        picked.is_some()
    }

    pub fn brush_measure_due(&self) -> bool {
        self.painting.brush_measure_due()
    }

    pub fn take_brush_measured(&mut self) -> bool {
        self.painting.take_brush_measured()
    }
}

#[cfg(test)]
mod tests;
