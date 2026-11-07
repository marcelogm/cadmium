use std::time::Instant;

use cadmium_engine::canvas::PaintTotals;
use cadmium_engine::draw_up::DrawUpRequest;
use cadmium_engine::pigments::{MAX_SLOTS, mixture_srgb8};
use cadmium_engine::present::ViewTransform;
use cadmium_engine::stroke::{StrokeSample, Tool};
use cadmium_engine::surface::Surface;
use cadmium_engine::tube::Nozzle;

use super::stroke::StrokePreparation;
use super::{Contact, LoadRing, Loading, Squeezing};
use crate::state::AppState;
use crate::state::dip::mean_mixture;
use crate::state::recipe::recipe_from_fractions;

const LOAD_CANCEL_SCREEN_PX: f64 = 4.0;
const STILL_TUBE_SCREEN_PX: f32 = 3.0;
const MAX_HOLD_STEP_SECONDS: f32 = 0.1;
const FULL_LEVEL: f32 = 0.999;

fn load_cancelled(origin: (f64, f64), current: (f64, f64), threshold_px: f64) -> bool {
    (current.0 - origin.0).hypot(current.1 - origin.1) > threshold_px
}

fn held_on<T>(tool: Tool, on_knife: T, in_reserve: T) -> T {
    if tool.profile().holds_on_blade() {
        on_knife
    } else {
        in_reserve
    }
}

impl AppState {
    /// Keeps how the tray widget shows the tray, on a screen of `device_scale` device px
    /// per widget px; a view or a scale that isn't a scale is ignored.
    pub fn set_tray_view(&mut self, view: ViewTransform, device_scale: f64) {
        let zoom_is_a_scale = view.zoom.is_finite() && view.zoom > 0.0;
        if zoom_is_a_scale && device_scale.is_finite() && device_scale > 0.0 {
            self.tray_view = view;
            self.tray_device_scale = device_scale;
        }
    }

    /// Widget px per tray px: what screen distances on the tray are measured in.
    pub fn tray_screen_zoom(&self) -> f32 {
        self.tray_view.zoom / self.tray_device_scale as f32
    }

    pub(super) fn follow_squeezing(
        &mut self,
        squeezing: Squeezing,
        pen: StrokeSample,
        samples: &[StrokeSample],
    ) {
        self.contact = Some(Contact::Squeezing(Squeezing { pen, ..squeezing }));
        self.note_samples(samples);
        self.wake();
    }

    pub(super) fn follow_loading(
        &mut self,
        loading: Loading,
        pen: StrokeSample,
        samples: &[StrokeSample],
    ) {
        let threshold_px = LOAD_CANCEL_SCREEN_PX / f64::from(self.tray_screen_zoom());
        let cancelled_at = samples
            .iter()
            .position(|sample| load_cancelled(loading.origin, (sample.x, sample.y), threshold_px));
        if let Some(index) = cancelled_at {
            self.begin_prepared_stroke(
                Surface::Tray,
                loading.tool,
                samples[index],
                StrokePreparation::None,
            );
            self.add_samples(&samples[index + 1..]);
            return;
        }
        self.contact = Some(Contact::Loading(Loading { pen, ..loading }));
        self.note_samples(samples);
        self.wake();
    }

    pub fn advance_hold(&mut self, seconds: f32) {
        if seconds.is_nan() || seconds <= 0.0 {
            return;
        }
        let seconds = seconds.min(MAX_HOLD_STEP_SECONDS);
        let radius = self.session.brush.geometry.max_diameter_px / 2.0;
        let still_holding = match self.contact {
            Some(Contact::Squeezing(squeezing)) => {
                self.advance_squeezing(squeezing, seconds, radius);
                true
            }
            Some(Contact::Loading(loading)) => self.advance_loading(loading, seconds, radius),
            Some(Contact::Stroke { .. }) | None => false,
        };
        if still_holding {
            self.wake();
        }
    }

    fn advance_squeezing(&mut self, squeezing: Squeezing, seconds: f32, radius: f32) {
        let Squeezing {
            pen,
            nozzle_at,
            squeezed,
        } = squeezing;
        let squeezed_now = self.session.squeeze(
            &self.gpu,
            Nozzle {
                from: [nozzle_at.0 as f32, nozzle_at.1 as f32],
                to: [pen.x as f32, pen.y as f32],
                radius,
                slot: self.tube_slot,
                pressure: pen.pressure,
                seconds,
                still_reach_px: STILL_TUBE_SCREEN_PX / self.tray_screen_zoom(),
            },
        );
        self.contact = Some(Contact::Squeezing(Squeezing {
            pen,
            nozzle_at: (pen.x, pen.y),
            squeezed: squeezed || squeezed_now,
        }));
    }

    fn advance_loading(&mut self, loading: Loading, seconds: f32, radius: f32) -> bool {
        let pen = loading.pen;
        let request = DrawUpRequest {
            center: [pen.x as f32, pen.y as f32],
            radius,
            pressure: pen.pressure,
            seconds,
        };
        let Ok(drawn) = self.session.draw_up(&self.gpu, request) else {
            self.release();
            return false;
        };
        let taken = (drawn.taken.volume > 0.0).then(|| self.colour_of(&drawn.taken));
        self.contact = Some(Contact::Loading(Loading {
            colour: taken.or(loading.colour),
            level: Some(drawn.level),
            ..loading
        }));
        true
    }

    /// Advances the hold, which is always on the tray, by the time since the last tray frame.
    pub(in crate::state) fn advance_hold_by_clock(&mut self) {
        if !self.holds_on(Surface::Tray) {
            return;
        }
        let now = Instant::now();
        let seconds = self
            .hold_clock
            .replace(now)
            .map_or(0.0, |last| now.duration_since(last).as_secs_f32());
        self.advance_hold(seconds);
    }

    fn srgb8_of(&self, fractions: &[f32; MAX_SLOTS]) -> [u8; 3] {
        mixture_srgb8(
            &self.session.db,
            &self.session.canvas.spec.pigment_slots,
            fractions,
        )
    }

    fn colour_of(&self, totals: &PaintTotals) -> [u8; 3] {
        self.srgb8_of(&mean_mixture(totals).fractions)
    }

    fn held_level(&self, tool: Tool) -> f32 {
        held_on(tool, self.knife_level(), self.reserve_level())
    }

    pub(super) fn held_colour(&self, tool: Tool) -> Option<[u8; 3]> {
        let held = held_on(tool, &self.knife_mixture, &self.reserve_mixture);
        (!held.is_empty()).then(|| self.srgb8_of(&held.fractions))
    }

    pub fn load_ring(&self) -> Option<LoadRing> {
        let Some(Contact::Loading(Loading {
            tool,
            pen,
            colour,
            level,
            ..
        })) = self.contact
        else {
            return None;
        };
        let level = level.unwrap_or_else(|| self.held_level(tool));
        let colour = match colour {
            Some(taken) => taken,
            None if level >= FULL_LEVEL => self.held_colour(tool)?,
            None => return None,
        };
        Some(LoadRing {
            level,
            pressure: pen.pressure,
            colour,
        })
    }

    pub(super) fn finish_squeezing(&mut self, squeezed: bool) {
        self.session.end_hold();
        if squeezed {
            self.edits.record();
        }
    }

    pub(super) fn finish_loading(&mut self, tool: Tool, loaded: bool) {
        self.session.end_hold();
        let readout = self.measure_brush();
        if !loaded {
            return;
        }
        self.edits.record();
        let Some(readout) = readout else {
            return;
        };
        let held = held_on(tool, readout.knife, readout.reserve);
        let recipe = recipe_from_fractions(&mean_mixture(&held).fractions);
        if !recipe.is_empty() {
            self.session.brush.recipe = recipe;
        }
    }
}

#[cfg(test)]
mod tests;
