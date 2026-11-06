use cadmium_engine::stroke::StrokeSample;
use gtk::prelude::*;
use gtk::{gdk, graphene, gsk};

use crate::state::AppState;
use crate::tools::UiTool;

const OUTLINE_WIDTH: f32 = 1.0;
const OUTLINE_MIN_RADIUS: f32 = 2.0;
const OUTLINE_LIGHT: gdk::RGBA = gdk::RGBA::new(1.0, 1.0, 1.0, 0.9);
const OUTLINE_DARK: gdk::RGBA = gdk::RGBA::new(0.0, 0.0, 0.0, 0.7);

pub(crate) struct CursorOutline {
    pub semi_axes: [f32; 2],
    pub rotation_deg: f32,
}

impl CursorOutline {
    /// The outline as the screen shows it when the picture is turned clockwise by `rotation`.
    pub(crate) fn turned(self, rotation: f32) -> CursorOutline {
        CursorOutline {
            rotation_deg: self.rotation_deg + rotation.to_degrees(),
            ..self
        }
    }

    pub(crate) fn of_brush(state: &AppState, to_widget: f32) -> CursorOutline {
        let mut brush = state.session.brush.clone();
        if let UiTool::Paint(tool) = state.ui_tool {
            brush.tool = tool;
        }
        let pen = StrokeSample {
            x: 0.0,
            y: 0.0,
            pressure: state.pen.pressure,
            tilt_x_deg: state.pen.tilt_x,
            tilt_y_deg: state.pen.tilt_y,
            time_ms: 0,
        };
        let canvas_radius = brush.geometry.max_diameter_px / 2.0;
        let (semi_axes, rotation_deg) = match state.session.engine.brush_outline(&brush, &pen) {
            Some(outline) => (outline.semi_axes, outline.orientation.to_degrees()),
            None => ([canvas_radius; 2], 0.0),
        };
        CursorOutline {
            semi_axes: semi_axes.map(|semi| semi * to_widget),
            rotation_deg,
        }
    }

    pub(crate) fn radius(&self) -> f32 {
        self.semi_axes[0].max(self.semi_axes[1])
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.radius() >= OUTLINE_MIN_RADIUS
    }

    /// The outline's two rings: a dark one just outside a light one, so it shows on any paint.
    pub(crate) fn rings(&self, centre: (f32, f32)) -> [RingStroke; 2] {
        let [along, across] = self.semi_axes;
        let ring = |semi_axes, colour| RingStroke {
            centre,
            semi_axes,
            rotation_deg: self.rotation_deg,
            colour,
            width: OUTLINE_WIDTH,
        };
        [
            ring(
                [along + OUTLINE_WIDTH, across + OUTLINE_WIDTH],
                OUTLINE_DARK,
            ),
            ring(self.semi_axes, OUTLINE_LIGHT),
        ]
    }

    pub(crate) fn append_to(&self, snapshot: &gtk::Snapshot, centre: (f32, f32)) {
        for ring in self.rings(centre) {
            append_ring(snapshot, &ring);
        }
    }
}

pub(crate) struct RingStroke {
    pub(crate) centre: (f32, f32),
    pub(crate) semi_axes: [f32; 2],
    pub(crate) rotation_deg: f32,
    pub(crate) colour: gdk::RGBA,
    pub(crate) width: f32,
}

pub(crate) fn append_ring(snapshot: &gtk::Snapshot, ring: &RingStroke) {
    let (x, y) = ring.centre;
    let [along, across] = ring.semi_axes;
    snapshot.save();
    snapshot.translate(&graphene::Point::new(x, y));
    snapshot.rotate(ring.rotation_deg);
    let bounds = graphene::Rect::new(-along, -across, 2.0 * along, 2.0 * across);
    let corner = graphene::Size::new(along, across);
    let path = gsk::PathBuilder::new();
    path.add_rounded_rect(&gsk::RoundedRect::new(
        bounds, corner, corner, corner, corner,
    ));
    snapshot.append_stroke(&path.to_path(), &gsk::Stroke::new(ring.width), &ring.colour);
    snapshot.restore();
}

#[cfg(test)]
mod tests;
