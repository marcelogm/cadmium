//! What the schematic is drawn on: a Cairo context and the display's pixel scale, so straight
//! lines land on whole device pixels; in tests, it also notes where each part landed.

#[cfg(test)]
use std::cell::RefCell;
use std::ops::Deref;

use gtk::cairo;

use crate::ink::Fit;

/// A box, `[min, max]`.
pub(super) type Bounds = [[f64; 2]; 2];

/// What a noted box holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Part {
    /// A view of the tip or the footprint, with its ferrule and handle.
    View,
    TitleBlock,
    /// A view's or the footprint's caption.
    Caption,
    /// A dimension's line, arrows and figure (not its extension lines).
    Dimension,
    /// The fan's arc, rays and figure, drawn on the front view.
    FanAngle,
}

/// Where a part landed, in the widget's units.
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) struct Mark {
    pub(super) part: Part,
    pub(super) bounds: Bounds,
}

pub(super) struct Canvas<'a> {
    context: &'a cairo::Context,
    /// Device pixels per widget unit.
    pixel_scale: f64,
    #[cfg(test)]
    pub(super) marks: RefCell<Vec<Mark>>,
}

impl Deref for Canvas<'_> {
    type Target = cairo::Context;

    fn deref(&self) -> &cairo::Context {
        self.context
    }
}

impl<'a> Canvas<'a> {
    pub(super) fn new(context: &'a cairo::Context, pixel_scale: f64) -> Canvas<'a> {
        Canvas {
            context,
            pixel_scale,
            #[cfg(test)]
            marks: RefCell::default(),
        }
    }

    /// `point` moved by under half a device pixel so that a straight line `width` wide through
    /// it, across or down, covers whole device pixels (`Fit::snapped`'s rule).
    pub(super) fn snapped(&self, [x, y]: [f64; 2], width: f64) -> [f64; 2] {
        let (device_x, device_y) = self.user_to_device(x, y);
        let (across, down) = self
            .user_to_device_distance(width, 0.0)
            .unwrap_or((width, 0.0));
        let fit = Fit {
            scale: 1.0,
            origin: [device_x, device_y],
        };
        let [snapped_x, snapped_y] = fit.snapped(self.pixel_scale, across.hypot(down)).origin;
        self.device_to_user(snapped_x, snapped_y)
            .map_or([x, y], |(x, y)| [x, y])
    }

    /// The box in the widget that `bounds`, in the current user space, covers.
    pub(super) fn on_widget(&self, [min, max]: Bounds) -> Bounds {
        let corners = [min, [max[0], min[1]], max, [min[0], max[1]]];
        let placed = corners.map(|[x, y]| {
            let (x, y) = self.user_to_device(x, y);
            [x, y]
        });
        hull(&placed)
    }

    /// Notes that `part` covers `bounds`, a box in the widget (tests only).
    pub(super) fn mark(&self, part: Part, bounds: Bounds) {
        #[cfg(test)]
        self.marks.borrow_mut().push(Mark { part, bounds });
        #[cfg(not(test))]
        let _ = (part, bounds);
    }
}

/// The smallest box holding `points`.
pub(super) fn hull(points: &[[f64; 2]]) -> Bounds {
    let start = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
    points.iter().fold(start, |[min, max], &[x, y]| {
        [
            [min[0].min(x), min[1].min(y)],
            [max[0].max(x), max[1].max(y)],
        ]
    })
}

/// The smallest box holding both.
pub(super) fn union([a_min, a_max]: Bounds, [b_min, b_max]: Bounds) -> Bounds {
    hull(&[a_min, a_max, b_min, b_max])
}
