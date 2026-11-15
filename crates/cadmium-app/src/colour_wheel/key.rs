//! What the wheel's picture is mixed from, so it is mixed again only when that changes.

use cadmium_engine::palette::{Palette, circle_axes};
use cadmium_engine::pigments::PigmentDb;

use super::image::{WheelImage, WheelPaint, wheel_image};

/// Everything the picture of the wheel is made from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WheelKey {
    palette: Palette,
    slots: Vec<String>,
    darken_bits: u32,
}

impl WheelKey {
    pub(crate) fn of(palette: &Palette, slots: &[String], darken: f32) -> WheelKey {
        WheelKey {
            palette: palette.clone(),
            slots: slots.to_vec(),
            darken_bits: darken.to_bits(),
        }
    }

    pub(super) fn image(&self, db: &PigmentDb) -> WheelImage {
        wheel_image(&WheelPaint {
            db,
            slots: &self.slots,
            palette: &self.palette,
            axes: circle_axes(&self.slots),
            darken: f32::from_bits(self.darken_bits),
        })
    }
}

/// Whether the wheel must be mixed again: only while it shows, and only when what it was last
/// mixed from is not what it would be mixed from now.
pub(super) fn wheel_is_stale(shows: bool, drawn: Option<&WheelKey>, wanted: &WheelKey) -> bool {
    shows && drawn != Some(wanted)
}

#[cfg(test)]
mod tests;
