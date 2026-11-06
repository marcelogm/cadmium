//! Fitting a drawing into a widget: one uniform scale, centred.

/// Where a drawing lands in a widget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Fit {
    pub(super) scale: f64,
    /// Where the drawing's (0, 0) lands.
    pub(super) origin: [f64; 2],
}

impl Fit {
    pub(super) fn place(&self, [x, y]: [f32; 2]) -> (f64, f64) {
        (
            self.origin[0] + f64::from(x) * self.scale,
            self.origin[1] + f64::from(y) * self.scale,
        )
    }

    /// The same fit with its origin moved under half a device pixel, so a line of
    /// `line_width` through the drawing's x = 0 or y = 0 covers whole device pixels on a
    /// display of `pixel_scale` device pixels per unit: on a pixel's centre when the line is
    /// an odd number of pixels wide, on its edge when even.
    pub(super) fn snapped(self, pixel_scale: f64, line_width: f64) -> Fit {
        let offset = (line_width * pixel_scale / 2.0).fract();
        let snap = |at: f64| ((at * pixel_scale - offset).round() + offset) / pixel_scale;
        Fit {
            scale: self.scale,
            origin: self.origin.map(snap),
        }
    }
}

/// Fits the box from `min` to `max` into a `frame`, `padding` in from every edge, as large
/// as it goes undistorted, centred; an empty box gets a huge but finite scale.
pub(super) fn contain(min: [f64; 2], max: [f64; 2], frame: [f64; 2], padding: f64) -> Fit {
    let content = [0, 1].map(|axis| (max[axis] - min[axis]).max(f64::EPSILON));
    let room = frame.map(|side| (side - 2.0 * padding).max(0.0));
    let scale = (room[0] / content[0]).min(room[1] / content[1]);
    let origin = [0, 1].map(|axis| (frame[axis] - content[axis] * scale) / 2.0 - min[axis] * scale);
    Fit { scale, origin }
}

#[cfg(test)]
mod tests;
