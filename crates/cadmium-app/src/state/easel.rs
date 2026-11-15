use cadmium_engine::gpu::GpuContext;
use cadmium_engine::pigments::Mixture;
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::Tool;

use super::EMPTY_MIXTURE;
use crate::tools::UiTool;

/// Which easel a gesture, a load or an undo is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EaselId {
    Painting,
}

/// A canvas with the tool in hand over it: the session, the gesture running on it and what
/// the tool was last read to hold. It neither wakes views nor counts edits; `AppState` does.
pub struct Easel {
    pub session: PaintSession,
    pub ui_tool: UiTool,
    pub reload_each_stroke: bool,
    pub clean_each_stroke: bool,
    pub tip_mixture: Mixture,
    pub knife_mixture: Mixture,
    pub(super) reserve_volume: f32,
    pub(super) reserve_mixture: Mixture,
    pub(super) knife_volume: f32,
    pub(super) stroking: bool,
    /// The tool the session goes back to once a stroke ends.
    pub(super) resting_tool: Tool,
    pub(super) brush_measure_waits: bool,
    /// Set when a deferred brush read lands; the window drains it to refresh the panels.
    pub(super) brush_measured: bool,
}

impl Easel {
    pub fn new(session: PaintSession) -> Easel {
        let resting_tool = session.brush.tool;
        Easel {
            session,
            ui_tool: UiTool::default(),
            reload_each_stroke: false,
            clean_each_stroke: false,
            tip_mixture: EMPTY_MIXTURE,
            knife_mixture: EMPTY_MIXTURE,
            reserve_volume: 0.0,
            reserve_mixture: EMPTY_MIXTURE,
            knife_volume: 0.0,
            stroking: false,
            resting_tool,
            brush_measure_waits: false,
            brush_measured: false,
        }
    }

    pub fn set_ui_tool(&mut self, tool: UiTool) {
        self.ui_tool = tool;
        let UiTool::Paint(paint_tool) = tool else {
            return;
        };
        self.resting_tool = paint_tool;
        if !self.stroking {
            self.session.brush.tool = paint_tool;
        }
    }

    pub fn encode_frame(
        &mut self,
        gpu: &GpuContext,
        clock_ms: f64,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.brush_measure_due() {
            self.brush_measure_waits = false;
            self.brush_measured = self.measure_brush(gpu).is_some();
        }
        self.session.encode_frame(gpu, clock_ms, encoder);
        if !self.stroking {
            self.session.brush.tool = self.resting_tool;
        }
    }
}
