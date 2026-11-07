use cadmium_engine::brush::FULL_RESERVOIR_VOLUME_UM;
use cadmium_engine::canvas::PaintTotals;
use cadmium_engine::engine::{BrushReadout, FULL_DIP, Tip};
use cadmium_engine::pigments::Mixture;
use cadmium_engine::surface::Surface;

use super::recipe::{
    PARTIAL_DIP, PigmentClick, PigmentDip, plan_pigment_dip, recipe_from_fractions,
};
use super::{AppState, EMPTY_MIXTURE};

fn fill_level(volume: f32, capacity: f32) -> f32 {
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

impl AppState {
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
        if self.session.top_up_knife(&self.gpu).is_err() {
            self.wake();
        }
    }

    pub fn reload_brush(&mut self) {
        self.release();
        self.session.reload_brush(&self.gpu);
        self.measure_brush_once_painted();
    }

    pub fn clean_brush(&mut self) {
        self.put_down_stroke_tool();
        if self.shows_tray_tip() {
            self.session.clean_tray_tip(&self.gpu);
        } else {
            self.session.clean_brush(&self.gpu);
        }
        self.measure_brush_once_painted();
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
        self.measure_brush_once_painted();
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

    /// Reads the brush now, or once the frames have painted the dabs queued ahead of the
    /// change: reading it sooner would push the whole backlog to the GPU in one submission.
    pub(super) fn measure_brush_once_painted(&mut self) {
        if self.session.has_queued_dabs() {
            self.pacing.brush_measure_waits = true;
        } else {
            self.measure_brush();
        }
    }

    /// Whether a deferred brush read is due on the next frame.
    pub fn brush_measure_due(&self) -> bool {
        self.pacing.brush_measure_waits && !self.session.has_queued_dabs()
    }

    /// Whether a deferred brush read landed since the last call, so the panels can refresh.
    pub fn take_brush_measured(&mut self) -> bool {
        std::mem::take(&mut self.pacing.brush_measured)
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
        if self.focus == Surface::Tray {
            match self.session.read_tip(&self.gpu, Tip::Tray) {
                Ok(tray) => self.tray_tip_mixture = tray.tip_mean,
                Err(_) => self.wake(),
            }
        }
        Some(readout)
    }
}

#[cfg(test)]
mod tests;
