//! The Brush group: the brush's name and its suggested size.

use adw::prelude::*;
use cadmium_engine::brush::BrushSpec;

const SIZE_STEP_PX: f64 = 1.0;
/// The size row shows whole pixels.
const SIZE_DIGITS: u32 = 0;

pub(super) struct BrushGroup {
    pub(super) group: adw::PreferencesGroup,
    pub(super) name: adw::EntryRow,
    pub(super) size: adw::SpinRow,
}

pub(super) fn brush_group() -> BrushGroup {
    let name = adw::EntryRow::builder().title("Name").build();
    let [low, high] = BrushSpec::SIZE_RANGE_PX.map(f64::from);
    let size = adw::SpinRow::with_range(low, high, SIZE_STEP_PX);
    size.set_title("Suggested Size");
    size.set_subtitle("Pixels, set when the brush is picked");
    size.set_digits(SIZE_DIGITS);
    let group = adw::PreferencesGroup::builder().title("Brush").build();
    group.add(&name);
    group.add(&size);
    BrushGroup { group, name, size }
}

impl BrushGroup {
    /// Shows `brush`'s name and size; the name only when it differs, so typing isn't disturbed.
    pub(super) fn show(&self, brush: &BrushSpec) {
        if self.name.text().as_str() != brush.name {
            self.name.set_text(&brush.name);
        }
        self.size.set_value(f64::from(brush.size_px));
    }
}

/// Whether the size row showing `shown` asks for a size other than `size_px`. The row shows
/// `size_px` rounded to its digits, and re-reads that rounded text when it is activated or
/// loses focus: that echo is no change, or a fractional size would turn the draft dirty.
pub(super) fn changes_size(shown: f64, size_px: f32) -> bool {
    let scale = 10_f64.powi(SIZE_DIGITS as i32);
    let rounded = (f64::from(size_px) * scale).round() / scale;
    shown != rounded
}

#[cfg(test)]
mod tests;
