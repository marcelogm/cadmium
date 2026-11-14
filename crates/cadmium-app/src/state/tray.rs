use cadmium_engine::present::ViewTransform;
use cadmium_engine::session::ClearOutcome;

use super::AppState;
use super::recipe::PigmentClick;

impl AppState {
    pub fn slot_count(&self) -> usize {
        self.session.canvas.spec.pigment_slots.len()
    }

    pub fn add_pile(&mut self, id: &str) -> Result<usize, String> {
        let slot = self.session.add_slot(&self.gpu, id)?;
        self.edits.record();
        self.click_pile(slot, PigmentClick::CleanAndLoad);
        self.wake();
        Ok(slot)
    }

    pub fn click_pile(&mut self, slot: usize, click: PigmentClick) {
        self.choose_pigment(slot, click);
    }

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

    pub fn clear_tray(&mut self) {
        self.release();
        if let ClearOutcome::Cleared { .. } = self.session.clear_tray(&self.gpu) {
            self.edits.record();
        }
        self.wake();
    }
}

#[cfg(test)]
mod tests;
