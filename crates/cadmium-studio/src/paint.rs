//! What a studio canvas is made of and what its brush is loaded with.

use cadmium_engine::canvas::{Primer, Weave};
use cadmium_engine::params::SimParams;

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
