use cadmium_engine::render::ViewMode;

use super::{AppState, EaselId, ProvisionalTrail};

impl AppState {
    pub fn encode_frame(&mut self, id: EaselId, encoder: &mut wgpu::CommandEncoder) {
        let gpu = self.gpu.clone();
        let allowance = self.pacing.allowance(&gpu);
        let now_ms = self.now_ms();
        let easel = self.easel_mut(id);
        easel.session.limit_dabs_per_frame(Some(allowance));
        let measuring = easel.brush_measure_due();
        self.pacing.begin_frame(&gpu, encoder);
        let easel = self.easel_mut(id);
        easel.encode_frame(&gpu, now_ms, encoder);
        let measure_failed = measuring && !easel.brush_measured;
        let dabs = easel.session.last_frame_dabs();
        self.pacing.end_frame(encoder, dabs);
        if id == EaselId::Painting {
            self.capture_provisional();
        }
        self.pen.samples_last_frame = std::mem::take(&mut self.samples_since_frame);
        if measure_failed {
            self.wake();
        }
    }

    pub fn frame_submitted(&self) {
        self.pacing.frame_submitted();
    }

    /// The provisional paint over the texture this frame presents.
    pub fn provisional_trail(&self) -> Option<&ProvisionalTrail> {
        self.provisional.presented.as_ref()
    }

    fn capture_provisional(&mut self) {
        let session = &self.painting.session;
        let pending = session.queued_dabs();
        let shown_in_colour = session.view_mode == ViewMode::Color;
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

    pub fn gpu_is_lost(&self) -> bool {
        self.gpu.is_lost()
    }
}

#[cfg(test)]
mod tests;
