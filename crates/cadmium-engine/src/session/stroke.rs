use std::collections::VecDeque;

use crate::gpu::GpuContext;
use crate::stroke::{Dab, StrokeBuilder, StrokeSample};
use crate::surface::Surface;

use super::PaintSession;
use super::queue::{Queued, QueuedStroke};

impl PaintSession {
    pub fn begin_stroke(&mut self, sample: StrokeSample) {
        self.begin_stroke_on(Surface::Painting, sample);
    }

    /// Queues a stroke behind whatever is still waiting to paint; the engine crosses into
    /// it only once the strokes and brush changes ahead of it are encoded.
    pub fn begin_stroke_on(&mut self, surface: Surface, sample: StrokeSample) {
        self.end_hold();
        self.end_stroke();
        let brush = self.brush.clone();
        let mut builder = StrokeBuilder::new(brush.stroke_geometry());
        let mut stroke = QueuedStroke::new(surface, brush);
        stroke.dabs.extend(builder.push(sample));
        let first_in_line = self.queued.is_empty();
        self.queued.push_back(Queued::Stroke(stroke));
        self.stroke = Some(builder);
        if first_in_line {
            self.begin_head();
        }
    }

    pub fn add_samples(&mut self, samples: &[StrokeSample]) {
        let Some(builder) = self.stroke.as_mut() else {
            return;
        };
        let Some(stroke) = drawn_stroke(&mut self.queued) else {
            return;
        };
        for sample in samples {
            stroke.dabs.extend(builder.push(*sample));
        }
    }

    pub fn end_stroke(&mut self) {
        let Some(mut builder) = self.stroke.take() else {
            return;
        };
        let Some(stroke) = drawn_stroke(&mut self.queued) else {
            return;
        };
        stroke.dabs.extend(builder.finish());
        stroke.ended = true;
        if self.queued.front().is_some_and(Queued::is_painted_out) {
            self.end_head();
        }
    }

    /// Caps how many queued dabs one frame paints; the rest wait for the next frames, in
    /// order, so a frame's GPU work stays bounded while the painting comes out the same.
    /// `None` paints every queued dab each frame.
    pub fn limit_dabs_per_frame(&mut self, limit: Option<usize>) {
        self.frame_dab_limit = limit.map(|limit| limit.max(1));
    }

    pub fn last_frame_dabs(&self) -> usize {
        self.last_frame_dabs
    }

    /// The dabs still to paint on `surface`, stroke after stroke, in painting order.
    pub fn queued_dabs_on(&self, surface: Surface) -> Vec<Dab> {
        self.queued
            .iter()
            .filter_map(|entry| match entry {
                Queued::Stroke(stroke) if stroke.surface == surface => Some(&stroke.dabs),
                _ => None,
            })
            .flatten()
            .copied()
            .collect()
    }

    /// Whether dabs or brush changes still wait for frames to reach the GPU.
    pub fn has_queued_dabs(&self) -> bool {
        self.queued.iter().any(Queued::holds_work)
    }

    pub fn has_pending_work(&self) -> bool {
        self.has_pending_work_on(Surface::Painting)
    }

    pub fn tray_has_pending_work(&self) -> bool {
        self.has_pending_work_on(Surface::Tray)
    }

    fn has_pending_work_on(&self, surface: Surface) -> bool {
        self.queued
            .iter()
            .any(|entry| entry.awaits_frames_on(surface))
            || !self.canvas_of(surface).dirty.is_empty()
            || !self.canvas_of(surface).possibly_dirty.is_empty()
    }

    pub(super) fn settle_pending_step(&mut self, gpu: &GpuContext) {
        self.end_hold();
        self.end_stroke();
        self.flush(gpu);
    }
}

fn drawn_stroke(queued: &mut VecDeque<Queued>) -> Option<&mut QueuedStroke> {
    queued.iter_mut().rev().find_map(|entry| match entry {
        Queued::Stroke(stroke) if !stroke.ended => Some(stroke),
        _ => None,
    })
}
