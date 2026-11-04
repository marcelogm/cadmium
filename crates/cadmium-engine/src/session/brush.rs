use crate::engine::{BrushOutline, BrushReadout};
use crate::gpu::{GpuContext, GpuLost};
use crate::pigments::Mixture;
use crate::stroke::StrokeSample;

use super::PaintSession;

impl PaintSession {
    pub fn brush_outline(&self, pen: &StrokeSample) -> Option<BrushOutline> {
        self.engine.brush_outline(&self.brush, pen)
    }

    pub fn resize_tip(&mut self) {
        self.engine
            .resize_tip(self.brush.geometry.max_diameter_px / 2.0);
    }

    pub fn reload_brush(&mut self, gpu: &GpuContext) {
        self.flush(gpu);
        self.resize_tip();
        let mixture = self.recipe_mixture();
        if mixture.is_empty() {
            self.engine.clean_brush(gpu);
        } else {
            self.engine.load_reservoir(gpu, &mixture);
        }
    }

    pub fn dip(&mut self, gpu: &GpuContext, mixture: &Mixture, amount: f32) -> Result<(), GpuLost> {
        self.flush(gpu);
        if self.holds_knife() {
            self.resize_tip();
            self.engine.dip_knife(gpu, mixture, amount)
        } else {
            self.engine.dip(gpu, mixture, amount);
            Ok(())
        }
    }

    pub fn restore_reserve(&mut self, gpu: &GpuContext) {
        self.flush(gpu);
        let mixture = self.recipe_mixture();
        if !mixture.is_empty() {
            self.engine.restore_reserve(gpu, &mixture);
        }
    }

    pub fn tip_mean_mixture(&mut self, gpu: &GpuContext) -> Result<Mixture, GpuLost> {
        self.flush(gpu);
        self.engine.tip_mean_mixture(gpu)
    }

    pub fn read_brush(&mut self, gpu: &GpuContext) -> Result<BrushReadout, GpuLost> {
        self.flush(gpu);
        self.engine.read_brush(gpu)
    }

    pub fn clean_brush(&mut self, gpu: &GpuContext) {
        self.flush(gpu);
        if self.holds_knife() {
            self.engine.clean_knife(gpu);
        } else {
            self.engine.clean_brush(gpu);
        }
    }

    pub(super) fn holds_knife(&self) -> bool {
        self.brush.tool.profile().holds_on_blade()
    }

    fn recipe_mixture(&self) -> Mixture {
        self.brush
            .reservoir_mixture(self.canvas.spec.pigment_slots.len())
    }
}
