mod new_canvas;
mod slots;
mod spec_validation;
mod textures;
mod tiles;

use cadmium_engine::canvas::{CanvasSpec, Primer, Weave};

fn spec(primer: Primer, weave: Weave) -> CanvasSpec {
    CanvasSpec {
        width: 256,
        height: 128,
        primer,
        weave,
        pigment_slots: vec!["titanium_white".into(), "ultramarine_blue".into()],
    }
}
