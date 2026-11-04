use crate::canvas::{CanvasSpec, Primer, Weave};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Surface {
    Painting,
    Tray,
}

pub const TRAY_WIDTH: u32 = 1536;
pub const TRAY_HEIGHT: u32 = 1024;

pub fn tray_spec(pigment_slots: &[String]) -> CanvasSpec {
    CanvasSpec {
        width: TRAY_WIDTH,
        height: TRAY_HEIGHT,
        primer: Primer::Tray,
        weave: Weave::Smooth,
        pigment_slots: pigment_slots.to_vec(),
    }
}
