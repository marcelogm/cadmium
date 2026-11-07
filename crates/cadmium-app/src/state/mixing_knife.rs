use cadmium_engine::brush::FULL_RESERVOIR_VOLUME_UM;
use cadmium_engine::engine::BrushReadout;
use cadmium_engine::pigments::Mixture;
use cadmium_engine::stroke::Tool;
use cadmium_engine::surface::Surface;

use super::AppState;
use super::dip::{fill_level, mean_mixture};
use super::recipe::recipe_from_fractions;
use crate::tools::panel::PanelTool;

pub(super) const MIXING_KNIFE_DIAMETER_PX: f32 = 80.0;

/// The tray's mixing knife: a blade of its own size, whose face keeps the paint it
/// scrapes up from one pass to the next.
#[derive(Clone, Copy, Debug)]
pub(super) struct MixingKnife {
    diameter_px: f32,
    /// The brush's own size while the mixing knife's stands in for it in a stroke.
    brush_diameter_px: Option<f32>,
    face: Mixture,
}

impl Default for MixingKnife {
    fn default() -> Self {
        MixingKnife {
            diameter_px: MIXING_KNIFE_DIAMETER_PX,
            brush_diameter_px: None,
            face: super::EMPTY_MIXTURE,
        }
    }
}

/// Whether `tool` may stroke `surface`: the mixing knife works on the tray only.
pub(super) fn strokes_on(tool: Tool, surface: Surface) -> bool {
    surface == Surface::Tray || !tool.profile().holds_on_face()
}

impl AppState {
    pub fn mixing_knife_diameter_px(&self) -> f32 {
        self.mixing_knife.diameter_px
    }

    fn mixing_knife_is_focused(&self) -> bool {
        self.focused_tool() == PanelTool::Paint(Tool::MixingKnife)
    }

    /// The size of the tool the side panel describes.
    pub fn focused_size(&self) -> f32 {
        if self.mixing_knife_is_focused() {
            self.mixing_knife.diameter_px
        } else {
            self.session.brush.geometry.max_diameter_px
        }
    }

    /// Sizes the tool the side panel describes.
    pub fn set_focused_size(&mut self, diameter_px: f32) {
        if !self.mixing_knife_is_focused() {
            self.set_brush_size(diameter_px);
            return;
        }
        self.release();
        if diameter_px.is_finite() && diameter_px > 0.0 {
            self.mixing_knife.diameter_px = diameter_px;
        }
    }

    /// Strokes with the mixing knife's size, keeping the brush's to hand it back.
    pub(super) fn take_up_mixing_knife_size(&mut self, tool: Tool) {
        if !tool.profile().holds_on_face() || self.mixing_knife.brush_diameter_px.is_some() {
            return;
        }
        let geometry = &mut self.session.brush.geometry;
        self.mixing_knife.brush_diameter_px = Some(geometry.max_diameter_px);
        geometry.max_diameter_px = self.mixing_knife.diameter_px;
        self.session.resize_tip();
    }

    pub(super) fn hand_back_brush_size(&mut self) {
        if let Some(diameter_px) = self.mixing_knife.brush_diameter_px.take() {
            self.session.brush.geometry.max_diameter_px = diameter_px;
            self.session.resize_tip();
        }
    }

    /// The diameter `tool` strokes at.
    pub(crate) fn stroke_diameter_px(&self, tool: Tool) -> f32 {
        if tool.profile().holds_on_face() {
            self.mixing_knife.diameter_px
        } else {
            self.mixing_knife
                .brush_diameter_px
                .unwrap_or(self.session.brush.geometry.max_diameter_px)
        }
    }

    pub fn mixing_knife_mixture(&self) -> Mixture {
        self.mixing_knife.face
    }

    /// The pigments on the face, in their shares.
    pub fn mixing_knife_recipe(&self) -> Vec<(usize, f32)> {
        if self.mixing_knife_is_clean() {
            return Vec::new();
        }
        recipe_from_fractions(&self.mixing_knife.face.fractions)
    }

    pub fn mixing_knife_is_clean(&self) -> bool {
        self.mixing_knife.face.is_empty()
    }

    /// How full the face is, against what a full dip lays on a blade its size.
    pub fn mixing_knife_level(&self) -> f32 {
        let capacity = self
            .session
            .engine
            .blade_full_dip_volume(FULL_RESERVOIR_VOLUME_UM, self.mixing_knife.diameter_px);
        fill_level(self.mixing_knife.face.volume_um, capacity)
    }

    pub(super) fn note_face(&mut self, readout: &BrushReadout) {
        self.mixing_knife.face = mean_mixture(&readout.face);
    }

    /// Cleans the mixing knife's face, leaving the brush in hand as it is.
    pub(super) fn clean_mixing_knife(&mut self) {
        self.session.brush.tool = Tool::MixingKnife;
        self.session.clean_brush(&self.gpu);
        self.session.brush.tool = self.resting_tool;
    }

    /// Cleans what the side panel describes: the mixing knife's face, or the brush.
    pub(super) fn clean_focused(&mut self) {
        if self.mixing_knife_is_focused() {
            self.clean_mixing_knife();
        } else {
            self.session.clean_brush(&self.gpu);
        }
    }
}

#[cfg(test)]
mod tests;
