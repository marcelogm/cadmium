use std::collections::BTreeSet;

use crate::brush::BrushSettings;
use crate::canvas::{Canvas, TileCoord, tiles_in_rect};
use crate::engine::{DabBatch, PaintEngine, WetOilEngine};
use crate::gpu::{GpuContext, new_encoder};
use crate::stroke::Dab;

use super::queue::{Queued, WalkProgress, WalkStep, next_step};
use super::{MotionAnchor, PaintSession, surface_canvas};

const SAVE_MARGIN_PX: f32 = 4.0;

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
        self.history.begin_step(stroke.surface);
        self.walked_surface = stroke.surface;
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
        let batch: Vec<Dab> = stroke.dabs.drain(..count).collect();
        let Some(latest) = batch.last() else {
            return;
        };
        self.motion_anchor = Some(MotionAnchor::at_latest_dab(latest.time_ms));
        let canvas = surface_canvas(stroke.surface, &mut self.canvas, &mut self.tray);
        let touched = tiles_touched_by(&self.engine, canvas, &stroke.brush, &batch);
        self.history
            .encode_save_tiles(gpu, canvas, &touched, encoder);
        let batch = DabBatch {
            brush: &stroke.brush,
            dabs: &batch,
        };
        self.engine.encode_dabs(gpu, canvas, batch, encoder);
    }
}

fn tiles_touched_by(
    engine: &WetOilEngine,
    canvas: &Canvas,
    brush: &BrushSettings,
    dabs: &[Dab],
) -> Vec<TileCoord> {
    let width = canvas.spec.width;
    let height = canvas.spec.height;
    let touched: BTreeSet<TileCoord> = dabs
        .iter()
        .filter_map(|dab| engine.dab_bounds(canvas, brush, dab))
        .flat_map(|[x0, y0, x1, y1]| {
            tiles_in_rect(
                width,
                height,
                [
                    x0 - SAVE_MARGIN_PX,
                    y0 - SAVE_MARGIN_PX,
                    x1 + SAVE_MARGIN_PX,
                    y1 + SAVE_MARGIN_PX,
                ],
            )
        })
        .collect();
    touched.into_iter().collect()
}
