use cadmium_engine::surface::Surface;

use cadmium_engine::render::ViewMode;

use super::{AppState, ProvisionalTrail};

impl AppState {
    pub fn encode_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.prepare_frame();
        self.advance_hold_by_clock(Surface::Painting);
        let now_ms = self.now_ms();
        self.pacing.begin_frame(&self.gpu, encoder);
        self.session.encode_frame(&self.gpu, now_ms, encoder);
        self.pacing
            .end_frame(encoder, self.session.last_frame_dabs());
        self.capture_provisional();
        self.after_frame();
    }

    pub fn encode_tray_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.prepare_frame();
        self.advance_hold_by_clock(Surface::Tray);
        let now_ms = self.now_ms();
        self.pacing.begin_frame(&self.gpu, encoder);
        self.session.encode_tray_frame(&self.gpu, now_ms, encoder);
        self.pacing
            .end_frame(encoder, self.session.last_frame_dabs());
        self.after_frame();
    }

    pub fn frame_submitted(&self) {
        self.pacing.frame_submitted();
    }

    /// The provisional paint over the texture this frame presents.
    pub fn provisional_trail(&self) -> Option<&ProvisionalTrail> {
        self.provisional.presented.as_ref()
    }

    fn capture_provisional(&mut self) {
        let pending = self.session.queued_dabs_on(Surface::Painting);
        let shown_in_colour = self.session.view_mode == ViewMode::Color;
        let colour = self
            .provisional
            .colour
            .filter(|_| shown_in_colour && !pending.is_empty());
        let opacity = self
            .provisional
            .fade
            .advance(self.now_ms(), colour.is_some());
        self.provisional.presented =
            colour.map(|colour| ProvisionalTrail::of(colour, &pending, self.view).faded(opacity));
    }

    fn prepare_frame(&mut self) {
        if self.pacing.brush_measure_waits && !self.session.has_queued_dabs() {
            self.pacing.brush_measure_waits = false;
            self.pacing.brush_measured = self.measure_brush().is_some();
        }
        let allowance = self.pacing.allowance(&self.gpu);
        self.session.limit_dabs_per_frame(Some(allowance));
    }

    fn after_frame(&mut self) {
        if !self.is_stroking() {
            self.session.brush.tool = self.resting_tool;
        }
        self.pen.samples_last_frame = std::mem::take(&mut self.samples_since_frame);
    }

    pub fn gpu_is_lost(&self) -> bool {
        self.gpu.is_lost()
    }
}

#[cfg(test)]
mod tests;
