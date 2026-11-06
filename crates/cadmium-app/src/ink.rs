//! Line drawings with Cairo: fitting a drawing into a widget and tracing its outlines.

use gtk::cairo;

mod fit;

pub(crate) use fit::{Fit, contain};

/// Traces `points` as one closed path: the tip's outlines are open polylines.
pub(crate) fn trace(context: &cairo::Context, fit: &Fit, points: &[[f32; 2]]) {
    context.new_path();
    for &point in points {
        let (x, y) = fit.place(point);
        context.line_to(x, y);
    }
    context.close_path();
}
