use crate::engine::PaintEngine;
use crate::gpu::GpuContext;
use crate::surface::Surface;

use super::{PaintSession, surface_canvas};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MotionAnchor {
    stroke_time_ms: f32,
    frame_clock_ms: Option<f64>,
}

impl MotionAnchor {
    pub(super) fn at_latest_dab(stroke_time_ms: f32) -> MotionAnchor {
        MotionAnchor {
            stroke_time_ms,
            frame_clock_ms: None,
        }
    }

    fn bind_to(&mut self, frame_clock_ms: f64) {
        self.frame_clock_ms.get_or_insert(frame_clock_ms);
    }

    fn stroke_time_at(&self, frame_clock_ms: f64) -> f32 {
        let since_anchor_ms = self
            .frame_clock_ms
            .map_or(0.0, |anchored_ms| (frame_clock_ms - anchored_ms).max(0.0));
        (f64::from(self.stroke_time_ms) + since_anchor_ms) as f32
    }
}

impl PaintSession {
    pub fn bristles_settling(&self, frame_clock_ms: f64) -> bool {
        self.bristles_settling_on(Surface::Painting, frame_clock_ms)
    }

    pub fn tray_bristles_settling(&self, frame_clock_ms: f64) -> bool {
        self.bristles_settling_on(Surface::Tray, frame_clock_ms)
    }

    fn bristles_settling_on(&self, surface: Surface, frame_clock_ms: f64) -> bool {
        self.walked_surface == surface
            && self
                .stroke_time_at(frame_clock_ms)
                .is_some_and(|stroke_time_ms| self.engine.bristles_settling(stroke_time_ms))
    }

    fn stroke_time_at(&self, frame_clock_ms: f64) -> Option<f32> {
        self.motion_anchor
            .map(|anchor| anchor.stroke_time_at(frame_clock_ms))
    }

    pub(super) fn encode_dabs_or_settle(
        &mut self,
        gpu: &GpuContext,
        surface: Surface,
        frame_clock_ms: f64,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let work_queued = self.has_queued_dabs();
        self.last_frame_dabs = self.walk_frame(gpu, encoder);
        let Some(anchor) = self.motion_anchor.as_mut() else {
            return;
        };
        anchor.bind_to(frame_clock_ms);
        let stroke_time_ms = anchor.stroke_time_at(frame_clock_ms);
        if work_queued || self.walked_surface != surface {
            return;
        }
        if self.engine.bristles_settling(stroke_time_ms) {
            let canvas = surface_canvas(surface, &self.canvas, &self.tray);
            self.engine
                .settle_bristles(gpu, canvas, stroke_time_ms, encoder);
        }
    }
}
