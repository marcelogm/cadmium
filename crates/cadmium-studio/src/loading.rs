//! How the studio puts a brush in hand, freshly loaded.

use cadmium_engine::brush::BrushSpec;
use cadmium_engine::gpu::GpuContext;
use cadmium_engine::session::PaintSession;

/// The paint a studio brush is loaded with.
pub(crate) struct Loading<'a> {
    pub recipe: &'a [(usize, f32)],
    pub load: f32,
    pub medium: f32,
}

/// Holds `brush` as it is, freshly loaded with `loading`.
pub(crate) fn wear(
    session: &mut PaintSession,
    gpu: &GpuContext,
    brush: &BrushSpec,
    loading: Loading<'_>,
) {
    let settings = &mut session.brush;
    settings.wear(brush);
    settings.recipe = loading.recipe.to_vec();
    settings.load = loading.load;
    settings.medium = loading.medium;
    session.reload_brush(gpu);
}
