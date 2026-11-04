use cadmium_engine::brush::FULL_RESERVOIR_VOLUME_UM;
use cadmium_engine::canvas::PaintTotals;
use cadmium_engine::engine::{BrushReadout, FULL_DIP};
use cadmium_engine::pigments::Mixture;
use cadmium_engine::surface::Surface;

use super::recipe::{
    PARTIAL_DIP, PigmentClick, PigmentDip, plan_pigment_dip, recipe_from_fractions,
};
use super::{AppState, EMPTY_MIXTURE};

const KNIFE_TOP_UP_TOLERANCE: f32 = 1e-3;

pub(super) fn reserve_level(volume: f32, capacity: f32) -> f32 {
    if capacity > 0.0 && volume.is_finite() {
        (volume / capacity).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

pub(super) fn knife_top_up(volume: f32, target: f32) -> Option<f32> {
    if !target.is_finite() || target <= 0.0 || !volume.is_finite() {
        return None;
    }
    let missing = target - volume.max(0.0);
    (missing > target * KNIFE_TOP_UP_TOLERANCE).then(|| (missing / target).min(FULL_DIP))
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

impl AppState {
    pub fn palette_mixture(&self) -> Mixture {
        self.session.brush.reservoir_mixture(self.slot_count())
    }

    pub fn reserve_level(&self) -> f32 {
        reserve_level(
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
        reserve_level(self.knife_volume, capacity)
    }

    pub fn knife_is_clean(&self) -> bool {
        self.knife_volume <= 0.0
    }

    pub(super) fn clean_and_load(&mut self) {
        if !self.holds_knife() {
            self.session.reload_brush(&self.gpu);
            return;
        }
        self.session.clean_brush(&self.gpu);
        let mixture = self.palette_mixture();
        if !mixture.is_empty() {
            self.dip(&mixture, FULL_DIP);
        }
    }

    fn dip(&mut self, mixture: &Mixture, amount: f32) {
        if self.session.dip(&self.gpu, mixture, amount).is_err() {
            self.wake();
        }
    }

    pub(super) fn top_up(&mut self) {
        if !self.holds_knife() {
            self.session.restore_reserve(&self.gpu);
            return;
        }
        let mixture = self.palette_mixture();
        if mixture.is_empty() {
            return;
        }
        self.session.flush(&self.gpu);
        self.session.resize_tip();
        let Ok(totals) = self.session.engine.knife_totals(&self.gpu) else {
            self.wake();
            return;
        };
        let target = self.session.engine.knife_full_dip_volume(mixture.volume_um);
        if let Some(amount) = knife_top_up(totals.volume as f32, target) {
            self.dip(&mixture, amount);
        }
    }

    pub fn reload_brush(&mut self) {
        self.release();
        self.session.reload_brush(&self.gpu);
        self.measure_brush();
    }

    pub fn clean_brush(&mut self) {
        self.put_down_stroke_tool();
        self.session.clean_brush(&self.gpu);
        self.measure_brush();
        self.wake();
    }

    pub fn choose_pigment(&mut self, slot: usize, click: PigmentClick) {
        self.put_down_stroke_tool();
        let (recipe, dip) = plan_pigment_dip(&self.session.brush.recipe, slot, click);
        self.session.brush.recipe = recipe;
        match dip {
            PigmentDip::CleanAndLoad => self.clean_and_load(),
            PigmentDip::DipAgain => {
                let mixture = self.palette_mixture();
                self.dip(&mixture, FULL_DIP);
            }
            PigmentDip::AddOnTop(slot) => {
                let mut single = self.session.brush.clone();
                single.recipe = vec![(slot, 1.0)];
                let mixture = single.reservoir_mixture(self.slot_count());
                self.dip(&mixture, PARTIAL_DIP);
            }
        }
        self.measure_brush();
        self.wake();
    }

    pub fn pick_colour_on(&mut self, surface: Surface, x: f32, y: f32) -> bool {
        self.put_down_stroke_tool();
        let Ok(Some(picked)) = self.session.pick_colour_on(&self.gpu, surface, x, y) else {
            self.wake();
            return false;
        };
        self.session.brush.recipe = recipe_from_fractions(&picked.fractions);
        self.session.reload_brush(&self.gpu);
        if self.holds_knife() {
            self.clean_and_load();
        }
        self.measure_brush();
        self.wake();
        true
    }

    pub(super) fn measure_brush(&mut self) -> Option<BrushReadout> {
        let Ok(readout) = self.session.read_brush(&self.gpu) else {
            self.wake();
            return None;
        };
        self.tip_mixture = readout.tip_mean;
        self.reserve_volume = readout.reserve.volume as f32;
        self.reserve_mixture = mean_mixture(&readout.reserve);
        self.knife_mixture = mean_mixture(&readout.knife);
        self.knife_volume = readout.knife.volume as f32;
        Some(readout)
    }
}

#[cfg(test)]
mod tests;
