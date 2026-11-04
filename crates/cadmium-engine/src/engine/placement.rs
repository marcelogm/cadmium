use super::dab_record::{
    DabRecord, blade_capacity_um, blade_gap_um, bristle_spread, capillary_scale, rate_over_step,
};
use super::footprint::{Footprint, knife_half_width};
use super::{
    BLADE_COVERAGE_MARGIN_PX, BLADE_SPILL_BAND_PX, MAX_TIP_RADIUS, TIP_HALF, WetOilEngine,
};
use crate::brush::{BrushSettings, DEFAULT_MAX_DIAMETER_PX, DEFAULT_SPACING};
use crate::canvas::Canvas;
use crate::compute::WORKGROUP_SIZE;
use crate::params::SimParams;
use crate::region::clamped_region;
use crate::stroke::Dab;
use crate::tool::ToolProfile;

const REFERENCE_STEP_PX: f32 = DEFAULT_SPACING * DEFAULT_MAX_DIAMETER_PX;
const DAB_APRON_PX: i32 = 2;
const MAX_SWEEP_PX: f32 = 32.0;
const CAPILLARY_SHARE: f32 = 0.2;
const CROSS_CAPILLARY_SHARE: f32 = 0.01;
const SPREAD_APRON_PX: i32 = 1;
const TIP_APRON_TEXELS: f32 = 1.0;
const MIN_BRISTLE_PERIOD_PX: f32 = 3.0;
const KNIFE_SPILL_SHARE: f32 = 0.5;
const FIRST_REST_FOLLOW: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct Placement {
    pub(super) record: DabRecord,
    pub(super) canvas_rect: [f32; 4],
    pub(super) canvas_groups: [u32; 2],
    pub(super) tip_groups: [u32; 2],
    pub(super) spread_groups: [u32; 2],
    pub(super) profile: &'static ToolProfile,
}

impl WetOilEngine {
    pub(super) fn place_all(
        &mut self,
        canvas: &Canvas,
        brush: &BrushSettings,
        dabs: &[Dab],
    ) -> Vec<Placement> {
        let mut placements = Vec::with_capacity(dabs.len());
        for dab in dabs {
            let (step_px, sweep) = self.step_since_previous_dab(dab);
            placements.extend(place(
                dab,
                step_px,
                sweep,
                brush,
                &self.params,
                self.loaded_thickness_um,
                canvas,
            ));
        }
        placements
    }

    pub(super) fn take_first_rest(&mut self, placements: &mut [Placement]) {
        if let Some(first_resting) = placements
            .iter_mut()
            .find(|placement| !placement.profile.wipes())
            && self.rest_pending
        {
            first_resting.record.rest_follow = FIRST_REST_FOLLOW;
            self.rest_pending = false;
        }
    }

    fn step_since_previous_dab(&mut self, dab: &Dab) -> (f32, [f32; 2]) {
        if !is_finite(dab) {
            return (REFERENCE_STEP_PX, [0.0; 2]);
        }
        let back = self.previous_dab.map(|[x, y]| [x - dab.x, y - dab.y]);
        self.previous_dab = Some([dab.x, dab.y]);
        let Some(back) = back else {
            return (REFERENCE_STEP_PX, [0.0; 2]);
        };
        let step = back[0].hypot(back[1]);
        let sweep = if step <= MAX_SWEEP_PX { back } else { [0.0; 2] };
        (step, sweep)
    }

    pub fn dab_bounds(
        &self,
        canvas: &Canvas,
        brush: &BrushSettings,
        dab: &Dab,
    ) -> Option<[f32; 4]> {
        place(
            dab,
            REFERENCE_STEP_PX,
            [0.0; 2],
            brush,
            &self.params,
            self.loaded_thickness_um,
            canvas,
        )
        .map(|placement| placement.canvas_rect)
    }
}

impl Placement {
    pub(super) fn fits_within(&self, size: [u32; 2]) -> bool {
        (0..2).all(|axis| self.record.canvas_size[axis] <= size[axis])
    }
}

pub(super) fn mark_dab_regions(canvas: &mut Canvas, placements: &[Placement]) {
    for placement in placements {
        let [x0, y0, x1, y1] = placement.canvas_rect;
        canvas.dirty.mark_rect(x0, y0, x1, y1);
        canvas.wet_tiles.mark_rect(x0, y0, x1, y1);
    }
}

fn heading_known(dab: &Dab) -> bool {
    dab.speed > 0.0
}

fn is_finite(dab: &Dab) -> bool {
    [
        dab.x,
        dab.y,
        dab.radius,
        dab.pressure,
        dab.altitude,
        dab.azimuth,
        dab.direction,
        dab.speed,
    ]
    .iter()
    .all(|value| value.is_finite())
}

fn tip_window(semi_axes: [f32; 2]) -> ([i32; 2], [u32; 2]) {
    let reach = semi_axes.map(|semi| (semi + TIP_APRON_TEXELS).ceil() as u32);
    let origin = reach.map(|r| (TIP_HALF - r) as i32);
    let size = reach.map(|r| 2 * r + 1);
    (origin, size)
}

#[expect(clippy::too_many_arguments, clippy::too_many_lines)]
fn place(
    dab: &Dab,
    step_px: f32,
    sweep: [f32; 2],
    brush: &BrushSettings,
    params: &SimParams,
    loaded_thickness_um: f32,
    canvas: &Canvas,
) -> Option<Placement> {
    let profile = brush.tool.profile();
    if !profile.places_dabs() || !is_finite(dab) {
        return None;
    }
    if profile.has_blade_footprint() && !heading_known(dab) {
        return None;
    }
    let footprint = Footprint::of(dab, brush, params);
    let (bristle_contrast, bristle_count) = bristle_spread(brush.shape, params);
    let step_ratio = step_px / REFERENCE_STEP_PX;
    let [half_x, half_y] = footprint.canvas_half_extents();
    let swept_from = [dab.x + sweep[0], dab.y + sweep[1]];
    let first = [
        (dab.x.min(swept_from[0]) - half_x).floor() as i32,
        (dab.y.min(swept_from[1]) - half_y).floor() as i32,
    ];
    let last = [
        (dab.x.max(swept_from[0]) + half_x).floor() as i32,
        (dab.y.max(swept_from[1]) + half_y).floor() as i32,
    ];
    let limits = [canvas.spec.width as i32, canvas.spec.height as i32];
    let (canvas_origin, canvas_size) = clamped_region(first, last, DAB_APRON_PX, limits)?;
    let (spread_origin, spread_size) = clamped_region(first, last, SPREAD_APRON_PX, limits)?;
    let [x0, y0] = canvas_origin.map(|origin| origin as f32);
    let [x1, y1] = [0, 1].map(|axis| (canvas_origin[axis] + canvas_size[axis] as i32) as f32);
    let (tip_origin, tip_size) = tip_window(footprint.semi_axes());
    let (sin, cos) = footprint.orientation.sin_cos();
    let spreading = profile.spreading.rates(params);
    let record = DabRecord {
        center: [dab.x, dab.y],
        radius: footprint.radius,
        pressure: dab.pressure,
        altitude: dab.altitude,
        azimuth: dab.azimuth,
        direction: dab.direction,
        speed: dab.speed,
        orientation: [cos, sin],
        elongation: footprint.elongation,
        tool_mode: profile.shader_mode,
        canvas_origin,
        canvas_size,
        tip_origin,
        tip_size,
        transfer_fraction: params.get("transfer_fraction"),
        velocity_cutoff: params.get("velocity_cutoff"),
        residual_um: params.get("residual_um"),
        penetration_um: params.get("penetration_um"),
        bristle_contrast: if profile.wipes() {
            0.0
        } else {
            bristle_contrast
        },
        bristle_count: bristle_count.min(2.0 * footprint.radius / MIN_BRISTLE_PERIOD_PX),
        refill_rate: rate_over_step(params.get("refill_rate"), step_ratio),
        loaded_thickness_um,
        advection: spreading.advection,
        squeeze: spreading.squeeze,
        diffusion: spreading.diffusion,
        blade_gap_um: blade_gap_um(params, dab.pressure, dab.altitude),
        spread_origin,
        spread_size,
        rag_strength: params.get("rag_strength"),
        rest_follow: profile.rest_follow,
        blade_capacity_um: blade_capacity_um(dab.altitude),
        blade_spill_share: KNIFE_SPILL_SHARE,
        step_ratio,
        sweep,
        capillary_rate: rate_over_step(CAPILLARY_SHARE, step_ratio),
        cross_capillary_rate: rate_over_step(CROSS_CAPILLARY_SHARE, step_ratio),
        capillary_scale: capillary_scale(tip_size),
        _pad: [0.0; 2],
    };
    Some(Placement {
        record,
        canvas_rect: [x0, y0, x1 - 1.0, y1 - 1.0],
        canvas_groups: canvas_size.map(|s| s.div_ceil(WORKGROUP_SIZE)),
        tip_groups: tip_size.map(|s| s.div_ceil(WORKGROUP_SIZE)),
        spread_groups: spread_size.map(|s| s.div_ceil(WORKGROUP_SIZE)),
        profile,
    })
}

pub(super) fn max_dab_region_side() -> u32 {
    let along = knife_half_width(MAX_TIP_RADIUS) + BLADE_COVERAGE_MARGIN_PX;
    let across = MAX_TIP_RADIUS + BLADE_SPILL_BAND_PX + BLADE_COVERAGE_MARGIN_PX;
    let half_extent = across.hypot(along).ceil() as u32;
    2 * (half_extent + 1 + DAB_APRON_PX as u32) + 2 * MAX_SWEEP_PX as u32
}
