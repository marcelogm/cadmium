use crate::canvas::{CanvasSpec, Primer, Weave};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Surface {
    Painting,
    Tray,
}

pub const TRAY_WIDTH: u32 = 1536;
pub const TRAY_HEIGHT: u32 = 768;
/// The tray size documents up to format 3 were saved with.
pub const LEGACY_TRAY_WIDTH: u32 = 768;
pub const LEGACY_TRAY_HEIGHT: u32 = 512;

pub fn tray_spec(pigment_slots: &[String]) -> CanvasSpec {
    sized_tray_spec(pigment_slots, TRAY_WIDTH, TRAY_HEIGHT)
}

pub fn legacy_tray_spec(pigment_slots: &[String]) -> CanvasSpec {
    sized_tray_spec(pigment_slots, LEGACY_TRAY_WIDTH, LEGACY_TRAY_HEIGHT)
}

fn sized_tray_spec(pigment_slots: &[String], width: u32, height: u32) -> CanvasSpec {
    CanvasSpec {
        width,
        height,
        primer: Primer::Tray,
        weave: Weave::Smooth,
        pigment_slots: pigment_slots.to_vec(),
    }
}
