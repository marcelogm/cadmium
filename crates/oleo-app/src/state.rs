use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use oleo_engine::canvas::CanvasSpec;
use oleo_engine::gpu::GpuContext;
use oleo_engine::present::ViewTransform;
use oleo_engine::session::PaintSession;
use oleo_engine::stroke::{StrokeSample, Tool};

use crate::tools::UiTool;

const RATE_SMOOTHING: f32 = 0.2;
const MILLISECONDS_PER_SECOND: f32 = 1000.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PenReadout {
    pub pressure: f32,
    pub tilt_x: f32,
    pub tilt_y: f32,
    pub samples_last_frame: u32,
    pub rate_hz: f32,
}

impl PenReadout {
    pub fn record(&mut self, sample: &StrokeSample, previous_time_ms: Option<u32>) {
        self.pressure = sample.pressure;
        self.tilt_x = sample.tilt_x_deg;
        self.tilt_y = sample.tilt_y_deg;
        let Some(previous) = previous_time_ms else {
            return;
        };
        let interval_ms = sample.time_ms.wrapping_sub(previous);
        if interval_ms == 0 || interval_ms > u32::MAX / 2 {
            return;
        }
        let instant_rate = MILLISECONDS_PER_SECOND / interval_ms as f32;
        self.rate_hz = if self.rate_hz > 0.0 {
            self.rate_hz + (instant_rate - self.rate_hz) * RATE_SMOOTHING
        } else {
            instant_rate
        };
    }
}

pub type SharedState = Rc<RefCell<AppState>>;

pub struct AppState {
    pub gpu: Rc<GpuContext>,
    pub session: PaintSession,
    pub view: ViewTransform,
    pub ui_tool: UiTool,
    pub dirty_since_save: bool,
    pub file: Option<PathBuf>,
    pub pen: PenReadout,
    stroking: bool,
    tool_after_stroke: Option<Tool>,
    samples_since_frame: u32,
    last_sample_time_ms: Option<u32>,
}

impl AppState {
    pub fn new(gpu: Rc<GpuContext>, spec: CanvasSpec) -> AppState {
        let session = PaintSession::new(&gpu, spec);
        AppState {
            gpu,
            session,
            view: ViewTransform {
                origin_x: 0.0,
                origin_y: 0.0,
                zoom: 1.0,
            },
            ui_tool: UiTool::default(),
            dirty_since_save: false,
            file: None,
            pen: PenReadout::default(),
            stroking: false,
            tool_after_stroke: None,
            samples_since_frame: 0,
            last_sample_time_ms: None,
        }
    }

    pub fn start_painting(&mut self, spec: CanvasSpec) {
        self.end_stroke();
        let brush = self.session.brush.clone();
        self.session = PaintSession::new(&self.gpu, spec);
        self.session.brush = brush;
        self.session.reload_brush(&self.gpu);
        self.dirty_since_save = false;
        self.file = None;
    }

    pub fn set_ui_tool(&mut self, tool: UiTool) {
        self.ui_tool = tool;
        let UiTool::Paint(paint_tool) = tool else {
            return;
        };
        if self.tool_after_stroke.is_some() {
            self.tool_after_stroke = Some(paint_tool);
        } else {
            self.session.brush.tool = paint_tool;
        }
    }

    pub fn is_stroking(&self) -> bool {
        self.stroking
    }

    pub fn begin_stroke(&mut self, tool: Tool, sample: StrokeSample) {
        self.end_stroke();
        if self.session.brush.tool != tool {
            self.tool_after_stroke = Some(self.session.brush.tool);
            self.session.brush.tool = tool;
        }
        self.session.begin_stroke(&self.gpu, sample);
        self.stroking = true;
        self.last_sample_time_ms = None;
        self.note_samples(std::slice::from_ref(&sample));
    }

    pub fn add_samples(&mut self, samples: &[StrokeSample]) {
        if !self.stroking || samples.is_empty() {
            return;
        }
        self.session.add_samples(samples);
        self.note_samples(samples);
    }

    pub fn end_stroke(&mut self) {
        if !self.stroking {
            return;
        }
        self.session.end_stroke(&self.gpu);
        self.stroking = false;
        self.dirty_since_save = true;
    }

    pub fn encode_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.session.encode_frame(&self.gpu, encoder);
        if !self.stroking
            && let Some(tool) = self.tool_after_stroke.take()
        {
            self.session.brush.tool = tool;
        }
        self.pen.samples_last_frame = std::mem::take(&mut self.samples_since_frame);
    }

    fn note_samples(&mut self, samples: &[StrokeSample]) {
        for sample in samples {
            self.pen.record(sample, self.last_sample_time_ms);
            self.last_sample_time_ms = Some(sample.time_ms);
        }
        self.samples_since_frame += samples.len() as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_at(time_ms: u32) -> StrokeSample {
        StrokeSample {
            x: 0.0,
            y: 0.0,
            pressure: 0.5,
            tilt_x_deg: 10.0,
            tilt_y_deg: -20.0,
            time_ms,
        }
    }

    #[test]
    fn pen_rate_follows_the_sample_interval() {
        let mut readout = PenReadout::default();
        readout.record(&sample_at(0), None);
        readout.record(&sample_at(5), Some(0));
        assert!((readout.rate_hz - 200.0).abs() < 1e-3);
        assert_eq!(readout.tilt_y, -20.0);
    }

    #[test]
    fn samples_with_the_same_timestamp_leave_the_rate_alone() {
        let mut readout = PenReadout {
            rate_hz: 120.0,
            ..PenReadout::default()
        };
        readout.record(&sample_at(7), Some(7));
        assert_eq!(readout.rate_hz, 120.0);
    }

    #[test]
    fn clock_going_backwards_does_not_spike_the_rate() {
        let mut readout = PenReadout::default();
        readout.record(&sample_at(3), Some(10));
        assert_eq!(readout.rate_hz, 0.0);
    }
}
