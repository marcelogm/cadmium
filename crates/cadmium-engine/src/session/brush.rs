use crate::engine::{BrushOutline, BrushReadout, Tip};
use crate::gpu::{GpuContext, GpuLost};
use crate::pigments::Mixture;
use crate::stroke::StrokeSample;

use super::PaintSession;
use super::queue::{BrushChange, KnifeLoad, Queued};

impl PaintSession {
    pub fn brush_outline(&self, pen: &StrokeSample) -> Option<BrushOutline> {
        self.engine.brush_outline(&self.brush, pen)
    }

    pub fn resize_tip(&mut self) {
        let radius_px = self.brush.geometry.max_diameter_px / 2.0;
        if self.has_backlog() {
            self.queued
                .push_back(Queued::Brush(BrushChange::ResizeTip { radius_px }));
        } else {
            self.engine.resize_tip(radius_px);
        }
    }

    pub fn reload_brush(&mut self, gpu: &GpuContext) {
        self.resize_tip();
        let mixture = self.recipe_mixture();
        self.change_brush(gpu, BrushChange::Reload { mixture });
    }

    pub fn dip(&mut self, gpu: &GpuContext, mixture: &Mixture, amount: f32) -> Result<(), GpuLost> {
        let mixture = *mixture;
        if self.holds_knife() {
            self.resize_tip();
            self.load_knife(gpu, KnifeLoad::Dip { mixture, amount })
        } else {
            self.change_brush(gpu, BrushChange::Dip { mixture, amount });
            Ok(())
        }
    }

    pub fn restore_reserve(&mut self, gpu: &GpuContext) {
        let mixture = self.recipe_mixture();
        if !mixture.is_empty() {
            self.change_brush(gpu, BrushChange::RestoreReserve { mixture });
        }
    }

    /// Dips the knife in the palette's mixture for just what it lacks of a full dip.
    pub fn top_up_knife(&mut self, gpu: &GpuContext) -> Result<(), GpuLost> {
        let mixture = self.recipe_mixture();
        if mixture.is_empty() {
            return Ok(());
        }
        self.resize_tip();
        self.load_knife(gpu, KnifeLoad::TopUp { mixture })
    }

    pub fn tip_mean_mixture(&mut self, gpu: &GpuContext) -> Result<Mixture, GpuLost> {
        self.flush(gpu);
        self.on_tip(Tip::Painting, |session| {
            session.engine.tip_mean_mixture(gpu)
        })
    }

    pub fn read_brush(&mut self, gpu: &GpuContext) -> Result<BrushReadout, GpuLost> {
        self.read_tip(gpu, Tip::Painting)
    }

    /// What `tip` holds: the painting brush's, or the tray's own tools'.
    pub fn read_tip(&mut self, gpu: &GpuContext, tip: Tip) -> Result<BrushReadout, GpuLost> {
        self.flush(gpu);
        self.on_tip(tip, |session| session.engine.read_brush(gpu))
    }

    pub fn clean_tray_tip(&mut self, gpu: &GpuContext) {
        self.change_brush(gpu, BrushChange::CleanTrayTip);
    }

    pub fn clean_brush(&mut self, gpu: &GpuContext) {
        let knife = self.holds_knife();
        self.change_brush(gpu, BrushChange::Clean { knife });
    }

    pub(super) fn holds_knife(&self) -> bool {
        self.brush.tool.profile().holds_on_blade()
    }

    fn recipe_mixture(&self) -> Mixture {
        self.brush
            .reservoir_mixture(self.canvas.spec.pigment_slots.len())
    }
}
