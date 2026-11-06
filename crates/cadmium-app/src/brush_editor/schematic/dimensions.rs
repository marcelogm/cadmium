//! The schematic's dimensions in blue: length L and tip width W on the front view, the fan's
//! angle, thickness t on the side view.

use std::f64::consts::FRAC_PI_2;

use cadmium_studio::TipGeometry;

use super::canvas::{Canvas, Part, hull};
use super::pen::{self, DIMENSION, DIMENSION_LABEL, Dimension, Pen};
use super::sheet::{FRONT_X, MIN_TIP_WIDTH, SIDE_X, Sheet};
use crate::ink::Fit;

const ARC_PEN: Pen = Pen::new(DIMENSION, 0.8);
const RAY_PEN: Pen = Pen::new(DIMENSION, 0.5);
/// The ferrule's mouth, in root half-widths.
pub(super) const MOUTH: f64 = 1.035;
const LENGTH_GAP: f64 = 14.0;
const BELOW_TIP: f64 = 16.0;
const EXTENSION_GAP: f64 = 2.0;
/// The length's lower extension starts this far right of the axis.
const AXIS_GAP: f64 = 4.0;
const MIN_FAN_DEG: f32 = 0.5;
const FAN_ARC_SHARE: f64 = 0.5;
const MAX_FAN_ARC: f64 = 64.0;
const FAN_RAY_OVERSHOOT: f64 = 6.0;
const FAN_LABEL_BELOW: f64 = 12.0;
pub(super) const SHORT_DASH: [f64; 2] = [3.0, 2.0];
pub(super) const THICKNESS_SLACK: f64 = 1.06;
const THIN_HEAD: f32 = 0.2;
const THIN_EXTRA: f64 = 1.5;
const ROOT_GAP: f64 = 4.0;

pub(super) fn draw(canvas: &Canvas, tip: &TipGeometry, sheet: &Sheet) {
    length(canvas, tip, sheet);
    tip_width(canvas, tip, sheet);
    fan(canvas, tip, sheet);
    thickness(canvas, tip, sheet);
}

fn length(canvas: &Canvas, tip: &TipGeometry, sheet: &Sheet) {
    let scale = sheet.scale;
    let top = sheet.roots_y;
    let bottom = top + f64::from(tip.dimensions.length) * scale;
    let x = FRONT_X + sheet.half_width * scale + LENGTH_GAP;
    let label = format!("L {:.2}R", tip.dimensions.length);
    pen::vertical_dimension(
        canvas,
        &Dimension {
            from: [x, top],
            to: [x, bottom],
            label: &label,
            extension_from: [FRONT_X + MOUTH * scale + EXTENSION_GAP, FRONT_X + AXIS_GAP],
        },
    );
}

/// W, under the front view's deepest point (the mockup put it under the tip's corners, which
/// ran it through a domed tip).
fn tip_width(canvas: &Canvas, tip: &TipGeometry, sheet: &Sheet) {
    let edge = tip.front.tip_edge();
    let (Some(&left), Some(&right)) = (edge.first(), edge.last()) else {
        return;
    };
    if tip.dimensions.tip_width <= MIN_TIP_WIDTH {
        return;
    }
    let fit = Fit {
        scale: sheet.scale,
        origin: [FRONT_X, sheet.roots_y],
    };
    let [(x0, y0), (x1, y1)] = [left, right].map(|corner| fit.place(corner));
    let label = tip_width_label(tip);
    let y = sheet.roots_y + sheet.front_depth * sheet.scale + BELOW_TIP;
    pen::horizontal_dimension(
        canvas,
        &Dimension {
            from: [x0, y],
            to: [x1, y],
            label: &label,
            extension_from: [y0 + EXTENSION_GAP, y1 + EXTENSION_GAP],
        },
    );
}

/// W's figure.
pub(super) fn tip_width_label(tip: &TipGeometry) -> String {
    format!("W {:.2}R", tip.dimensions.tip_width)
}

fn fan(canvas: &Canvas, tip: &TipGeometry, sheet: &Sheet) {
    let fan_deg = tip.dimensions.fan_deg;
    if fan_deg <= MIN_FAN_DEG {
        return;
    }
    let [x, y] = [FRONT_X, sheet.roots_y];
    let radius = (f64::from(tip.dimensions.length) * sheet.scale * FAN_ARC_SHARE).min(MAX_FAN_ARC);
    let half = f64::from(fan_deg).to_radians() / 2.0;
    let ends = [FRAC_PI_2 + half, FRAC_PI_2 - half];
    canvas.part(Part::FanAngle, || {
        pen::ink(canvas, ARC_PEN);
        canvas.new_sub_path();
        canvas.arc_negative(x, y, radius, ends[0], ends[1]);
        canvas.covers(|| {
            canvas
                .stroke_extents()
                .map_or([[x, y]; 2], |(x0, y0, x1, y1)| [[x0, y0], [x1, y1]])
        });
        let _ = canvas.stroke();
        let reach = radius + FAN_RAY_OVERSHOOT;
        let rays = ends.map(|angle| [x + angle.cos() * reach, y + angle.sin() * reach]);
        for end in rays {
            pen::dashed(canvas, [x, y], end, &SHORT_DASH, RAY_PEN);
        }
        canvas.covers(|| hull(&[[x, y], rays[0], rays[1]]));
        let at = [x, y + radius + FAN_LABEL_BELOW];
        pen::write(canvas, at, &format!("{fan_deg:.0}°"), &DIMENSION_LABEL);
    });
}

fn thickness(canvas: &Canvas, tip: &TipGeometry, sheet: &Sheet) {
    let thickness = tip.dimensions.thickness;
    let extra = if thickness < THIN_HEAD {
        THIN_EXTRA
    } else {
        0.0
    };
    let half = f64::from(thickness) * THICKNESS_SLACK * sheet.scale + extra;
    let y = sheet.roots_y + f64::from(tip.dimensions.length) * sheet.scale + BELOW_TIP;
    let from = sheet.roots_y + ROOT_GAP;
    let label = format!("t {thickness:.2}");
    pen::horizontal_dimension(
        canvas,
        &Dimension {
            from: [SIDE_X - half, y],
            to: [SIDE_X + half, y],
            label: &label,
            extension_from: [from, from],
        },
    );
}
