use crate::canvas::PaintTotals;
use crate::engine::{BrushOutline, BrushReadout, Tip};
use crate::gpu::{GpuContext, GpuLost};
use crate::pigments::{MAX_SLOTS, Mixture};
use crate::stroke::StrokeSample;
use crate::tool::Holder;

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
        if self.brush.tool.profile().holds_on_face() {
            // The mixing knife's face loads only by scraping paint up off the tray.
            return Ok(());
        }
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
    /// A session without the tray's tip (a headless one) reads it as empty.
    pub fn read_tip(&mut self, gpu: &GpuContext, tip: Tip) -> Result<BrushReadout, GpuLost> {
        if tip == Tip::Tray && !self.engine.has_tray_tip() {
            return Ok(BrushReadout {
                tip_mean: Mixture {
                    volume_um: 0.0,
                    fractions: [0.0; MAX_SLOTS],
                },
                reserve: PaintTotals::default(),
                knife: PaintTotals::default(),
                face: PaintTotals::default(),
            });
        }
        self.flush(gpu);
        self.on_tip(tip, |session| session.engine.read_brush(gpu))
    }

    /// Does nothing without the tray's tip, so it never cleans the painting brush instead.
    pub fn clean_tray_tip(&mut self, gpu: &GpuContext) {
        if !self.engine.has_tray_tip() {
            return;
        }
        self.change_brush(gpu, BrushChange::CleanTrayTip);
    }

    pub fn clean_brush(&mut self, gpu: &GpuContext) {
        self.clean_holder(gpu, self.brush.tool.profile().holder);
    }

    /// Cleans what `holder` carries, whatever tool is in hand.
    pub fn clean_holder(&mut self, gpu: &GpuContext, holder: Option<Holder>) {
        self.change_brush(gpu, BrushChange::Clean { holder });
    }

    pub(super) fn holds_knife(&self) -> bool {
        self.brush.tool.profile().holds_on_blade()
    }

    fn recipe_mixture(&self) -> Mixture {
        self.brush
            .reservoir_mixture(self.canvas.spec.pigment_slots.len())
    }
}
