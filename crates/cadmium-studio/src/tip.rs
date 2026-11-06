//! The tip of a brush as a technical drawing: hair bundle, ferrule and dimensions.
//!
//! Every coordinate is in root half-widths: the hair bundle's half-width at its roots (what
//! the engine calls the ferrule radius; the drawn ferrule's mouth is 1.035 of it). All views
//! share one frame: x runs across the head (positive to the right) and y runs down from the
//! ferrule's bottom edge, so the hair roots sit at y = 0, the hairs below (y > 0) and the
//! ferrule above (y < 0). The side view uses the same frame with x along the thickness; the
//! footprint (the tip seen from below) puts x across and y along the thickness, centred on
//! the axis.
//!
//! Outlines are open polylines: the first point is not repeated at the end and no point is
//! repeated at a corner, so the caller closes each path (Cairo's `close_path`).
//!
//! It is a drawing, not the engine's hair layout. The maths follows the approved schematic:
//! each axis draws to a point as far as the hair's wet point lets it, the belly swells the
//! bundle, the dome shortens the outer hairs, the slant tilts the tip, and a fan turns each
//! hair by its share of the angle (±45° at a 90° fan). As in the engine, a hair never gets
//! shorter than a fifth of the head's length.

use std::f32::consts::{PI, TAU};

use cadmium_engine::brush::{HairSpec, HeadSpec};

/// Segments down each side and across the tip of an outline.
const OUTLINE_STEPS: usize = 28;
/// Segments along each guide line.
const HAIR_STEPS: usize = 14;
/// Guide lines stop just short of the tip, so they never touch the outline.
const HAIR_REACH: f32 = 0.985;
/// Gaps between guide lines across the front view (one line fewer than gaps).
const FRONT_HAIR_GAPS: usize = 13;
/// The fewest gaps across the side view, however thin the head.
const MIN_SIDE_HAIR_GAPS: usize = 3;
/// Points round the footprint.
const FOOTPRINT_STEPS: usize = 48;
/// How far a fully pointed bundle draws in at its tip, as a share of its root width.
const POINT_PULL: f32 = 0.92;
/// How late along the hair the point draws in: higher keeps the bundle full for longer.
const POINT_EXPONENT: f32 = 2.4;
/// Share of the point a bone-dry hair still makes; the wet point adds the rest.
const DRY_POINT_SHARE: f32 = 0.35;
/// Thickness at which a head starts drawing to a point across its width as well.
const ROUND_POINT_ONSET: f32 = 0.5;
/// Fan angle at which the hairs no longer gather to a point. The engine fades the point out
/// by 10°; the drawing keeps it to 45° on purpose (the owner-approved schematic).
const FAN_OPEN_DEG: f32 = 45.0;
/// How much a full belly swells the bundle at mid-length, as a share of its root width.
const BELLY_SWELL: f32 = 0.5;
/// The dome's cap: no deeper than the tip is wide, nor than a share of the length.
const DOME_CAP_PER_REACH: f32 = 1.1;
const DOME_CAP_PER_LENGTH: f32 = 0.45;
/// How far a full slant lengthens the outermost hair, per half-width of the tip.
const SLANT_PER_REACH: f32 = 0.75;
/// The shortest a hair gets, as a share of the head's length (the engine's floor too).
const MIN_LENGTH_SHARE: f32 = 0.2;
/// The ferrule's mouth, a touch wider than the hair bundle.
const FERRULE_MOUTH: f32 = 1.035;
/// The ferrule's height, in root half-widths.
const FERRULE_HEIGHT: f32 = 1.55;
const FERRULE_STEPS: usize = 40;
/// The crimp, as shares of the ferrule's height from its mouth, and how deep it bites.
const CRIMP: [f32; 2] = [0.33, 0.43];
const CRIMP_DEPTH: f32 = 0.024;
/// Where the ferrule's neck starts turning round, and over what share of its height.
const NECK_START: f32 = 0.45;
const NECK_SPAN: f32 = 0.4;
/// The handle's radius: a base plus a share of the head's thickness, within a range.
const HANDLE_RADIUS_BASE: f32 = 0.55;
const HANDLE_RADIUS_PER_THICKNESS: f32 = 0.35;
const HANDLE_RADIUS_RANGE: [f32; 2] = [0.42, 0.88];
const HANDLE_HEIGHT: f32 = 0.9;
const HANDLE_FLARE: f32 = 1.04;

/// A brush tip drawn three ways, with its named dimensions.
#[derive(Clone, Debug, PartialEq)]
pub struct TipGeometry {
    /// Seen face on, across the head's width.
    pub front: TipView,
    /// Seen edge on, across the head's thickness.
    pub side: TipView,
    /// The tip seen from below: x across, y along the thickness.
    /// Counter-clockwise in x-right, y-up axes (clockwise on a y-down screen), open.
    pub footprint: Vec<[f32; 2]>,
    /// The dimensions the schematic labels.
    pub dimensions: Dimensions,
}

/// One view of the tip.
#[derive(Clone, Debug, PartialEq)]
pub struct TipView {
    /// The hair bundle, open: down the left edge from root to tip, across the tip from left
    /// to right, back up the right edge (counter-clockwise on a y-down screen); the root
    /// line closes it.
    pub outline: Vec<[f32; 2]>,
    /// Guide lines along the hairs, root to just short of the tip, left to right.
    pub hairs: Vec<Vec<[f32; 2]>>,
    /// The ferrule and handle above the roots, as seen in this view.
    pub ferrule: Ferrule,
}

/// The ferrule's profile in one view: the head's section at its mouth (y = 0), round at
/// its top (y = -1.55, in root half-widths), then the handle.
#[derive(Clone, Debug, PartialEq)]
pub struct Ferrule {
    /// Open: up the left side from the mouth, down the right side (clockwise on a y-down
    /// screen); the mouth line closes it.
    pub outline: Vec<[f32; 2]>,
    /// Half-width of the mouth.
    pub mouth: f32,
    /// The y of the crimp's two lines; each spans ±`mouth`.
    pub crimp: [f32; 2],
    /// The handle's right edge, from the ferrule's top up to where the drawing cuts it;
    /// the left edge mirrors it.
    pub handle: [[f32; 2]; 2],
}

/// The schematic's named dimensions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dimensions {
    /// L: the centre hair's length, in root half-widths.
    pub length: f32,
    /// W: the drawn tip's width (the span of the front view's tip edge), in root half-widths.
    pub tip_width: f32,
    /// t: the head's thickness, as a share of its width.
    pub thickness: f32,
    /// The fan's total angle, in degrees (0 to 90): the outermost hairs turn by half of it.
    pub fan_deg: f32,
    /// The slant, -1 to 1: positive lengthens the hairs on the right, negative on the left.
    pub slant: f32,
}

impl TipGeometry {
    /// Draws `head` with `hair`. Both are clamped first, so every coordinate is finite.
    pub fn new(head: &HeadSpec, hair: &HairSpec) -> TipGeometry {
        let head = head.clamped();
        let shape = Shape::new(&head, &hair.clamped());
        let front_at = |u: f32, t: f32| shape.front(u, t);
        let side_at = |w: f32, t: f32| shape.side(w, t);
        let side_gaps = (FRONT_HAIR_GAPS as f32 * head.thickness).round() as usize;
        let front = TipView {
            outline: outline(front_at),
            hairs: hairs(front_at, FRONT_HAIR_GAPS),
            ferrule: ferrule(1.0, head.thickness),
        };
        let [tip_min, tip_max] = bounds_of(front.tip_edge().iter());
        TipGeometry {
            side: TipView {
                outline: outline(side_at),
                hairs: hairs(side_at, side_gaps.max(MIN_SIDE_HAIR_GAPS)),
                ferrule: ferrule(head.thickness, head.thickness),
            },
            front,
            footprint: footprint(&shape),
            dimensions: Dimensions {
                length: head.length,
                tip_width: tip_max[0] - tip_min[0],
                thickness: head.thickness,
                fan_deg: head.fan_deg,
                slant: head.slant,
            },
        }
    }

    /// The smallest box holding the footprint: `[min, max]`.
    pub fn footprint_bounds(&self) -> [[f32; 2]; 2] {
        bounds_of(self.footprint.iter())
    }
}

impl TipView {
    /// The outline's tip edge, left to right, its first and last points the corners.
    ///
    /// Relies on the outline's layout as `TipGeometry` builds it; an outline of another
    /// length gives an empty edge.
    pub fn tip_edge(&self) -> &[[f32; 2]] {
        self.outline
            .get(OUTLINE_STEPS..=2 * OUTLINE_STEPS)
            .filter(|_| self.outline.len() == 3 * OUTLINE_STEPS + 1)
            .unwrap_or(&[])
    }

    /// The smallest box holding the hair bundle's outline and the ferrule's outline: not the
    /// handle, nor the guide lines (which lie inside the outline). `[min, max]`.
    pub fn bounds(&self) -> [[f32; 2]; 2] {
        bounds_of(self.outline.iter().chain(&self.ferrule.outline))
    }

    /// Like [`TipView::bounds`], and the handle's two edges as well: everything drawn.
    pub fn extent(&self) -> [[f32; 2]; 2] {
        let handle = self.ferrule.handle;
        let mirrored = handle.map(|[x, y]| [-x, y]);
        let [min, max] = self.bounds();
        bounds_of([min, max].iter().chain(&handle).chain(&mirrored))
    }
}

fn bounds_of<'a>(points: impl Iterator<Item = &'a [f32; 2]>) -> [[f32; 2]; 2] {
    let start = [[f32::INFINITY; 2], [f32::NEG_INFINITY; 2]];
    points.fold(start, |[min, max], &[x, y]| {
        [
            [min[0].min(x), min[1].min(y)],
            [max[0].max(x), max[1].max(y)],
        ]
    })
}

/// The head's shape, ready to place points: `u` runs across the head (-1 to 1) and `t`
/// along the hair, from its root (0) to its tip (1).
struct Shape {
    thickness: f32,
    length: f32,
    half_fan: f32,
    /// How far the bundle draws to a point across its width and across its thickness.
    point: [f32; 2],
    belly: f32,
    slant: f32,
    /// Half the tip's width.
    reach: f32,
    dome_cap: f32,
}

impl Shape {
    fn new(head: &HeadSpec, hair: &HairSpec) -> Shape {
        let round =
            ((head.thickness - ROUND_POINT_ONSET) / (1.0 - ROUND_POINT_ONSET)).clamp(0.0, 1.0);
        let open = 1.0 - (head.fan_deg / FAN_OPEN_DEG).min(1.0);
        let wet = head.point * (DRY_POINT_SHARE + (1.0 - DRY_POINT_SHARE) * hair.wet_point);
        let half_fan = head.fan_deg.to_radians() / 2.0;
        let mut shape = Shape {
            thickness: head.thickness,
            length: head.length,
            half_fan,
            point: [wet * round * open, wet * open],
            belly: head.belly,
            slant: head.slant,
            reach: 0.0,
            dome_cap: 0.0,
        };
        shape.reach = shape.taper(0, 1.0) + half_fan.sin() * head.length;
        let cap = (shape.reach * DOME_CAP_PER_REACH).min(head.length * DOME_CAP_PER_LENGTH);
        shape.dome_cap = cap * head.dome;
        shape
    }

    /// The bundle's width (axis 0) or thickness (axis 1) at `t`, as a share of the root's.
    fn taper(&self, axis: usize, t: f32) -> f32 {
        let swell = 1.0 + self.belly * BELLY_SWELL * (PI * t).sin();
        swell * (1.0 - self.point[axis] * POINT_PULL * t.powf(POINT_EXPONENT))
    }

    /// The hair at `u`: shortened towards the edges by the dome, tilted by the slant.
    fn hair_length(&self, u: f32) -> f32 {
        let dome = self.dome_cap * (1.0 - (1.0 - u * u).max(0.0).sqrt());
        let slant = self.slant * u * self.reach * SLANT_PER_REACH;
        (self.length - dome + slant).max(self.length * MIN_LENGTH_SHARE)
    }

    /// A point of the front view: each hair turned by its share of the fan.
    fn front(&self, u: f32, t: f32) -> [f32; 2] {
        let length = self.hair_length(u);
        let turn = u * self.half_fan;
        [
            u * self.taper(0, t) + turn.sin() * t * length,
            turn.cos() * t * length,
        ]
    }

    /// A point of the side view, `w` running across the thickness (-1 to 1).
    fn side(&self, w: f32, t: f32) -> [f32; 2] {
        [
            w * self.thickness * self.taper(1, t),
            t * self.hair_length(0.0),
        ]
    }
}

/// The bundle's outline: `OUTLINE_STEPS` points down the left edge (the root, not the
/// corner), `OUTLINE_STEPS + 1` across the tip (both corners), `OUTLINE_STEPS` up the right.
fn outline(at: impl Fn(f32, f32) -> [f32; 2]) -> Vec<[f32; 2]> {
    let step = |i: usize| i as f32 / OUTLINE_STEPS as f32;
    let left = (0..OUTLINE_STEPS).map(|i| at(-1.0, step(i)));
    let tip = (0..=OUTLINE_STEPS).map(|i| at(2.0 * step(i) - 1.0, 1.0));
    let right = (0..OUTLINE_STEPS).rev().map(|i| at(1.0, step(i)));
    left.chain(tip).chain(right).collect()
}

/// `gaps - 1` guide lines, evenly spread between the two edges.
fn hairs(at: impl Fn(f32, f32) -> [f32; 2], gaps: usize) -> Vec<Vec<[f32; 2]>> {
    (1..gaps)
        .map(|i| {
            let u = 2.0 * i as f32 / gaps as f32 - 1.0;
            (0..=HAIR_STEPS)
                .map(|j| at(u, j as f32 / HAIR_STEPS as f32 * HAIR_REACH))
                .collect()
        })
        .collect()
}

fn footprint(shape: &Shape) -> Vec<[f32; 2]> {
    (0..FOOTPRINT_STEPS)
        .map(|i| {
            let angle = i as f32 / FOOTPRINT_STEPS as f32 * TAU;
            [
                shape.front(angle.cos(), 1.0)[0],
                shape.side(angle.sin(), 1.0)[0],
            ]
        })
        .collect()
}

/// The ferrule seen across `half_width` (1 face on, the thickness edge on).
fn ferrule(half_width: f32, thickness: f32) -> Ferrule {
    let mouth = half_width * FERRULE_MOUTH;
    let [low, high] = HANDLE_RADIUS_RANGE;
    let radius = (HANDLE_RADIUS_BASE + HANDLE_RADIUS_PER_THICKNESS * thickness).clamp(low, high);
    let edge: Vec<[f32; 2]> = (0..=FERRULE_STEPS)
        .map(|i| {
            let v = i as f32 / FERRULE_STEPS as f32;
            [ferrule_half_width(v, mouth, radius), -v * FERRULE_HEIGHT]
        })
        .collect();
    let left = edge.iter().map(|&[x, y]| [-x, y]);
    let outline = left.chain(edge.iter().rev().copied()).collect();
    let top = -FERRULE_HEIGHT - HANDLE_HEIGHT;
    Ferrule {
        outline,
        mouth,
        crimp: CRIMP.map(|v| -v * FERRULE_HEIGHT),
        handle: [[radius, -FERRULE_HEIGHT], [radius * HANDLE_FLARE, top]],
    }
}

/// The ferrule's half-width at `v` (0 at the mouth, 1 at the top): the head's section,
/// crimped, then turning round into the handle.
fn ferrule_half_width(v: f32, mouth: f32, radius: f32) -> f32 {
    let crimp = if v > CRIMP[0] && v < CRIMP[1] {
        -CRIMP_DEPTH * ((v - CRIMP[0]) / (CRIMP[1] - CRIMP[0]) * PI).sin()
    } else {
        0.0
    };
    let neck = ((v - NECK_START) / NECK_SPAN).clamp(0.0, 1.0);
    mouth + (radius - mouth) * neck * neck * (3.0 - 2.0 * neck) + crimp
}

#[cfg(test)]
mod tests;
