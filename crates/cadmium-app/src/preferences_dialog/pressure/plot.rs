//! Where the unit square of the curve sits in the editor widget.

/// Widget px kept clear around the plot, so the end points' handles show whole.
const PADDING: f64 = 12.0;

/// The square the curve is drawn in: input rightward, output upward.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Plot {
    left: f64,
    top: f64,
    side: f64,
}

impl Plot {
    /// The largest square centred in a `width` × `height` widget, inset by [`PADDING`].
    pub(super) fn in_widget(width: f64, height: f64) -> Plot {
        let side = (width.min(height) - 2.0 * PADDING).max(1.0);
        Plot {
            left: (width - side) / 2.0,
            top: (height - side) / 2.0,
            side,
        }
    }

    /// The widget point of unit point (`x`, `y`).
    pub(super) fn to_widget(self, (x, y): (f32, f32)) -> (f64, f64) {
        (
            self.left + f64::from(x) * self.side,
            self.top + (1.0 - f64::from(y)) * self.side,
        )
    }

    /// The unit point under widget point (`x`, `y`), held to the square's edges.
    pub(super) fn to_unit(self, x: f64, y: f64) -> (f32, f32) {
        let along = ((x - self.left) / self.side).clamp(0.0, 1.0);
        let up = (1.0 - (y - self.top) / self.side).clamp(0.0, 1.0);
        (along as f32, up as f32)
    }

    /// `pixels` widget px as a distance in the unit square.
    pub(super) fn unit_distance(self, pixels: f64) -> f32 {
        (pixels / self.side) as f32
    }

    /// The square as (left, top, side) in widget px, for drawing.
    pub(super) fn bounds(self) -> (f64, f64, f64) {
        (self.left, self.top, self.side)
    }
}

#[cfg(test)]
mod tests;
