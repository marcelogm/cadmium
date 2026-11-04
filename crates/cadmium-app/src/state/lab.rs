use cadmium_engine::engine::PaintEngine;
use cadmium_engine::params::{ParamGroup, SimParams};
use cadmium_engine::pigments::MAX_SLOTS;
use cadmium_engine::render::ViewMode;

use super::AppState;

pub(super) fn clamp_view_mode(mode: ViewMode, slot_count: usize) -> ViewMode {
    match mode {
        ViewMode::Pigment(slot) if usize::from(slot) >= slot_count => match slot_count {
            0 => ViewMode::Color,
            count => ViewMode::Pigment((count.min(MAX_SLOTS) - 1) as u8),
        },
        other => other,
    }
}

pub(super) fn is_render_param(key: &str) -> bool {
    SimParams::specs()
        .iter()
        .any(|spec| spec.key == key && spec.group == ParamGroup::Render)
}

impl AppState {
    pub fn set_view_mode(&mut self, mode: ViewMode) {
        let mode = clamp_view_mode(mode, self.slot_count());
        if self.session.view_mode != mode {
            self.session.view_mode = mode;
            self.session.canvas.dirty.mark_all();
            self.wake();
        }
    }

    pub fn set_param(&mut self, key: &str, value: f32) {
        self.session.engine.params_mut().set(key, value);
        if is_render_param(key) {
            self.reshade_both_surfaces();
        }
    }

    pub fn reset_params(&mut self) {
        *self.session.engine.params_mut() = SimParams::default();
        self.reshade_both_surfaces();
    }

    fn reshade_both_surfaces(&mut self) {
        self.session.canvas.dirty.mark_all();
        self.session.tray.dirty.mark_all();
        self.wake();
    }
}

#[cfg(test)]
mod tests;
