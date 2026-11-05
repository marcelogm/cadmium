use cadmium_engine::surface::Surface;

use super::AppState;

impl AppState {
    pub fn encode_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.advance_hold_by_clock(Surface::Painting);
        let now_ms = self.now_ms();
        self.session.encode_frame(&self.gpu, now_ms, encoder);
        self.after_frame();
    }

    pub fn encode_tray_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.advance_hold_by_clock(Surface::Tray);
        let now_ms = self.now_ms();
        self.session.encode_tray_frame(&self.gpu, now_ms, encoder);
        self.after_frame();
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
