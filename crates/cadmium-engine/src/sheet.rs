use crate::canvas::{CanvasSpec, Primer, Weave};

pub const SHEET_WIDTH_PX: u32 = 768;
pub const SHEET_HEIGHT_PX: u32 = 512;
pub const SHEET_HELD_PAINT_BUDGET_BYTES: u64 = 32 * 1024 * 1024;

pub fn sheet_spec(pigment_slots: &[String]) -> CanvasSpec {
    CanvasSpec {
        width: SHEET_WIDTH_PX,
        height: SHEET_HEIGHT_PX,
        primer: Primer::Paper,
        weave: Weave::Smooth,
        pigment_slots: pigment_slots.to_vec(),
        margin_px: 0,
    }
}
