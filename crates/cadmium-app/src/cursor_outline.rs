use cadmium_engine::engine::BrushOutline;
use cadmium_engine::stroke::StrokeSample;
use gtk::prelude::*;
use gtk::{gdk, graphene, gsk};

use crate::state::AppState;
use crate::tools::UiTool;

const OUTLINE_WIDTH: f32 = 1.0;
const DARK_RIM_WIDTHS: f32 = 3.0;
const OUTLINE_MIN_RADIUS: f32 = 2.0;
const OUTLINE_LIGHT: gdk::RGBA = gdk::RGBA::new(1.0, 1.0, 1.0, 0.9);
const OUTLINE_DARK: gdk::RGBA = gdk::RGBA::new(0.0, 0.0, 0.0, 0.7);

enum OutlineShape {
    Ellipse {
        semi_axes: [f32; 2],
        rotation_deg: f32,
    },
    Polygon(Vec<(f32, f32)>),
}

pub(crate) struct CursorOutline {
    shape: OutlineShape,
}

impl CursorOutline {
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
        let shape = match state.session.engine.brush_outline(&brush, &pen) {
            Some(BrushOutline::Ellipse {
                semi_axes,
                orientation,
            }) => OutlineShape::Ellipse {
                semi_axes: semi_axes.map(|semi| semi * to_widget),
                rotation_deg: orientation.to_degrees(),
            },
            Some(BrushOutline::Bristles(points)) => OutlineShape::Polygon(
                points
                    .iter()
                    .map(|[x, y]| (x * to_widget, y * to_widget))
                    .collect(),
            ),
            None => OutlineShape::Ellipse {
                semi_axes: [canvas_radius * to_widget; 2],
                rotation_deg: 0.0,
            },
        };
        CursorOutline { shape }
    }

    pub(crate) fn radius(&self) -> f32 {
        match &self.shape {
            OutlineShape::Ellipse { semi_axes, .. } => semi_axes[0].max(semi_axes[1]),
            OutlineShape::Polygon(points) => {
                points.iter().map(|(x, y)| x.hypot(*y)).fold(0.0, f32::max)
            }
        }
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.radius() >= OUTLINE_MIN_RADIUS
    }

    pub(crate) fn append_to(&self, snapshot: &gtk::Snapshot, centre: (f32, f32)) {
        match &self.shape {
            OutlineShape::Ellipse {
                semi_axes,
                rotation_deg,
            } => append_ellipse(snapshot, centre, *semi_axes, *rotation_deg),
            OutlineShape::Polygon(points) => append_polygon(snapshot, centre, points),
        }
    }
}

fn append_ellipse(
    snapshot: &gtk::Snapshot,
    centre: (f32, f32),
    semi_axes: [f32; 2],
    rotation_deg: f32,
) {
    let [along, across] = semi_axes;
    append_ring(
        snapshot,
        &RingStroke {
            centre,
            semi_axes: [along + OUTLINE_WIDTH, across + OUTLINE_WIDTH],
            rotation_deg,
            colour: OUTLINE_DARK,
            width: OUTLINE_WIDTH,
        },
    );
    append_ring(
        snapshot,
        &RingStroke {
            centre,
            semi_axes,
            rotation_deg,
            colour: OUTLINE_LIGHT,
            width: OUTLINE_WIDTH,
        },
    );
}

fn append_polygon(snapshot: &gtk::Snapshot, (x, y): (f32, f32), points: &[(f32, f32)]) {
    let Some(((first_x, first_y), rest)) = points.split_first() else {
        return;
    };
    let path = gsk::PathBuilder::new();
    path.move_to(x + first_x, y + first_y);
    for (point_x, point_y) in rest {
        path.line_to(x + point_x, y + point_y);
    }
    path.close();
    let path = path.to_path();
    snapshot.append_stroke(
        &path,
        &gsk::Stroke::new(DARK_RIM_WIDTHS * OUTLINE_WIDTH),
        &OUTLINE_DARK,
    );
    snapshot.append_stroke(&path, &gsk::Stroke::new(OUTLINE_WIDTH), &OUTLINE_LIGHT);
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
