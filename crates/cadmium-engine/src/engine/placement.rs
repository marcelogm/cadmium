use super::dab_record::{
    DabRecord, blade_capacity_um, blade_gap_um, capillary_scale, rate_over_step,
};
use super::footprint::Footprint;
use super::{MAX_TIP_RADIUS, TIP_HALF, WetOilEngine};
use crate::brush::{BrushSettings, DEFAULT_MAX_DIAMETER_PX, DEFAULT_SPACING};
use crate::canvas::Canvas;
use crate::compute::WORKGROUP_SIZE;
use crate::region::clamped_region;
use crate::stroke::Dab;
use crate::tool::ToolProfile;

const REFERENCE_STEP_PX: f32 = DEFAULT_SPACING * DEFAULT_MAX_DIAMETER_PX;
const DAB_APRON_PX: i32 = 2;
const MAX_SWEEP_PX: f32 = 32.0;
const CAPILLARY_SHARE: f32 = 0.2;
const SPREAD_APRON_PX: i32 = 1;
const TIP_APRON_TEXELS: f32 = 1.0;
const KNIFE_SPILL_SHARE: f32 = 0.5;
const FIRST_REST_FOLLOW: f32 = 1.0;
const _: () = assert!(DEFAULT_SPACING * 2.0 * MAX_TIP_RADIUS < MAX_SWEEP_PX);

#[derive(Clone, Copy, Debug)]
pub(super) struct PreviousDab {
    at: [f32; 2],
    placed: bool,
}

struct DabStep {
    px: f32,
    sweep: [f32; 2],
}

const STILL_STEP: DabStep = DabStep {
    px: REFERENCE_STEP_PX,
    sweep: [0.0; 2],
};

struct DabRegions {
    footprint: Footprint,
    canvas_origin: [i32; 2],
    canvas_size: [u32; 2],
    spread_origin: [i32; 2],
    spread_size: [u32; 2],
    tip_origin: [i32; 2],
    tip_size: [u32; 2],
}

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
        dabs.iter()
            .filter_map(|dab| self.place_dab(canvas, brush, dab))
            .collect()
    }

    fn place_dab(
        &mut self,
        canvas: &Canvas,
        brush: &BrushSettings,
        dab: &Dab,
    ) -> Option<Placement> {
        let step = self.step_since_previous_dab(dab);
        let mut placement = self.place(canvas, brush, dab, step)?;
        self.mark_previous_dab_placed();
        if placement.profile.moves_bristles() {
            self.bristles
                .advance(&mut placement.record, dab, &self.params);
        }
        Some(placement)
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

    fn step_since_previous_dab(&mut self, dab: &Dab) -> DabStep {
        if !is_finite(dab) {
            return STILL_STEP;
        }
        let previous = self.previous_dab.replace(PreviousDab {
            at: [dab.x, dab.y],
            placed: false,
        });
        let Some(PreviousDab { at: [x, y], placed }) = previous else {
            return STILL_STEP;
        };
        let back = [x - dab.x, y - dab.y];
        let step = back[0].hypot(back[1]);
        let sweeps = placed && step <= MAX_SWEEP_PX;
        DabStep {
            px: step,
            sweep: if sweeps { back } else { [0.0; 2] },
        }
    }

    fn mark_previous_dab_placed(&mut self) {
        if let Some(previous) = self.previous_dab.as_mut() {
            previous.placed = true;
        }
    }

    pub fn dab_bounds(
        &self,
        canvas: &Canvas,
        brush: &BrushSettings,
        dab: &Dab,
    ) -> Option<[f32; 4]> {
        self.place(canvas, brush, dab, STILL_STEP)
            .map(|placement| placement.canvas_rect)
    }

    fn place(
        &self,
        canvas: &Canvas,
        brush: &BrushSettings,
        dab: &Dab,
        step: DabStep,
    ) -> Option<Placement> {
        let profile = brush.tool.profile();
        if !profile.places_dabs() || !is_finite(dab) {
            return None;
        }
        if profile.has_blade_footprint() && !heading_known(dab) {
            return None;
        }
        let footprint = Footprint::of(dab, brush, self.tip_radius);
        let regions = DabRegions::around(dab, step.sweep, footprint, canvas)?;
        Some(Placement {
            record: self.record(brush, dab, step, &regions),
            canvas_rect: regions.canvas_rect(),
            canvas_groups: regions.canvas_size.map(|s| s.div_ceil(WORKGROUP_SIZE)),
            tip_groups: regions.tip_size.map(|s| s.div_ceil(WORKGROUP_SIZE)),
            spread_groups: regions.spread_size.map(|s| s.div_ceil(WORKGROUP_SIZE)),
            profile,
        })
    }

    fn record(
        &self,
        brush: &BrushSettings,
        dab: &Dab,
        step: DabStep,
        regions: &DabRegions,
    ) -> DabRecord {
        let footprint = &regions.footprint;
        let (sin, cos) = footprint.orientation.sin_cos();
        let step_ratio = step.px / REFERENCE_STEP_PX;
        DabRecord {
            center: [dab.x, dab.y],
            radius: footprint.radius,
            pressure: dab.pressure,
            altitude: dab.altitude,
            azimuth: dab.azimuth,
            direction: dab.direction,
            speed: dab.speed,
            orientation: [cos, sin],
            elongation: footprint.elongation,
            tool_mode: brush.tool.profile().shader_mode,
            canvas_origin: regions.canvas_origin,
            canvas_size: regions.canvas_size,
            tip_origin: regions.tip_origin,
            tip_size: regions.tip_size,
            spread_origin: regions.spread_origin,
            spread_size: regions.spread_size,
            sweep: step.sweep,
            ..self.transfer_settings(brush, dab, step_ratio, regions)
        }
    }

    fn transfer_settings(
        &self,
        brush: &BrushSettings,
        dab: &Dab,
        step_ratio: f32,
        regions: &DabRegions,
    ) -> DabRecord {
        let params = &self.params;
        let profile = brush.tool.profile();
        let spreading = profile.spreading.rates(params);
        DabRecord {
            transfer_fraction: params.get("transfer_fraction"),
            velocity_cutoff: params.get("velocity_cutoff"),
            residual_um: params.get("residual_um"),
            penetration_um: params.get("penetration_um"),
            bristle_contact: u32::from(profile.has_bristle_footprint()),
            refill_rate: rate_over_step(params.get("refill_rate"), step_ratio),
            loaded_thickness_um: self.loaded_thickness_um,
            full_reserve_volume: self.reserve_volume(self.loaded_thickness_um),
            advection: spreading.advection,
            squeeze: spreading.squeeze,
            ferrule_press_share: params.get("ferrule_press_share"),
            diffusion: spreading.diffusion,
            blade_gap_um: blade_gap_um(params, dab.pressure, dab.altitude),
            rag_strength: params.get("rag_strength"),
            rest_follow: profile.rest_follow,
            blade_capacity_um: blade_capacity_um(dab.altitude),
            blade_spill_share: KNIFE_SPILL_SHARE,
            step_ratio,
            capillary_rate: rate_over_step(CAPILLARY_SHARE, step_ratio),
            wicking_rate: rate_over_step(params.get("wicking_share"), step_ratio),
            slump_rate: rate_over_step(spreading.slump, step_ratio),
            slump_yield_um2_per_px: params.get("slump_yield_um2_per_px"),
            hair_load_limit: params.get("bristle_load_limit"),
            capillary_scale: capillary_scale(regions.tip_size),
            ..bytemuck::Zeroable::zeroed()
        }
    }
}

impl DabRegions {
    fn around(
        dab: &Dab,
        sweep: [f32; 2],
        footprint: Footprint,
        canvas: &Canvas,
    ) -> Option<DabRegions> {
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
        let (tip_origin, tip_size) = tip_window(footprint.semi_axes());
        Some(DabRegions {
            footprint,
            canvas_origin,
            canvas_size,
            spread_origin,
            spread_size,
            tip_origin,
            tip_size,
        })
    }

    fn canvas_rect(&self) -> [f32; 4] {
        let [x0, y0] = self.canvas_origin.map(|origin| origin as f32);
        let [x1, y1] =
            [0, 1].map(|axis| (self.canvas_origin[axis] + self.canvas_size[axis] as i32) as f32);
        [x0, y0, x1 - 1.0, y1 - 1.0]
    }
}

pub(super) fn mark_dab_regions(canvas: &mut Canvas, placements: &[Placement]) {
    for placement in placements {
        let [x0, y0, x1, y1] = placement.canvas_rect;
        if placement.profile.has_bristle_footprint() {
            canvas.possibly_dirty.mark_rect(x0, y0, x1, y1);
        } else {
            canvas.dirty.mark_rect(x0, y0, x1, y1);
        }
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
