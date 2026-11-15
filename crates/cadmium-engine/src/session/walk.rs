use std::collections::BTreeSet;

use crate::brush::BrushSettings;
use crate::canvas::{Canvas, TileCoord, tiles_in_rect};
use crate::engine::{DabBatch, PaintEngine, WetOilEngine};
use crate::gpu::{GpuContext, new_encoder};
use crate::stroke::Dab;

use super::queue::{Queued, SweptDab, WalkProgress, WalkStep, next_step};
use super::{MotionAnchor, PaintSession};

impl PaintSession {
    pub fn flush(&mut self, gpu: &GpuContext) {
        if self.queued.is_empty() {
            return;
        }
        let mut encoder = new_encoder(gpu, "flush_queue");
        loop {
            match next_step(self.queued.front(), WalkProgress::unlimited()) {
                WalkStep::Wait => break,
                WalkStep::Barrier => self.submit_then_load_knife(gpu, &mut encoder),
                step => self.take_step(gpu, step, &mut encoder),
            }
        }
        gpu.queue.submit([encoder.finish()]);
    }

    pub(super) fn walk_frame(
        &mut self,
        gpu: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
    ) -> usize {
        let mut progress = WalkProgress::limited_to(self.frame_dab_limit);
        loop {
            match next_step(self.queued.front(), progress) {
                WalkStep::Wait => return progress.dabs_painted,
                WalkStep::Barrier => self.run_head_knife_load(gpu),
                step => progress = self.advance(gpu, progress, step, encoder),
            }
        }
    }

    fn advance(
        &mut self,
        gpu: &GpuContext,
        progress: WalkProgress,
        step: WalkStep,
        encoder: &mut wgpu::CommandEncoder,
    ) -> WalkProgress {
        self.take_step(gpu, step, encoder);
        progress.took(step)
    }

    fn submit_then_load_knife(&mut self, gpu: &GpuContext, encoder: &mut wgpu::CommandEncoder) {
        let encoded = std::mem::replace(encoder, new_encoder(gpu, "flush_queue"));
        gpu.queue.submit([encoded.finish()]);
        self.run_head_knife_load(gpu);
    }

    fn take_step(&mut self, gpu: &GpuContext, step: WalkStep, encoder: &mut wgpu::CommandEncoder) {
        match step {
            WalkStep::Begin => self.begin_head(),
            WalkStep::Dabs(count) => self.encode_head_dabs(gpu, count, encoder),
            WalkStep::End => self.end_head(),
            WalkStep::Command => self.encode_head_brush_change(gpu, encoder),
            WalkStep::Barrier | WalkStep::Wait => {}
        }
    }

    pub(super) fn begin_head(&mut self) {
        let Some(Queued::Stroke(stroke)) = self.queued.front_mut() else {
            return;
        };
        stroke.begun = true;
        self.history.begin_step();
        self.engine.begin_stroke();
        self.motion_anchor = None;
    }

    pub(super) fn end_head(&mut self) {
        self.queued.pop_front();
        self.engine.end_stroke();
        self.history.commit_step();
    }

    fn encode_head_brush_change(&mut self, gpu: &GpuContext, encoder: &mut wgpu::CommandEncoder) {
        if let Some(Queued::Brush(change)) = self.queued.pop_front() {
            self.encode_brush_change(gpu, change, encoder);
        }
    }

    fn run_head_knife_load(&mut self, gpu: &GpuContext) {
        if let Some(Queued::Knife(load)) = self.queued.pop_front() {
            let _ = self.run_knife_load(gpu, load);
        }
    }

    fn encode_head_dabs(
        &mut self,
        gpu: &GpuContext,
        count: usize,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let Some(Queued::Stroke(stroke)) = self.queued.front_mut() else {
            return;
        };
        let swept = stroke.take_dabs(count);
        let Some(latest) = swept.last() else {
            return;
        };
        self.motion_anchor = Some(MotionAnchor::at_latest_dab(latest.dab.time_ms));
        let batch: Vec<Dab> = swept.iter().map(|swept| swept.dab).collect();
        let touched = tiles_touched_by(&self.engine, &self.canvas, &stroke.brush, &swept);
        self.history
            .encode_save_tiles(gpu, &self.canvas, &touched, encoder);
        if stroke.brush.tool.profile().changes_held_paint() && self.history.awaits_held_paint() {
            let before = self.engine.encode_capture_held_paint(gpu, encoder);
            self.history.hold_before(before, self.engine.load_epoch());
        }
        let batch = DabBatch {
            brush: &stroke.brush,
            dabs: &batch,
        };
        self.engine
            .encode_dabs(gpu, &mut self.canvas, batch, encoder);
    }
}

fn tiles_touched_by(
    engine: &WetOilEngine,
    canvas: &Canvas,
    brush: &BrushSettings,
    dabs: &[SweptDab],
) -> Vec<TileCoord> {
    let width = canvas.spec.width;
    let height = canvas.spec.height;
    let touched: BTreeSet<TileCoord> = dabs
        .iter()
        .filter_map(|swept| engine.dab_bounds(canvas, brush, &swept.dab, swept.swept_from.as_ref()))
        .flat_map(|bounds| tiles_in_rect(width, height, bounds))
        .collect();
    touched.into_iter().collect()
}
