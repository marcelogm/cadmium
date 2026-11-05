use std::collections::BTreeSet;

use crate::canvas::{TileCoord, tiles_in_rect};
use crate::engine::{DabBatch, PaintEngine};
use crate::gpu::{GpuContext, new_encoder};
use crate::stroke::{StrokeBuilder, StrokeSample};
use crate::surface::Surface;

use super::{MotionAnchor, PaintSession, surface_canvas};

const SAVE_MARGIN_PX: f32 = 4.0;

impl PaintSession {
    pub fn begin_stroke(&mut self, gpu: &GpuContext, sample: StrokeSample) {
        self.begin_stroke_on(gpu, Surface::Painting, sample);
    }

    pub fn begin_stroke_on(&mut self, gpu: &GpuContext, surface: Surface, sample: StrokeSample) {
        self.settle_pending_step(gpu);
        self.history.begin_step(surface);
        self.stroke_surface = surface;
        self.step_open = true;
        self.stroke_brush = self.brush.clone();
        self.engine.begin_stroke();
        self.motion_anchor = None;
        let mut builder = StrokeBuilder::new(self.stroke_brush.stroke_geometry());
        self.queued.extend(builder.push(sample));
        self.stroke = Some(builder);
    }

    pub fn add_samples(&mut self, samples: &[StrokeSample]) {
        let Some(builder) = self.stroke.as_mut() else {
            return;
        };
        for sample in samples {
            self.queued.extend(builder.push(*sample));
        }
    }

    pub fn end_stroke(&mut self) {
        let Some(mut builder) = self.stroke.take() else {
            return;
        };
        self.queued.extend(builder.finish());
        if self.queued.is_empty() {
            self.commit_open_step();
        }
    }

    pub fn has_pending_work(&self) -> bool {
        self.has_pending_work_on(Surface::Painting)
    }

    pub fn tray_has_pending_work(&self) -> bool {
        self.has_pending_work_on(Surface::Tray)
    }

    fn has_pending_work_on(&self, surface: Surface) -> bool {
        let stroke_pending = !self.queued.is_empty() || self.step_awaits_commit();
        (self.stroke_surface == surface && stroke_pending)
            || !self.canvas_of(surface).dirty.is_empty()
    }

    fn step_awaits_commit(&self) -> bool {
        self.step_open && self.stroke.is_none()
    }

    pub fn flush(&mut self, gpu: &GpuContext) {
        if self.queued.is_empty() && !self.step_awaits_commit() {
            return;
        }
        let mut encoder = new_encoder(gpu, "flush_queued_dabs");
        self.encode_queued_dabs(gpu, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
    }

    pub(super) fn settle_pending_step(&mut self, gpu: &GpuContext) {
        self.end_hold();
        self.end_stroke();
        self.flush(gpu);
    }

    pub(super) fn encode_queued_dabs(
        &mut self,
        gpu: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if let Some(latest) = self.queued.last() {
            self.motion_anchor = Some(MotionAnchor::at_latest_dab(latest.time_ms));
            let touched = self.tiles_touched_by_queued_dabs();
            let canvas = surface_canvas(self.stroke_surface, &mut self.canvas, &mut self.tray);
            self.history
                .encode_save_tiles(gpu, canvas, &touched, encoder);
            let batch = DabBatch {
                brush: &self.stroke_brush,
                dabs: &self.queued,
            };
            self.engine.encode_dabs(gpu, canvas, batch, encoder);
            self.queued.clear();
        }
        if self.stroke.is_none() {
            self.engine.end_stroke();
            self.commit_open_step();
        }
    }

    fn commit_open_step(&mut self) {
        if self.step_open {
            self.history.commit_step();
            self.step_open = false;
        }
    }

    fn tiles_touched_by_queued_dabs(&self) -> Vec<TileCoord> {
        let canvas = self.canvas_of(self.stroke_surface);
        let width = canvas.spec.width;
        let height = canvas.spec.height;
        let touched: BTreeSet<TileCoord> = self
            .queued
            .iter()
            .filter_map(|dab| self.engine.dab_bounds(canvas, &self.stroke_brush, dab))
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
}
