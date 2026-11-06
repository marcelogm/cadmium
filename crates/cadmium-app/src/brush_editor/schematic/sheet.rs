//! Where the schematic puts each part, in sheet units.

use cadmium_studio::TipGeometry;

/// The sheet: 320 × 560 sheet units, and 22 more for the title block's name row.
pub(super) const SHEET: [f64; 2] = [320.0, 582.0];
/// Nothing is drawn closer than this to the sheet's edge but the paper, grid and frame.
pub(super) const SHEET_MARGIN: f64 = 10.0;
pub(super) const FRONT_X: f64 = 96.0;
pub(super) const SIDE_X: f64 = 246.0;
pub(super) const FOOTPRINT_X: f64 = 160.0;
pub(super) const TITLE_TOP: f64 = 488.0;
/// Sheet units per root half-width, at most.
const MAX_SCALE: f64 = 46.0;
/// The height the views share, and the ferrule-and-handle part of it, in root half-widths.
const VIEW_HEIGHT: f64 = 300.0;
const ABOVE_ROOTS: f64 = 2.6;
/// The width rule: 118 units for the front view, fitted to at least 1.15 half-widths.
const VIEW_WIDTH: f64 = 118.0;
const MIN_FITTED_HALF_WIDTH: f64 = 1.15;
const WIDTH_SLACK: f64 = 1.1;
/// Room left of the front view's axis and right of the side view's, so extreme fans and
/// bellies stay on the sheet.
const LEFT_ROOM: f64 = FRONT_X - SHEET_MARGIN;
const RIGHT_ROOM: f64 = SHEET[0] - SHEET_MARGIN - SIDE_X;
/// Room kept between the front view and the side view, for the length's dimension, so a
/// wide fan beside a full belly stays apart.
pub(super) const BETWEEN_VIEWS: f64 = 32.0;
/// Where the handle is cut, and the ferrule (1.55) plus handle (0.9) below it, in half-widths.
const HANDLE_TOP: f64 = 40.0;
const FERRULE_AND_HANDLE: f64 = 2.45;
const FOOTPRINT_LOWEST: f64 = 430.0;
const FOOTPRINT_GAP: f64 = 92.0;
const FOOTPRINT_MAX_SCALE: f64 = 52.0;
const FOOTPRINT_ROOM: f64 = 128.0;
const MIN_FOOTPRINT_REACH: f64 = 1.1;
/// The footprint's caption sits this far above its centre line, more for a thick footprint.
const CAPTION_GAP: f64 = 16.0;
const MIN_CAPTION_RISE: f64 = 28.0;
/// W is drawn only for a tip wider than this, in root half-widths.
pub(super) const MIN_TIP_WIDTH: f32 = 0.12;
/// How far below the deepest point the lowest figure (W's, t's) reaches: its line 16 down,
/// its baseline 11 more, and room for the glyphs' descent.
pub(super) const FIGURES_BELOW: f64 = 30.0;
/// The footprint's caption's height above its baseline, with a little room.
pub(super) const CAPTION_HEIGHT: f64 = 8.0;
/// Room kept round the footprint's caption and W's figure, across.
const FIGURE_ROOM: f64 = 4.0;

/// How wide the figures the layout steers round are, as the pen sets them.
#[derive(Clone, Copy, Debug)]
pub(super) struct Figures {
    /// The footprint's caption.
    pub(super) footprint_caption: f64,
    /// W's figure.
    pub(super) tip_width: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Sheet {
    /// Sheet units per root half-width in the front and side views.
    pub(super) scale: f64,
    /// Where the hair roots sit in both views.
    pub(super) roots_y: f64,
    /// The front view's widest reach either side of its axis, in root half-widths.
    pub(super) half_width: f64,
    /// The front view's deepest point below the roots, in root half-widths.
    pub(super) front_depth: f64,
    /// The footprint's centre line, its own scale, and its caption's baseline.
    pub(super) footprint_y: f64,
    pub(super) footprint_scale: f64,
    pub(super) footprint_caption_y: f64,
}

impl Sheet {
    pub(super) fn of(tip: &TipGeometry, figures: &Figures) -> Sheet {
        let front_half = reach_x(tip.front.extent());
        let side_half = reach_x(tip.side.extent());
        let front_depth = f64::from(tip.front.bounds()[1][1]);
        let deepest =
            f64::from(tip.side.bounds()[1][1].max(tip.dimensions.length)).max(front_depth);
        let scale = MAX_SCALE
            .min(VIEW_HEIGHT / (deepest + ABOVE_ROOTS))
            .min(VIEW_WIDTH / MIN_FITTED_HALF_WIDTH.max(front_half * WIDTH_SLACK))
            .min(LEFT_ROOM / front_half)
            .min(RIGHT_ROOM / side_half)
            .min((SIDE_X - FRONT_X - BETWEEN_VIEWS) / (front_half + side_half));
        let roots_y = HANDLE_TOP + FERRULE_AND_HANDLE * scale;
        let bottom = roots_y + deepest * scale;
        let clear = width_figure_under_caption(tip, scale, figures)
            .then_some(bottom + FIGURES_BELOW + CAPTION_HEIGHT);
        let [footprint_y, footprint_scale, footprint_caption_y] =
            footprint_layout(tip, bottom, clear);
        Sheet {
            scale,
            roots_y,
            half_width: front_half,
            front_depth,
            footprint_y,
            footprint_scale,
            footprint_caption_y,
        }
    }
}

/// The footprint's centre line, scale and caption's baseline, under views ending at `bottom`;
/// `clear`, when W's figure is in the caption's way, is the highest the caption's baseline
/// may sit. Lowered for it, the footprint stops above the title block; past that, it shrinks.
fn footprint_layout(tip: &TipGeometry, bottom: f64, clear: Option<f64>) -> [f64; 3] {
    let reach = reach_x(tip.footprint_bounds()).max(MIN_FOOTPRINT_REACH);
    let scale = FOOTPRINT_MAX_SCALE.min(FOOTPRINT_ROOM / reach);
    let thickness = f64::from(tip.dimensions.thickness);
    let rise = MIN_CAPTION_RISE.max(thickness * scale + CAPTION_GAP);
    let Some(clear) = clear else {
        let y = (bottom + FOOTPRINT_GAP).min(FOOTPRINT_LOWEST);
        return [y, scale, y - rise];
    };
    let y = (bottom + FOOTPRINT_GAP)
        .max(clear + rise)
        .min(FOOTPRINT_LOWEST);
    if y - rise >= clear {
        return [y, scale, y - rise];
    }
    // The views end high enough that the caption always has room over its gap.
    debug_assert!(y - clear > CAPTION_GAP, "{y} {clear}");
    let shrunk = scale.min((y - clear - CAPTION_GAP).max(0.0) / thickness);
    let shrunk_rise = MIN_CAPTION_RISE.max(thickness * shrunk + CAPTION_GAP);
    [y, shrunk, y - shrunk_rise]
}

/// Whether W's dimension, at `scale`, reaches under the footprint's caption, where a wide
/// tip's figure would run into it.
fn width_figure_under_caption(tip: &TipGeometry, scale: f64, figures: &Figures) -> bool {
    let edge = tip.front.tip_edge();
    let (Some(left), Some(right)) = (edge.first(), edge.last()) else {
        return false;
    };
    if tip.dimensions.tip_width <= MIN_TIP_WIDTH {
        return false;
    }
    let [left, right] = [left[0], right[0]].map(f64::from);
    let label_reach = (left + right) / 2.0 * scale + figures.tip_width / 2.0;
    let reach = (right * scale).max(label_reach) + FIGURE_ROOM;
    FRONT_X + reach > FOOTPRINT_X - figures.footprint_caption / 2.0 - FIGURE_ROOM
}

/// The widest reach either side of x = 0 of a box `[min, max]`.
fn reach_x([min, max]: [[f32; 2]; 2]) -> f64 {
    f64::from(min[0].abs().max(max[0]))
}

#[cfg(test)]
mod tests;
