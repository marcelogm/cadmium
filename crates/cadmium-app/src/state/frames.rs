use cadmium_engine::surface::Surface;

use super::AppState;

impl AppState {
    pub fn encode_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.prepare_frame();
        self.advance_hold_by_clock(Surface::Painting);
        let now_ms = self.now_ms();
        self.pacing.begin_frame(&self.gpu, encoder);
        self.session.encode_frame(&self.gpu, now_ms, encoder);
        self.pacing
            .end_frame(encoder, self.session.last_frame_dabs());
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

    fn prepare_frame(&mut self) {
        if self.pacing.brush_measure_waits && !self.session.has_queued_dabs() {
            self.pacing.brush_measure_waits = false;
            self.measure_brush();
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
