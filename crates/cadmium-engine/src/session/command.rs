use crate::engine::FULL_DIP;
use crate::gpu::{GpuContext, GpuLost, new_encoder};
use crate::pigments::Mixture;

use super::PaintSession;
use super::queue::{BrushChange, KnifeLoad, Queued};

const KNIFE_TOP_UP_TOLERANCE: f32 = 1e-3;

impl PaintSession {
    pub(super) fn has_backlog(&self) -> bool {
        self.queued.iter().any(Queued::holds_back_brush_changes)
    }

    pub(super) fn change_brush(&mut self, gpu: &GpuContext, change: BrushChange) {
        if self.has_backlog() {
            self.queued.push_back(Queued::Brush(change));
            return;
        }
        let mut encoder = new_encoder(gpu, "brush_change");
        self.encode_brush_change(gpu, change, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
    }

    pub(super) fn load_knife(&mut self, gpu: &GpuContext, load: KnifeLoad) -> Result<(), GpuLost> {
        if self.has_backlog() {
            self.queued.push_back(Queued::Knife(load));
            return Ok(());
        }
        self.run_knife_load(gpu, load)
    }

    pub(super) fn encode_brush_change(
        &mut self,
        gpu: &GpuContext,
        change: BrushChange,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        match change {
            BrushChange::ResizeTip { radius_px } => self.engine.resize_tip(radius_px),
            BrushChange::Reload { mixture } if mixture.is_empty() => {
                self.engine.encode_clean_brush(encoder);
            }
            BrushChange::Reload { mixture } => {
                self.engine.encode_load_reservoir(gpu, &mixture, encoder);
            }
            BrushChange::RestoreReserve { mixture } => {
                self.engine.encode_restore_reserve(gpu, &mixture, encoder);
            }
            BrushChange::Dip { mixture, amount } => {
                self.engine.encode_dip(gpu, &mixture, amount, encoder);
            }
            BrushChange::Clean { knife: true } => self.engine.encode_clean_knife(encoder),
            BrushChange::Clean { knife: false } => self.engine.encode_clean_brush(encoder),
        }
    }

    pub(super) fn run_knife_load(
        &mut self,
        gpu: &GpuContext,
        load: KnifeLoad,
    ) -> Result<(), GpuLost> {
        match load {
            KnifeLoad::Dip { mixture, amount } => self.engine.dip_knife(gpu, &mixture, amount),
            KnifeLoad::TopUp { mixture } => self.top_up_knife_now(gpu, &mixture),
        }
    }

    fn top_up_knife_now(&mut self, gpu: &GpuContext, mixture: &Mixture) -> Result<(), GpuLost> {
        let held = self.engine.knife_totals(gpu)?.volume as f32;
        let target = self.engine.knife_full_dip_volume(mixture.volume_um);
        match knife_top_up(held, target) {
            Some(amount) => self.engine.dip_knife(gpu, mixture, amount),
            None => Ok(()),
        }
    }
}

fn knife_top_up(volume: f32, target: f32) -> Option<f32> {
    if !target.is_finite() || target <= 0.0 || !volume.is_finite() {
        return None;
    }
    let missing = target - volume.max(0.0);
    (missing > target * KNIFE_TOP_UP_TOLERANCE).then(|| (missing / target).min(FULL_DIP))
}

#[cfg(test)]
mod tests;
