use crate::canvas::{CanvasSpec, PaintingRect, Primer, Weave};

pub const SHEET_WIDTH_PX: u32 = 768;
pub const SHEET_HEIGHT_PX: u32 = 512;
/// The whole mixing sheet, which has no margin.
pub const SHEET_RECT: PaintingRect = PaintingRect {
    x: 0,
    y: 0,
    width: SHEET_WIDTH_PX,
    height: SHEET_HEIGHT_PX,
};
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
