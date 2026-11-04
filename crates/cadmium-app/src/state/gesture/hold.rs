use std::time::Instant;

use cadmium_engine::canvas::PaintTotals;
use cadmium_engine::draw_up::DrawUpRequest;
use cadmium_engine::pigments::mixture_srgb8;
use cadmium_engine::stroke::Tool;
use cadmium_engine::surface::Surface;
use cadmium_engine::tube::Nozzle;

use super::{Contact, LoadRing, Loading, Squeezing};
use crate::state::AppState;
use crate::state::dip::mean_mixture;
use crate::state::recipe::recipe_from_fractions;

pub(super) const LOAD_CANCEL_SCREEN_PX: f64 = 4.0;
const STILL_TUBE_SCREEN_PX: f32 = 3.0;
const MAX_HOLD_STEP_SECONDS: f32 = 0.1;
const FULL_LEVEL: f32 = 0.999;

pub(super) fn load_cancelled(origin: (f64, f64), current: (f64, f64), threshold_px: f64) -> bool {
    (current.0 - origin.0).hypot(current.1 - origin.1) > threshold_px
}

impl AppState {
    pub fn set_tray_zoom(&mut self, zoom: f32) {
        if zoom.is_finite() && zoom > 0.0 {
            self.tray_zoom = zoom;
        }
    }

    pub fn set_painting_device_scale(&mut self, scale: f32) {
        if scale.is_finite() && scale > 0.0 {
            self.painting_device_scale = scale;
        }
    }

    fn screen_zoom(&self, surface: Surface) -> f32 {
        match surface {
            Surface::Tray => self.tray_zoom,
            Surface::Painting => self.view.zoom / self.painting_device_scale,
        }
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
            surface,
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
                still_reach_px: STILL_TUBE_SCREEN_PX / self.screen_zoom(surface),
            },
        );
        self.contact = Some(Contact::Squeezing(Squeezing {
            surface,
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

    pub(in crate::state) fn advance_hold_by_clock(&mut self, frame_surface: Surface) {
        if self.hold_surface() != Some(frame_surface) {
            return;
        }
        let now = Instant::now();
        let seconds = self
            .hold_clock
            .replace(now)
            .map_or(0.0, |last| now.duration_since(last).as_secs_f32());
        self.advance_hold(seconds);
    }

    fn colour_of(&self, totals: &PaintTotals) -> [u8; 3] {
        mixture_srgb8(
            &self.session.db,
            &self.session.canvas.spec.pigment_slots,
            &mean_mixture(totals).fractions,
        )
    }

    fn held_level(&self, tool: Tool) -> f32 {
        if tool.profile().holds_on_blade() {
            self.knife_level()
        } else {
            self.reserve_level()
        }
    }

    fn held_colour(&self, tool: Tool) -> Option<[u8; 3]> {
        let held = if tool.profile().holds_on_blade() {
            &self.knife_mixture
        } else {
            &self.reserve_mixture
        };
        (!held.is_empty()).then(|| {
            mixture_srgb8(
                &self.session.db,
                &self.session.canvas.spec.pigment_slots,
                &held.fractions,
            )
        })
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

    pub(super) fn finish_loading(&mut self, tool: Tool, loaded: bool) {
        let readout = self.measure_brush();
        if !loaded {
            return;
        }
        self.edits.record();
        let Some(readout) = readout else {
            return;
        };
        let held = if tool.profile().holds_on_blade() {
            readout.knife
        } else {
            readout.reserve
        };
        let recipe = recipe_from_fractions(&mean_mixture(&held).fractions);
        if !recipe.is_empty() {
            self.session.brush.recipe = recipe;
        }
    }
}

#[cfg(test)]
mod tests;
