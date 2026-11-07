use crate::brush::{BrushSettings, DEFAULT_MAX_DIAMETER_PX, FULL_RESERVOIR_VOLUME_UM};
use crate::canvas::{Canvas, TileCoord};
use crate::compute::storage_buffer;
use crate::gpu::{GpuContext, GpuLost};
use crate::params::SimParams;
use crate::pigments::{MAX_SLOTS, Mixture};
use crate::stroke::Dab;

mod bristles;
mod dab_record;
mod face_load;
mod footprint;
mod knife_load;
mod pass;
mod placement;
mod readout;
mod reservoir;
pub(crate) mod shader;

pub use bristles::{
    BRISTLE_SETTLE_MS, BristleCell, BristleSnapshot, RELIEF_UM_PER_PX, bristle_reach_px,
};
pub use dab_record::blade_gap_um;
pub use footprint::BrushOutline;
pub use readout::BrushReadout;

pub(crate) use pass::pick::PickRequest;

use bristles::{BristleBuffers, BristleRig, BristleTuning, LayoutRequest, MAX_BRISTLES};
use dab_record::{upload_dab_records, upload_records};
use face_load::{FACE_DELTA_BYTES, FACE_LOAD_BYTES, FaceSettings};
use pass::dip::DipPass;
use pass::dry::DryPass;
use pass::pick::PickPass;
use pass::{DabPasses, DabResources};
use placement::{FirstRest, PreviousDab, mark_dab_regions};

pub const TIP_SIZE: u32 = 512;
pub const TIP_MARGIN: u32 = 12;
pub const DELTA_SCALE: f32 = 65536.0;
pub const MAX_TIP_THICKNESS_UM: f32 = FULL_RESERVOIR_VOLUME_UM;

const TIP_HALF: u32 = TIP_SIZE / 2;
const MAX_TIP_RADIUS: f32 = ((TIP_SIZE - TIP_MARGIN) / 2) as f32;
const MIN_DAB_RADIUS: f32 = 0.5;
const PIGMENT_CHANNELS: usize = MAX_SLOTS;
const VOLUME_CHANNEL: usize = PIGMENT_CHANNELS;
const TIP_CHANNELS: usize = PIGMENT_CHANNELS + 1;
const TIP_TEXELS: usize = (TIP_SIZE * TIP_SIZE) as usize;
const TIP_VALUES: usize = TIP_TEXELS * TIP_CHANNELS;
const TIP_BYTES: u64 = (TIP_VALUES * std::mem::size_of::<f32>()) as u64;
pub const FULL_DIP: f32 = 1.0;
const TIP_SUM_CHANNELS: usize = TIP_CHANNELS + 1;
const TIP_SUMS_BYTES: u64 = (TIP_SUM_CHANNELS * std::mem::size_of::<f32>()) as u64;
const CAPILLARY_POOLS: u64 = MAX_BRISTLES as u64;
const CAPILLARY_BYTES: u64 = (3 * CAPILLARY_POOLS * TIP_CHANNELS as u64 + CAPILLARY_POOLS)
    * std::mem::size_of::<u32>() as u64;
const KNIFE_DIP_LENGTH_PX: f32 = 380.0;
const KNIFE_LAID_CAPACITY_UM: f32 = MAX_TIP_THICKNESS_UM * KNIFE_DIP_LENGTH_PX;
const BLADE_SPILL_BAND_PX: f32 = 2.0;
const BLADE_COVERAGE_MARGIN_PX: f32 = 1.0;
const BLADE_ENDS: usize = 2;
const BLADE_CELLS: usize = TIP_SIZE as usize;
const BLADE_DELTA_CHANNELS: usize = TIP_CHANNELS + 1;
const BLADE_SPILL_VALUES: usize = BLADE_ENDS * TIP_CHANNELS;
const BLADE_LOAD_VALUES: usize = BLADE_CELLS * TIP_CHANNELS;
const BLADE_LOAD_BYTES: u64 = (BLADE_LOAD_VALUES * std::mem::size_of::<f32>()) as u64;
const BLADE_DELTA_BYTES: u64 =
    (BLADE_CELLS * BLADE_DELTA_CHANNELS * std::mem::size_of::<i32>()) as u64;
const BLADE_CELL_GROUP_SIZE: u32 = 64;
const DRY_WORKGROUP_SIZE: u32 = 8;
const BRISTLE_LAYOUT_SEED: u64 = 0x00C4_D317_B215_7E55;

/// The ferrule radius a brush of `diameter_px` paints with: half the
/// diameter, clamped to what the tip texture can hold.
pub fn ferrule_radius_for(diameter_px: f32) -> f32 {
    (diameter_px / 2.0).clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS)
}

pub struct DabBatch<'a> {
    pub brush: &'a BrushSettings,
    pub dabs: &'a [Dab],
}

pub trait PaintEngine {
    fn begin_stroke(&mut self);
    fn end_stroke(&mut self);
    fn encode_dabs(
        &mut self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        batch: DabBatch<'_>,
        encoder: &mut wgpu::CommandEncoder,
    );
    fn settle_bristles(
        &mut self,
        gpu: &GpuContext,
        canvas: &Canvas,
        now_ms: f32,
        encoder: &mut wgpu::CommandEncoder,
    );
    fn params(&self) -> &SimParams;
    fn params_mut(&mut self) -> &mut SimParams;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BodyState {
    pigment: [f32; PIGMENT_CHANNELS],
    volume: f32,
    granted_ratio: f32,
    granted_fractions: [f32; PIGMENT_CHANNELS],
}

const BODY_BYTES: u64 = std::mem::size_of::<BodyState>() as u64;

const WANTED_BYTES: u64 = 2 * std::mem::size_of::<u32>() as u64;

struct Buffers {
    tip: wgpu::Buffer,
    tip_delta: wgpu::Buffer,
    body: wgpu::Buffer,
    wanted: wgpu::Buffer,
    capillary_pools: wgpu::Buffer,
    tip_sums: wgpu::Buffer,
    blade_load: wgpu::Buffer,
    blade_delta: wgpu::Buffer,
    face_load: wgpu::Buffer,
    face_delta: wgpu::Buffer,
    bristles: BristleBuffers,
}

impl Buffers {
    fn new(gpu: &GpuContext) -> Buffers {
        Buffers {
            tip: storage_buffer(gpu, "brush_tip", TIP_BYTES),
            tip_delta: storage_buffer(gpu, "brush_tip_delta", TIP_BYTES),
            body: storage_buffer(gpu, "brush_body", BODY_BYTES),
            wanted: storage_buffer(gpu, "brush_refill_wanted", WANTED_BYTES),
            capillary_pools: storage_buffer(gpu, "brush_capillary_pools", CAPILLARY_BYTES),
            tip_sums: storage_buffer(gpu, "brush_tip_sums", TIP_SUMS_BYTES),
            blade_load: storage_buffer(gpu, "knife_blade_load", BLADE_LOAD_BYTES),
            blade_delta: storage_buffer(gpu, "knife_blade_delta", BLADE_DELTA_BYTES),
            face_load: storage_buffer(gpu, "mixing_knife_face_load", FACE_LOAD_BYTES),
            face_delta: storage_buffer(gpu, "mixing_knife_face_delta", FACE_DELTA_BYTES),
            bristles: BristleBuffers::new(gpu),
        }
    }
}

pub struct WetOilEngine {
    params: SimParams,
    tip_radius: f32,
    loaded_thickness_um: f32,
    buffers: Buffers,
    first_rest: FirstRest,
    previous_dab: Option<PreviousDab>,
    bristles: BristleRig,
    dab_passes: DabPasses,
    pick_pass: PickPass,
    dry_pass: DryPass,
    dip_pass: DipPass,
}

impl WetOilEngine {
    pub fn new(gpu: &GpuContext) -> Self {
        WetOilEngine {
            params: SimParams::default(),
            tip_radius: DEFAULT_MAX_DIAMETER_PX / 2.0,
            loaded_thickness_um: 0.0,
            buffers: Buffers::new(gpu),
            first_rest: FirstRest::new(),
            previous_dab: None,
            bristles: BristleRig::new(),
            dab_passes: DabPasses::new(gpu),
            pick_pass: PickPass::new(gpu),
            dry_pass: DryPass::new(gpu),
            dip_pass: DipPass::new(gpu),
        }
    }

    pub fn resize_tip(&mut self, radius_px: f32) {
        if radius_px.is_finite() {
            self.tip_radius = ferrule_radius_for(2.0 * radius_px);
        }
    }

    fn fit_bristles(
        &mut self,
        gpu: &GpuContext,
        brush: &BrushSettings,
        canvas: &Canvas,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if !brush.tool.profile().moves_bristles() {
            self.bristles.put_down();
            return;
        }
        self.bristles.take_up(brush.tool);
        let head = brush.head.clamped();
        let hair = brush.hair.clamped();
        self.bristles.wear(hair);
        let request = LayoutRequest {
            head,
            ferrule_radius_px: self.tip_radius,
            seed: BRISTLE_LAYOUT_SEED,
        };
        self.bristles
            .fit(gpu, &self.buffers.bristles, &request, encoder);
        let tuning = BristleTuning::of(&self.params, &head, &hair, canvas.tooth_height_um());
        self.buffers.bristles.tune(gpu, &tuning, encoder);
    }

    pub fn bristles_settling(&self, now_ms: f32) -> bool {
        self.bristles.settling(now_ms)
    }

    pub(crate) fn encode_eyedropper(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        request: PickRequest,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.pick_pass.encode(gpu, canvas, request, encoder);
    }

    pub fn picked_mixture(&self, gpu: &GpuContext) -> Result<Option<Mixture>, GpuLost> {
        self.pick_pass.picked_mixture(gpu, self.loaded_thickness_um)
    }

    pub fn encode_dry_tiles(
        &mut self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        tiles: &[TileCoord],
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.dry_pass.encode(gpu, canvas, tiles, encoder);
    }
}

impl PaintEngine for WetOilEngine {
    fn begin_stroke(&mut self) {
        self.first_rest.begin_stroke();
        self.previous_dab = None;
        self.bristles.begin_stroke();
    }

    fn end_stroke(&mut self) {
        self.bristles.lift();
    }

    fn encode_dabs(
        &mut self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        batch: DabBatch<'_>,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.fit_bristles(gpu, batch.brush, canvas, encoder);
        let mut placements = self.place_all(canvas, batch.brush, batch.dabs);
        if placements.is_empty() {
            return;
        }
        self.first_rest.take(&mut placements);
        let dab_records = upload_dab_records(gpu, &placements);
        self.dab_passes.fit_scratch(gpu, &placements);
        let resources = DabResources {
            gpu,
            canvas,
            dab_records: &dab_records,
            buffers: &self.buffers,
            face_settings: FaceSettings::of(&self.params),
        };
        let bind_groups = self.dab_passes.bind_groups(&resources);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("dab_exchange"),
            timestamp_writes: None,
        });
        self.dab_passes.encode(&bind_groups, &mut pass, &placements);
        drop(pass);
        mark_dab_regions(canvas, &placements);
    }

    fn settle_bristles(
        &mut self,
        gpu: &GpuContext,
        canvas: &Canvas,
        now_ms: f32,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let Some(record) = self.bristles.settle(now_ms) else {
            return;
        };
        let tuning = BristleTuning::of(
            &self.params,
            &self.bristles.head(),
            &self.bristles.hair(),
            canvas.tooth_height_um(),
        );
        self.buffers.bristles.tune(gpu, &tuning, encoder);
        let records = upload_records(gpu, &[record]);
        let resources = DabResources {
            gpu,
            canvas,
            dab_records: &records,
            buffers: &self.buffers,
            face_settings: FaceSettings::of(&self.params),
        };
        self.dab_passes.encode_bristles_alone(&resources, encoder);
    }

    fn params(&self) -> &SimParams {
        &self.params
    }

    fn params_mut(&mut self) -> &mut SimParams {
        &mut self.params
    }
}
