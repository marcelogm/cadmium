use super::handle::{HandleClock, HandleGauge, ferrule_over_nib, substeps};
use super::layout::{BristleLayout, LayoutRequest, layout};
use super::state::BristleBuffers;
use crate::brush::{HairSpec, HeadSpec};
use crate::engine::dab_record::DabRecord;
use crate::gpu::GpuContext;
use crate::params::SimParams;
use crate::stroke::{Dab, Tool};

const ROUND_ROOT_ORIENTATION: [f32; 2] = [0.0, 1.0];
pub const BRISTLE_SETTLE_MS: f32 = 1500.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LayoutKey {
    head_bits: [u32; 7],
    ferrule_radius_bits: u32,
}

impl LayoutKey {
    fn of(request: &LayoutRequest) -> LayoutKey {
        let h = request.head;
        LayoutKey {
            head_bits: [
                h.thickness,
                h.length,
                h.dome,
                h.slant,
                h.belly,
                h.fan_deg,
                h.density_per_px,
            ]
            .map(f32::to_bits),
            ferrule_radius_bits: request.ferrule_radius_px.to_bits(),
        }
    }
}

pub(crate) struct BristleRig {
    key: Option<LayoutKey>,
    count: u32,
    radii_px: Vec<f32>,
    clusters: Vec<u32>,
    mean_radius_px: f32,
    reach_px: f32,
    ferrule_radius_px: f32,
    root_half_extent_px: [f32; 2],
    head: HeadSpec,
    hair: HairSpec,
    tool: Option<Tool>,
    previous_orientation: Option<[f32; 2]>,
    reset_pending: bool,
    clock: HandleClock,
    last_dab: Option<DabRecord>,
    pressed: bool,
    lift_pending: bool,
    last_motion_ms: f32,
}

impl BristleRig {
    pub(crate) fn new() -> BristleRig {
        BristleRig {
            key: None,
            count: 0,
            radii_px: Vec::new(),
            clusters: Vec::new(),
            mean_radius_px: 0.0,
            reach_px: 0.0,
            ferrule_radius_px: 0.0,
            root_half_extent_px: [0.0; 2],
            head: HeadSpec::ROUND,
            hair: HairSpec::default(),
            tool: None,
            previous_orientation: None,
            reset_pending: true,
            clock: HandleClock::default(),
            last_dab: None,
            pressed: false,
            lift_pending: false,
            last_motion_ms: 0.0,
        }
    }

    pub(crate) fn radii_px(&self) -> &[f32] {
        &self.radii_px
    }

    pub(crate) fn in_hand(&self) -> bool {
        self.tool.is_some()
    }

    pub(crate) fn count(&self) -> u32 {
        self.count
    }

    pub(crate) fn ferrule_radius_px(&self) -> f32 {
        self.ferrule_radius_px
    }

    pub(crate) fn root_half_extent_px(&self) -> [f32; 2] {
        self.root_half_extent_px
    }

    pub(crate) fn head(&self) -> HeadSpec {
        self.head
    }

    pub(crate) fn wear(&mut self, hair: HairSpec) {
        self.hair = hair;
    }

    pub(crate) fn hair(&self) -> HairSpec {
        self.hair
    }

    pub(crate) fn clusters(&self) -> &[u32] {
        &self.clusters
    }

    pub(crate) fn reset(&mut self) {
        self.reset_pending = true;
        self.last_dab = None;
        self.previous_orientation = None;
        self.clock.forget_pose();
    }

    pub(crate) fn take_up(&mut self, tool: Tool) {
        if self.tool != Some(tool) {
            self.tool = Some(tool);
            self.reset();
        }
    }

    pub(crate) fn put_down(&mut self) {
        self.tool = None;
    }

    pub(crate) fn begin_stroke(&mut self) {
        self.clock.begin_stroke();
        self.last_dab = None;
        self.pressed = true;
        self.lift_pending = false;
    }

    pub(crate) fn lift(&mut self) {
        self.lift_pending |= std::mem::take(&mut self.pressed);
    }

    pub(crate) fn settling(&self, now_ms: f32) -> bool {
        let still_moving = self.lift_pending || now_ms - self.last_motion_ms < BRISTLE_SETTLE_MS;
        self.tool.is_some() && self.last_dab.is_some() && still_moving
    }

    pub(crate) fn fit(
        &mut self,
        gpu: &GpuContext,
        buffers: &BristleBuffers,
        request: &LayoutRequest,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.head = request.head;
        let key = LayoutKey::of(request);
        if self.key == Some(key) {
            return;
        }
        let laid = layout(request);
        buffers.upload(gpu, &laid, encoder);
        self.adopt(&laid, request.ferrule_radius_px);
        self.key = Some(key);
        self.reset();
    }

    fn adopt(&mut self, laid: &BristleLayout, ferrule_radius_px: f32) {
        let bristles = &laid.bristles;
        let radius_sum_px: f32 = bristles.iter().map(|bristle| bristle.radius_px).sum();
        self.count = bristles.len() as u32;
        self.radii_px = bristles.iter().map(|bristle| bristle.radius_px).collect();
        self.clusters = bristles.iter().map(|bristle| bristle.cluster).collect();
        self.mean_radius_px = radius_sum_px / bristles.len().max(1) as f32;
        self.reach_px = bristles
            .iter()
            .map(|bristle| bristle.length_px)
            .fold(0.0, f32::max);
        self.ferrule_radius_px = ferrule_radius_px;
        self.root_half_extent_px = bristles.iter().fold([0.0; 2], |extent, bristle| {
            [0, 1].map(|axis| extent[axis].max(bristle.root_px[axis].abs()))
        });
    }

    pub(crate) fn advance(&mut self, record: &mut DabRecord, dab: &Dab, params: &SimParams) {
        let gauge = HandleGauge {
            reach_px: self.reach_px,
            ferrule_radius_px: self.ferrule_radius_px,
            press_depth: params.get("bristle_press_depth"),
        };
        let pose = gauge.pose(dab);
        let lifted = gauge.lifted(dab);
        let travel_px = self.clock.travel_px(pose.ferrule_px, lifted.ferrule_px);
        record.handle_height_px = pose.ferrule_px[2];
        record.handle_axis = pose.axis;
        record.handle_rest_height_px = lifted.ferrule_px[2];
        record.ferrule_radius_px = self.ferrule_radius_px;
        record.dt_seconds = self.clock.elapsed_seconds(dab.time_ms);
        record.substeps = substeps(travel_px, self.mean_radius_px);
        record.bristle_total = self.count;
        record.bristle_stiffness = self.hair.stiffness;
        record.bristle_damping = self.hair.damping;
        record.bristle_reset = u32::from(std::mem::take(&mut self.reset_pending));
        record.bristle_orientation = self.continuous_orientation(record.orientation);
        self.last_dab = Some(*record);
        self.last_motion_ms = dab.time_ms;
    }

    pub(crate) fn settle(&mut self, now_ms: f32) -> Option<DabRecord> {
        self.tool?;
        let mut record = self.last_dab?;
        record.dt_seconds = self.clock.later_seconds(now_ms)?;
        if std::mem::take(&mut self.lift_pending) {
            self.last_motion_ms = now_ms;
        }
        if !self.pressed {
            record.handle_height_px += self.reach_px;
        }
        let ferrule_px =
            ferrule_over_nib(record.center, record.handle_height_px, record.handle_axis);
        let travel_px = self.clock.travel_px(ferrule_px, ferrule_px);
        record.substeps = substeps(travel_px, self.mean_radius_px);
        record.bristle_total = self.count;
        record.bristle_reset = u32::from(std::mem::take(&mut self.reset_pending));
        Some(record)
    }

    fn continuous_orientation(&mut self, orientation: [f32; 2]) -> [f32; 2] {
        if self.head.is_round() {
            return ROUND_ROOT_ORIENTATION;
        }
        let turned_back = self.previous_orientation.is_some_and(|previous| {
            previous[0] * orientation[0] + previous[1] * orientation[1] < 0.0
        });
        let continuous = if turned_back {
            orientation.map(|component| -component)
        } else {
            orientation
        };
        self.previous_orientation = Some(continuous);
        continuous
    }
}

#[cfg(test)]
mod tests;
