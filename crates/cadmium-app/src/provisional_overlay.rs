use gtk::prelude::*;
use gtk::{gdk, graphene, gsk};

use crate::state::Mark;

const CHANNEL_MAX: f32 = 255.0;

/// Paints `marks`, in widget px, as one union of discs in `colour`, so overlapping marks
/// do not build up opacity.
pub(crate) fn append_provisional(
    snapshot: &gtk::Snapshot,
    colour: [u8; 3],
    opacity: f32,
    marks: &[Mark],
) {
    if marks.is_empty() || opacity <= 0.0 {
        return;
    }
    let path = gsk::PathBuilder::new();
    for mark in marks {
        path.add_circle(
            &graphene::Point::new(mark.centre.0, mark.centre.1),
            mark.radius,
        );
    }
    let [red, green, blue] = colour.map(|channel| f32::from(channel) / CHANNEL_MAX);
    let rgba = gdk::RGBA::new(red, green, blue, opacity);
    snapshot.append_fill(&path.to_path(), gsk::FillRule::Winding, &rgba);
}
