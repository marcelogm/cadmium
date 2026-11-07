//! The engine's tips: the painting brush's, and one of the tray's own for its blender and
//! rag, so working paint on the tray never soils the brush. The tip a stroke or a brush
//! change works on is the active one; the other waits, parked. Switching swaps the two
//! sets of handles, so no paint is copied.

use super::bristles::BristleRig;
use super::{Buffers, WetOilEngine};
use crate::gpu::GpuContext;

/// Which tip the engine works with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tip {
    /// The painting tool's brush or knife.
    #[default]
    Painting,
    /// The tray's own blender and rag.
    Tray,
}

/// The tip that isn't active: its buffers, its bristles and how full it is.
pub(super) struct ParkedTip {
    buffers: Buffers,
    bristles: BristleRig,
    loaded_thickness_um: f32,
}

impl WetOilEngine {
    /// Gives the engine the tray's own tip, clean, if it hasn't one yet. Without it every
    /// stroke works the painting tip, as headless sessions do.
    pub fn add_tray_tip(&mut self, gpu: &GpuContext) {
        if self.parked.is_none() {
            self.parked = Some(Box::new(ParkedTip {
                buffers: Buffers::new(gpu),
                bristles: BristleRig::new(),
                loaded_thickness_um: 0.0,
            }));
        }
    }

    pub fn active_tip(&self) -> Tip {
        self.active
    }

    /// Makes `tip` the one strokes and brush changes work on.
    pub fn select_tip(&mut self, tip: Tip) {
        if tip == self.active {
            return;
        }
        let Some(parked) = self.parked.as_deref_mut() else {
            return;
        };
        std::mem::swap(&mut self.buffers, &mut parked.buffers);
        std::mem::swap(&mut self.bristles, &mut parked.bristles);
        std::mem::swap(
            &mut self.loaded_thickness_um,
            &mut parked.loaded_thickness_um,
        );
        self.active = tip;
    }
}
