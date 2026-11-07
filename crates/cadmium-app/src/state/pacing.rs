use std::cell::OnceCell;

use cadmium_engine::frame_budget::DabBudget;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::gpu_timer::GpuFrameTimer;

/// Paces the dabs of a stroke over frames: learns from timed frames how long the GPU takes
/// per dab, and allows each frame as many as keep it near its target. A stroke that ends
/// with dabs still queued has its brush read once they are painted.
#[derive(Default)]
pub(crate) struct FramePacing {
    budget: DabBudget,
    timer: OnceCell<Option<GpuFrameTimer>>,
    timing: Option<usize>,
    pub(crate) brush_measure_waits: bool,
    /// Set when a deferred brush read lands; the window drains it to refresh the panels.
    pub(crate) brush_measured: bool,
}

impl FramePacing {
    pub(crate) fn allowance(&mut self, gpu: &GpuContext) -> usize {
        let finished = match self.timer.get() {
            Some(Some(timer)) => timer.finished(gpu),
            _ => Vec::new(),
        };
        for frame in finished {
            self.budget.observe(frame.work, frame.gpu_ms);
        }
        self.budget.allowance()
    }

    pub(crate) fn begin_frame(&mut self, gpu: &GpuContext, encoder: &mut wgpu::CommandEncoder) {
        let timer = self.timer.get_or_init(|| GpuFrameTimer::new(gpu));
        self.timing = timer.as_ref().and_then(|timer| timer.begin(encoder));
    }

    pub(crate) fn end_frame(&mut self, encoder: &mut wgpu::CommandEncoder, dabs: usize) {
        let (Some(slot), Some(Some(timer))) = (self.timing.take(), self.timer.get_mut()) else {
            return;
        };
        if dabs > 0 {
            timer.end(encoder, slot, dabs);
        }
    }

    pub(crate) fn frame_submitted(&self) {
        if let Some(Some(timer)) = self.timer.get() {
            timer.after_submit();
        }
    }

    #[cfg(test)]
    pub(crate) fn cost_per_dab_ms(&self) -> f64 {
        self.budget.cost_per_dab_ms()
    }
}
