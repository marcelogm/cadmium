//! What a studio canvas is made of and what its brush is loaded with.

use cadmium_engine::canvas::{Primer, Weave};
use cadmium_engine::params::SimParams;

/// What brushes are shown in when the paint at hand would show nothing: the rack's
/// thumbnails always, the editor's examples when the painting's brush is empty or white.
pub const DEMO_PIGMENT: &str = "ultramarine_blue";

/// What the studio canvas is made of and what the brush is loaded with.
#[derive(Clone, Debug, PartialEq)]
pub struct StudioPaint {
    pub slots: Vec<String>,
    pub primer: Primer,
    pub weave: Weave,
    pub recipe: Vec<(usize, f32)>,
    pub load: f32,
    pub medium: f32,
    pub params: SimParams,
}
