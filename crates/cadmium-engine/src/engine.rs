use wgpu::util::DeviceExt;

use crate::brush::{
    BrushSettings, DEFAULT_LOAD, DEFAULT_MAX_DIAMETER_PX, FULL_RESERVOIR_VOLUME_UM,
};
use crate::canvas::{Canvas, PaintTotals, StateTexture, TILE_SIZE, TileCoord};
use crate::compute::{
    ComputeStage, WORKGROUP_SIZE, buffer_binding, read_buffer, storage_buffer, storage_entry,
    texture_binding, texture_entry, uniform_entry,
};
use crate::gpu::{GpuContext, GpuLost, read_mapped, staging_buffer};
use crate::params::SimParams;
use crate::pigments::{MAX_SLOTS, Mixture};
use crate::stroke::Dab;

mod dab_record;
mod footprint;
mod placement;
pub(crate) mod shader;

pub use dab_record::blade_gap_um;
pub use footprint::BrushOutline;

use dab_record::{DAB_RECORD_SIZE, DAB_RECORD_STRIDE, upload_dab_records};
use placement::{mark_dab_regions, max_dab_region_side};
use shader::{DAB_SNIPPETS, DRY_SNIPPETS, EYEDROPPER_SNIPPETS, shader_source};

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
const TIP_TEXEL_COUNT_SUM: usize = TIP_CHANNELS;
const TIP_SUMS_BYTES: u64 = (TIP_SUM_CHANNELS * std::mem::size_of::<f32>()) as u64;
const MIN_TIP_MEAN_VOLUME_UM: f32 = 1e-3;
const CAPILLARY_POOLS: u64 = TIP_SIZE as u64 + 1;
const CAPILLARY_BYTES: u64 = (3 * CAPILLARY_POOLS * TIP_CHANNELS as u64 + 2 * CAPILLARY_POOLS)
    * std::mem::size_of::<u32>() as u64;
const CAPILLARY_POOL_GROUP_SIZE: u32 = 64;
const CAPILLARY_POOL_GROUPS: u32 = (CAPILLARY_POOLS as u32).div_ceil(CAPILLARY_POOL_GROUP_SIZE);
const KNIFE_DIP_LENGTH_PX: f32 = 380.0;
const KNIFE_LAID_CAPACITY_UM: f32 = MAX_TIP_THICKNESS_UM * KNIFE_DIP_LENGTH_PX;
const DRAWN_LOADED_THICKNESS_UM: f32 = FULL_RESERVOIR_VOLUME_UM * DEFAULT_LOAD;
const BLADE_SPILL_BAND_PX: f32 = 2.0;
const BLADE_COVERAGE_MARGIN_PX: f32 = 1.0;
const BLADE_ENDS: usize = 2;
const BLADE_CELLS: usize = TIP_SIZE as usize;
const BLADE_DELTA_CHANNELS: usize = TIP_CHANNELS + 1;
const BLADE_SPILL_VALUES: usize = BLADE_ENDS * TIP_CHANNELS;
const BLADE_STATE_VALUES: usize = 2 + BLADE_ENDS + BLADE_SPILL_VALUES;
const BLADE_LOAD_VALUES: usize = BLADE_CELLS * TIP_CHANNELS;
const BLADE_LOAD_BYTES: u64 = (BLADE_LOAD_VALUES * std::mem::size_of::<f32>()) as u64;
const BLADE_DELTA_BYTES: u64 =
    (BLADE_CELLS * BLADE_DELTA_CHANNELS * std::mem::size_of::<i32>()) as u64;
const BLADE_STATE_BYTES: u64 = (BLADE_STATE_VALUES * std::mem::size_of::<f32>()) as u64;
const BLADE_CELL_GROUP_SIZE: u32 = 64;
const BLADE_CELL_GROUPS: u32 = (BLADE_CELLS as u32).div_ceil(BLADE_CELL_GROUP_SIZE);
const MAX_PICK_RADIUS_PX: f32 = 16.0;
const MIN_PICK_RADIUS_PX: f32 = 0.5;
const DRY_WORKGROUP_SIZE: u32 = 8;
const DRY_WORKGROUPS_PER_TILE_SIDE: u32 = TILE_SIZE / DRY_WORKGROUP_SIZE;
const TIP_GROUPS: u32 = TIP_SIZE.div_ceil(WORKGROUP_SIZE);

pub trait PaintEngine {
    fn begin_stroke(&mut self);
    #[expect(clippy::too_many_arguments)]
    fn encode_dabs(
        &mut self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        brush: &BrushSettings,
        dabs: &[Dab],
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

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct DipRecord {
    fractions: [f32; PIGMENT_CHANNELS],
    tip_radius: f32,
    tip_add_um: f32,
    tip_limit_um: f32,
    body_add_um: f32,
    body_limit_um: f32,
    _pad: [f32; 3],
}

impl DipRecord {
    fn measuring(tip_radius: f32) -> DipRecord {
        DipRecord {
            tip_radius,
            ..bytemuck::Zeroable::zeroed()
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PickRecord {
    center: [f32; 2],
    radius: f32,
    _pad: f32,
}
const WANTED_BYTES: u64 = 2 * std::mem::size_of::<u32>() as u64;
const CONTACT_REFERENCE_BYTES: u64 = std::mem::size_of::<f32>() as u64;
const PICKED_PAINT_FOUND: usize = 0;
const PICKED_FRACTIONS: usize = 1;
const PICKED_BYTES: u64 =
    ((PICKED_FRACTIONS + PIGMENT_CHANNELS) * std::mem::size_of::<f32>()) as u64;

struct ScratchSurface {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

struct Scratch {
    size: [u32; 2],
    pig0: ScratchSurface,
    pig1: ScratchSurface,
    vol: ScratchSurface,
    motion: ScratchSurface,
}

impl Scratch {
    fn new(gpu: &GpuContext, size: [u32; 2]) -> Scratch {
        Scratch {
            size,
            pig0: scratch_surface(gpu, "scratch_pig0", size, wgpu::TextureFormat::Rgba16Float),
            pig1: scratch_surface(gpu, "scratch_pig1", size, wgpu::TextureFormat::Rgba16Float),
            vol: scratch_surface(gpu, "scratch_vol", size, wgpu::TextureFormat::R32Float),
            motion: scratch_surface(
                gpu,
                "scratch_motion",
                size,
                wgpu::TextureFormat::Rgba32Float,
            ),
        }
    }
}

fn scratch_surface(
    gpu: &GpuContext,
    label: &str,
    [width, height]: [u32; 2],
    format: wgpu::TextureFormat,
) -> ScratchSurface {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    ScratchSurface {
        _texture: texture,
        view,
    }
}

fn blade_cell_cover(offset: i32, half_length: f32) -> f32 {
    let centre = offset as f32 + 0.5;
    (half_length - centre.abs() + 0.5).clamp(0.0, 1.0)
}

struct BladePipelines {
    rest: wgpu::ComputePipeline,
    want: wgpu::ComputePipeline,
    grant: wgpu::ComputePipeline,
    lay: wgpu::ComputePipeline,
    settle: wgpu::ComputePipeline,
    spill: wgpu::ComputePipeline,
}

impl BladePipelines {
    fn new(gpu: &GpuContext, compute: &ComputeStage) -> BladePipelines {
        let pipeline = |entry: &str| compute.pipeline(gpu, &format!("blade_{entry}"), entry);
        BladePipelines {
            rest: pipeline("rest"),
            want: pipeline("want"),
            grant: pipeline("grant"),
            lay: pipeline("lay"),
            settle: pipeline("settle"),
            spill: pipeline("spill"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushReadout {
    pub tip_mean: Mixture,
    pub reserve: PaintTotals,
    pub knife: PaintTotals,
}

struct CapillaryPipelines {
    gather: wgpu::ComputePipeline,
    settle: wgpu::ComputePipeline,
    distribute: wgpu::ComputePipeline,
}

pub struct WetOilEngine {
    params: SimParams,
    tip_radius: f32,
    loaded_thickness_um: f32,
    tip: wgpu::Buffer,
    tip_delta: wgpu::Buffer,
    body: wgpu::Buffer,
    wanted: wgpu::Buffer,
    capillary_rows: wgpu::Buffer,
    contact_reference: wgpu::Buffer,
    picked: wgpu::Buffer,
    tip_sums: wgpu::Buffer,
    blade_load: wgpu::Buffer,
    blade_delta: wgpu::Buffer,
    blade_grant: wgpu::Buffer,
    blade_state: wgpu::Buffer,
    rest_pending: bool,
    previous_dab: Option<[f32; 2]>,
    blade_compute: ComputeStage,
    blade_pipelines: BladePipelines,
    exchange_compute: ComputeStage,
    apply_compute: ComputeStage,
    refill_compute: ComputeStage,
    copy_compute: ComputeStage,
    advect_compute: ComputeStage,
    rag_compute: ComputeStage,
    pick_compute: ComputeStage,
    dry_compute: ComputeStage,
    dip_compute: ComputeStage,
    scratch: Option<Scratch>,
    reference_pipeline: wgpu::ComputePipeline,
    exchange_pipeline: wgpu::ComputePipeline,
    apply_pipeline: wgpu::ComputePipeline,
    gather_pipeline: wgpu::ComputePipeline,
    settle_pipeline: wgpu::ComputePipeline,
    distribute_pipeline: wgpu::ComputePipeline,
    capillary_pipelines: CapillaryPipelines,
    copy_pipeline: wgpu::ComputePipeline,
    advect_pipeline: wgpu::ComputePipeline,
    rag_pipeline: wgpu::ComputePipeline,
    pick_pipeline: wgpu::ComputePipeline,
    dry_pipeline: wgpu::ComputePipeline,
    fill_body_pipeline: wgpu::ComputePipeline,
    fill_tip_pipeline: wgpu::ComputePipeline,
    sum_tip_pipeline: wgpu::ComputePipeline,
}

impl WetOilEngine {
    #[expect(clippy::too_many_lines)]
    pub fn new(gpu: &GpuContext) -> Self {
        use wgpu::StorageTextureAccess::{ReadOnly, ReadWrite, WriteOnly};
        use wgpu::TextureFormat::{R32Float, Rgba16Float, Rgba32Float};

        let exchange_compute = ComputeStage::new(
            gpu,
            "dab_exchange",
            &shader_source(&DAB_SNIPPETS, include_str!("shaders/dab_exchange.wgsl")),
            &[
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                texture_entry(1, ReadWrite, Rgba16Float),
                texture_entry(2, ReadWrite, Rgba16Float),
                texture_entry(3, ReadWrite, R32Float),
                texture_entry(4, ReadOnly, R32Float),
                storage_entry(5, true),
                storage_entry(6, false),
                storage_entry(7, false),
            ],
        );
        let apply_compute = ComputeStage::new(
            gpu,
            "tip_apply",
            &shader_source(&DAB_SNIPPETS, include_str!("shaders/tip_apply.wgsl")),
            &[
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                storage_entry(1, false),
                storage_entry(2, false),
            ],
        );
        let refill_compute = ComputeStage::new(
            gpu,
            "tip_refill",
            &shader_source(&DAB_SNIPPETS, include_str!("shaders/tip_refill.wgsl")),
            &[
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                storage_entry(1, false),
                storage_entry(2, false),
                storage_entry(3, false),
                storage_entry(4, false),
                storage_entry(5, false),
            ],
        );
        let capillary_compute = refill_compute.with_shader(
            gpu,
            "tip_capillary",
            &shader_source(&DAB_SNIPPETS, include_str!("shaders/tip_capillary.wgsl")),
        );
        let spreading_entries = |motion_access| {
            [
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                texture_entry(1, ReadOnly, Rgba16Float),
                texture_entry(2, ReadOnly, Rgba16Float),
                texture_entry(3, ReadOnly, R32Float),
                texture_entry(4, WriteOnly, Rgba16Float),
                texture_entry(5, WriteOnly, Rgba16Float),
                texture_entry(6, WriteOnly, R32Float),
                texture_entry(7, motion_access, Rgba32Float),
            ]
        };
        let copy_compute = ComputeStage::new(
            gpu,
            "region_copy",
            &shader_source(&DAB_SNIPPETS, include_str!("shaders/region_copy.wgsl")),
            &spreading_entries(WriteOnly),
        );
        let advect_compute = ComputeStage::new(
            gpu,
            "advect",
            &shader_source(&DAB_SNIPPETS, include_str!("shaders/advect.wgsl")),
            &spreading_entries(ReadOnly),
        );

        let rag_compute = ComputeStage::new(
            gpu,
            "rag",
            &shader_source(&DAB_SNIPPETS, include_str!("shaders/rag.wgsl")),
            &[
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                texture_entry(1, ReadWrite, R32Float),
            ],
        );
        let pick_compute = ComputeStage::new(
            gpu,
            "eyedropper",
            &shader_source(
                &EYEDROPPER_SNIPPETS,
                include_str!("shaders/eyedropper.wgsl"),
            ),
            &[
                uniform_entry(0, None),
                texture_entry(1, ReadOnly, Rgba16Float),
                texture_entry(2, ReadOnly, Rgba16Float),
                texture_entry(3, ReadOnly, R32Float),
                texture_entry(4, ReadOnly, Rgba16Float),
                texture_entry(5, ReadOnly, Rgba16Float),
                storage_entry(6, false),
            ],
        );
        let dry_compute = ComputeStage::new(
            gpu,
            "dry",
            &shader_source(&DRY_SNIPPETS, include_str!("shaders/dry.wgsl")),
            &[
                uniform_entry(0, None),
                storage_entry(1, true),
                texture_entry(2, ReadWrite, Rgba16Float),
                texture_entry(3, ReadWrite, Rgba16Float),
                texture_entry(4, ReadWrite, R32Float),
                texture_entry(5, ReadWrite, Rgba16Float),
                texture_entry(6, ReadWrite, Rgba16Float),
                texture_entry(7, ReadWrite, R32Float),
                texture_entry(8, WriteOnly, Rgba16Float),
                texture_entry(9, WriteOnly, Rgba16Float),
            ],
        );

        let blade_compute = ComputeStage::new(
            gpu,
            "blade",
            &shader_source(&DAB_SNIPPETS, include_str!("shaders/blade.wgsl")),
            &[
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                texture_entry(1, ReadWrite, Rgba16Float),
                texture_entry(2, ReadWrite, Rgba16Float),
                texture_entry(3, ReadWrite, R32Float),
                texture_entry(4, ReadOnly, R32Float),
                storage_entry(5, false),
                storage_entry(6, false),
                storage_entry(7, false),
                storage_entry(8, false),
            ],
        );

        let dip_compute = ComputeStage::new(
            gpu,
            "brush_dip",
            &shader_source(&EYEDROPPER_SNIPPETS, include_str!("shaders/brush_dip.wgsl")),
            &[
                uniform_entry(0, None),
                storage_entry(1, false),
                storage_entry(2, false),
                storage_entry(3, false),
            ],
        );

        WetOilEngine {
            params: SimParams::default(),
            tip_radius: DEFAULT_MAX_DIAMETER_PX / 2.0,
            loaded_thickness_um: 0.0,
            tip: storage_buffer(gpu, "brush_tip", TIP_BYTES),
            tip_delta: storage_buffer(gpu, "brush_tip_delta", TIP_BYTES),
            body: storage_buffer(gpu, "brush_body", BODY_BYTES),
            wanted: storage_buffer(gpu, "brush_refill_wanted", WANTED_BYTES),
            capillary_rows: storage_buffer(gpu, "brush_capillary_rows", CAPILLARY_BYTES),
            contact_reference: storage_buffer(
                gpu,
                "dab_contact_reference",
                CONTACT_REFERENCE_BYTES,
            ),
            picked: storage_buffer(gpu, "eyedropper_picked", PICKED_BYTES),
            tip_sums: storage_buffer(gpu, "brush_tip_sums", TIP_SUMS_BYTES),
            blade_load: storage_buffer(gpu, "knife_blade_load", BLADE_LOAD_BYTES),
            blade_delta: storage_buffer(gpu, "knife_blade_delta", BLADE_DELTA_BYTES),
            blade_grant: storage_buffer(gpu, "knife_blade_grant", BLADE_LOAD_BYTES),
            blade_state: storage_buffer(gpu, "knife_blade_state", BLADE_STATE_BYTES),
            rest_pending: true,
            previous_dab: None,
            blade_pipelines: BladePipelines::new(gpu, &blade_compute),
            reference_pipeline: exchange_compute.pipeline(gpu, "dab_reference", "reference"),
            exchange_pipeline: exchange_compute.pipeline(gpu, "dab_exchange", "main"),
            apply_pipeline: apply_compute.pipeline(gpu, "tip_apply", "main"),
            gather_pipeline: refill_compute.pipeline(gpu, "tip_refill_gather", "gather"),
            settle_pipeline: refill_compute.pipeline(gpu, "tip_refill_settle", "settle"),
            distribute_pipeline: refill_compute.pipeline(
                gpu,
                "tip_refill_distribute",
                "distribute",
            ),
            capillary_pipelines: CapillaryPipelines {
                gather: capillary_compute.pipeline(gpu, "tip_capillary_gather", "gather"),
                settle: capillary_compute.pipeline(gpu, "tip_capillary_settle", "settle"),
                distribute: capillary_compute.pipeline(
                    gpu,
                    "tip_capillary_distribute",
                    "distribute",
                ),
            },
            copy_pipeline: copy_compute.pipeline(gpu, "region_copy", "main"),
            advect_pipeline: advect_compute.pipeline(gpu, "advect", "main"),
            rag_pipeline: rag_compute.pipeline(gpu, "rag", "main"),
            pick_pipeline: pick_compute.pipeline(gpu, "eyedropper", "sample"),
            dry_pipeline: dry_compute.pipeline(gpu, "dry", "main"),
            fill_body_pipeline: dip_compute.pipeline(gpu, "brush_fill_body", "fill_body"),
            fill_tip_pipeline: dip_compute.pipeline(gpu, "brush_fill_tip", "fill_tip"),
            sum_tip_pipeline: dip_compute.pipeline(gpu, "brush_sum_tip", "sum_tip"),
            blade_compute,
            exchange_compute,
            apply_compute,
            refill_compute,
            copy_compute,
            advect_compute,
            rag_compute,
            pick_compute,
            dry_compute,
            dip_compute,
            scratch: None,
        }
    }

    pub fn resize_tip(&mut self, radius_px: f32) {
        if radius_px.is_finite() {
            self.tip_radius = radius_px.clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS);
        }
    }

    pub fn load_reservoir(&mut self, gpu: &GpuContext, mixture: &Mixture) {
        self.clean_brush(gpu);
        self.dip(gpu, mixture, FULL_DIP);
    }

    pub fn dip(&mut self, gpu: &GpuContext, mixture: &Mixture, amount: f32) {
        let Some(thickness) = dip_thickness(mixture) else {
            return;
        };
        if !amount.is_finite() || amount <= 0.0 {
            return;
        }
        let record = DipRecord {
            fractions: dip_fractions(mixture),
            tip_radius: self.tip_radius,
            tip_add_um: amount * thickness,
            tip_limit_um: MAX_TIP_THICKNESS_UM,
            body_add_um: amount * self.reserve_volume(thickness),
            body_limit_um: self.reserve_volume(MAX_TIP_THICKNESS_UM),
            _pad: [0.0; 3],
        };
        self.submit_fill(gpu, &record, true);
        self.loaded_thickness_um = thickness;
    }

    pub fn restore_reserve(&mut self, gpu: &GpuContext, mixture: &Mixture) {
        let Some(thickness) = dip_thickness(mixture) else {
            return;
        };
        let full_reserve = self.reserve_volume(thickness);
        let record = DipRecord {
            fractions: dip_fractions(mixture),
            tip_radius: self.tip_radius,
            tip_add_um: 0.0,
            tip_limit_um: 0.0,
            body_add_um: full_reserve,
            body_limit_um: full_reserve,
            _pad: [0.0; 3],
        };
        self.submit_fill(gpu, &record, false);
        self.loaded_thickness_um = thickness;
    }

    pub(crate) fn draw_up(
        &mut self,
        gpu: &GpuContext,
        into_knife: bool,
        take: impl FnOnce(f32) -> Result<PaintTotals, GpuLost>,
    ) -> Result<(PaintTotals, f32), GpuLost> {
        if into_knife {
            self.draw_up_into_knife(gpu, take)
        } else {
            self.draw_up_into_reserve(gpu, take)
        }
    }

    #[expect(clippy::excessive_nesting)]
    fn draw_up_into_reserve(
        &mut self,
        gpu: &GpuContext,
        take: impl FnOnce(f32) -> Result<PaintTotals, GpuLost>,
    ) -> Result<(PaintTotals, f32), GpuLost> {
        let held = self.reserve_totals(gpu)?.volume as f32;
        let capacity = self.full_reserve_volume();
        let taken = take(capacity - held)?;
        if holds_paint(&taken) {
            self.add_to_reserve(gpu, &taken);
            if held <= 0.0 {
                self.loaded_thickness_um = self.loaded_thickness_um.max(DRAWN_LOADED_THICKNESS_UM);
            }
        }
        Ok((taken, level_after(held, capacity, &taken)))
    }

    fn add_to_reserve(&self, gpu: &GpuContext, taken: &PaintTotals) {
        let record = DipRecord {
            fractions: std::array::from_fn(|slot| (taken.pigment[slot] / taken.volume) as f32),
            tip_radius: self.tip_radius,
            tip_add_um: 0.0,
            tip_limit_um: 0.0,
            body_add_um: taken.volume as f32,
            body_limit_um: f32::MAX,
            _pad: [0.0; 3],
        };
        self.submit_fill(gpu, &record, false);
    }

    fn draw_up_into_knife(
        &mut self,
        gpu: &GpuContext,
        take: impl FnOnce(f32) -> Result<PaintTotals, GpuLost>,
    ) -> Result<(PaintTotals, f32), GpuLost> {
        let mut load = read_buffer(gpu, &self.blade_load)?;
        let shares = blade_shares(self.tip_radius);
        let held = knife_from_load(&load).volume as f32;
        let full_dip = self.knife_full_dip_volume(FULL_RESERVOIR_VOLUME_UM);
        let room = (full_dip - held).min(blade_room(&load, &shares)).max(0.0);
        let taken = take(room)?;
        if holds_paint(&taken) {
            add_to_blade(&mut load, &shares, &taken);
            gpu.queue
                .write_buffer(&self.blade_load, 0, bytemuck::cast_slice(&load));
        }
        Ok((taken, level_after(held, held + room, &taken)))
    }

    pub fn tip_mean_mixture(&self, gpu: &GpuContext) -> Result<Mixture, GpuLost> {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("brush_sum_tip"),
            });
        self.encode_sum_tip(gpu, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        Ok(tip_mean_from_sums(&read_buffer(gpu, &self.tip_sums)?))
    }

    pub fn reserve_totals(&self, gpu: &GpuContext) -> Result<PaintTotals, GpuLost> {
        Ok(reserve_from_body(&read_buffer(gpu, &self.body)?))
    }

    pub fn read_brush(&self, gpu: &GpuContext) -> Result<BrushReadout, GpuLost> {
        let sources = [&self.tip_sums, &self.body, &self.blade_load];
        let staging = staging_buffer(
            gpu,
            "brush_readout",
            sources.iter().map(|source| source.size()).sum(),
        );
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("brush_readout"),
            });
        self.encode_sum_tip(gpu, &mut encoder);
        let mut offset = 0;
        for source in sources {
            encoder.copy_buffer_to_buffer(source, 0, &staging, offset, source.size());
            offset += source.size();
        }
        gpu.queue.submit([encoder.finish()]);
        read_mapped(gpu, &staging, |bytes| {
            let values: &[f32] = bytemuck::cast_slice(bytes);
            let (sums, rest) = values.split_at(TIP_SUM_CHANNELS);
            let (body, load) = rest.split_at(BODY_BYTES as usize / std::mem::size_of::<f32>());
            BrushReadout {
                tip_mean: tip_mean_from_sums(sums),
                reserve: reserve_from_body(body),
                knife: knife_from_load(load),
            }
        })
    }

    fn encode_sum_tip(&self, gpu: &GpuContext, encoder: &mut wgpu::CommandEncoder) {
        let group = self.dip_bind_group(gpu, &DipRecord::measuring(self.tip_radius));
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("brush_sum_tip"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &group, &[]);
        pass.set_pipeline(&self.sum_tip_pipeline);
        pass.dispatch_workgroups(1, 1, 1);
    }

    pub fn full_reserve_volume(&self) -> f32 {
        self.reserve_volume(FULL_RESERVOIR_VOLUME_UM)
    }

    fn reserve_volume(&self, thickness_um: f32) -> f32 {
        self.params.get("reserve_factor") * thickness_um * disk_texels(self.tip_radius) as f32
    }

    fn submit_fill(&self, gpu: &GpuContext, record: &DipRecord, fills_tip: bool) {
        let group = self.dip_bind_group(gpu, record);
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("brush_dip"),
            });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("brush_dip"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &group, &[]);
        pass.set_pipeline(&self.fill_body_pipeline);
        pass.dispatch_workgroups(1, 1, 1);
        if fills_tip {
            pass.set_pipeline(&self.fill_tip_pipeline);
            pass.dispatch_workgroups(TIP_GROUPS, TIP_GROUPS, 1);
        }
        drop(pass);
        gpu.queue.submit([encoder.finish()]);
    }

    fn dip_bind_group(&self, gpu: &GpuContext, record: &DipRecord) -> wgpu::BindGroup {
        let uniform = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("brush_dip"),
                contents: bytemuck::bytes_of(record),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        self.dip_compute.bind_group(
            gpu,
            &[
                buffer_binding(0, &uniform),
                buffer_binding(1, &self.body),
                buffer_binding(2, &self.tip),
                buffer_binding(3, &self.tip_sums),
            ],
        )
    }

    pub fn clean_brush(&mut self, gpu: &GpuContext) {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clean_brush"),
            });
        encoder.clear_buffer(&self.tip, 0, None);
        encoder.clear_buffer(&self.tip_delta, 0, None);
        encoder.clear_buffer(&self.body, 0, None);
        encoder.clear_buffer(&self.wanted, 0, None);
        encoder.clear_buffer(&self.capillary_rows, 0, None);
        gpu.queue.submit([encoder.finish()]);
    }

    #[expect(clippy::excessive_nesting)]
    pub fn dip_knife(
        &mut self,
        gpu: &GpuContext,
        mixture: &Mixture,
        amount: f32,
    ) -> Result<(), GpuLost> {
        let Some(thickness) = dip_thickness(mixture) else {
            return Ok(());
        };
        if !amount.is_finite() || amount <= 0.0 {
            return Ok(());
        }
        let fractions = dip_fractions(mixture);
        let mut load = read_buffer(gpu, &self.blade_load)?;
        let reach = self.tip_radius.ceil() as i32 + 1;
        for offset in -reach..=reach {
            let cover = blade_cell_cover(offset, self.tip_radius);
            let cell = (TIP_HALF as i32 + offset).clamp(0, BLADE_CELLS as i32 - 1) as usize;
            let values = &mut load[cell * TIP_CHANNELS..(cell + 1) * TIP_CHANNELS];
            let room = (KNIFE_LAID_CAPACITY_UM - values[VOLUME_CHANNEL]).max(0.0);
            let added = (cover * amount * thickness * KNIFE_DIP_LENGTH_PX).min(room);
            for (slot, fraction) in fractions.iter().enumerate() {
                values[slot] += added * fraction;
            }
            values[VOLUME_CHANNEL] += added;
        }
        gpu.queue
            .write_buffer(&self.blade_load, 0, bytemuck::cast_slice(&load));
        Ok(())
    }

    pub fn knife_full_dip_volume(&self, thickness_um: f32) -> f32 {
        if !thickness_um.is_finite() || thickness_um <= 0.0 {
            return 0.0;
        }
        let thickness = thickness_um.min(MAX_TIP_THICKNESS_UM);
        let reach = self.tip_radius.ceil() as i32 + 1;
        (-reach..=reach)
            .map(|offset| {
                (blade_cell_cover(offset, self.tip_radius) * thickness * KNIFE_DIP_LENGTH_PX)
                    .min(KNIFE_LAID_CAPACITY_UM)
            })
            .sum()
    }

    pub fn clean_knife(&mut self, gpu: &GpuContext) {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clean_knife"),
            });
        encoder.clear_buffer(&self.blade_load, 0, None);
        encoder.clear_buffer(&self.blade_delta, 0, None);
        gpu.queue.submit([encoder.finish()]);
    }

    pub fn knife_totals(&self, gpu: &GpuContext) -> Result<PaintTotals, GpuLost> {
        Ok(knife_from_load(&read_buffer(gpu, &self.blade_load)?))
    }

    fn blade_bind_group(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        dabs: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        self.blade_compute.bind_group(
            gpu,
            &[
                dab_binding(0, dabs),
                texture_binding(1, canvas, StateTexture::WetPig0),
                texture_binding(2, canvas, StateTexture::WetPig1),
                texture_binding(3, canvas, StateTexture::WetVol),
                texture_binding(4, canvas, StateTexture::DryHeight),
                buffer_binding(5, &self.blade_load),
                buffer_binding(6, &self.blade_delta),
                buffer_binding(7, &self.blade_grant),
                buffer_binding(8, &self.blade_state),
            ],
        )
    }

    pub fn brush_totals(&self, gpu: &GpuContext) -> Result<PaintTotals, GpuLost> {
        let mut totals = PaintTotals::default();
        for texel in read_buffer(gpu, &self.tip)?.as_chunks::<TIP_CHANNELS>().0 {
            accumulate(&mut totals, texel);
        }
        accumulate(&mut totals, &read_buffer(gpu, &self.body)?[..TIP_CHANNELS]);
        Ok(totals)
    }

    #[expect(clippy::excessive_nesting)]
    pub fn reservoir_mixture(&self, gpu: &GpuContext) -> Result<Mixture, GpuLost> {
        let body = read_buffer(gpu, &self.body)?;
        let volume = body[VOLUME_CHANNEL];
        let fractions = std::array::from_fn(|slot| {
            if volume > 0.0 {
                body[slot] / volume
            } else {
                0.0
            }
        });
        Ok(Mixture {
            volume_um: self.loaded_thickness_um,
            fractions,
        })
    }

    #[expect(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn encode_eyedropper(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        x: f32,
        y: f32,
        radius: f32,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        encoder.clear_buffer(&self.picked, 0, None);
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        let radius = if radius.is_finite() {
            radius.clamp(MIN_PICK_RADIUS_PX, MAX_PICK_RADIUS_PX)
        } else {
            MIN_PICK_RADIUS_PX
        };
        let (width, height) = (canvas.spec.width as f32, canvas.spec.height as f32);
        if x + radius < 0.0 || y + radius < 0.0 || x - radius > width || y - radius > height {
            return;
        }
        let record = PickRecord {
            center: [x, y],
            radius,
            _pad: 0.0,
        };
        let pick = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("eyedropper"),
                contents: bytemuck::bytes_of(&record),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let group = self.pick_compute.bind_group(
            gpu,
            &[
                buffer_binding(0, &pick),
                texture_binding(1, canvas, StateTexture::WetPig0),
                texture_binding(2, canvas, StateTexture::WetPig1),
                texture_binding(3, canvas, StateTexture::WetVol),
                texture_binding(4, canvas, StateTexture::DryTopPig0),
                texture_binding(5, canvas, StateTexture::DryTopPig1),
                buffer_binding(6, &self.picked),
            ],
        );
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("eyedropper"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &group, &[]);
        pass.set_pipeline(&self.pick_pipeline);
        pass.dispatch_workgroups(1, 1, 1);
    }

    pub fn picked_mixture(&self, gpu: &GpuContext) -> Result<Option<Mixture>, GpuLost> {
        let picked = read_buffer(gpu, &self.picked)?;
        if picked[PICKED_PAINT_FOUND].to_bits() == 0 {
            return Ok(None);
        }
        Ok(Some(Mixture {
            volume_um: self.loaded_thickness_um,
            fractions: std::array::from_fn(|slot| picked[PICKED_FRACTIONS + slot]),
        }))
    }

    #[expect(clippy::too_many_lines)]
    pub fn encode_dry_tiles(
        &mut self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        tiles: &[TileCoord],
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if tiles.is_empty() {
            return;
        }
        let tile_list: Vec<[u32; 2]> = tiles.iter().map(|tile| [tile.x, tile.y]).collect();
        let tile_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("dry_tiles"),
                contents: bytemuck::cast_slice(&tile_list),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let group = self.dry_compute.bind_group(
            gpu,
            &[
                buffer_binding(0, canvas.slot_table()),
                buffer_binding(1, &tile_buffer),
                texture_binding(2, canvas, StateTexture::WetPig0),
                texture_binding(3, canvas, StateTexture::WetPig1),
                texture_binding(4, canvas, StateTexture::WetVol),
                texture_binding(5, canvas, StateTexture::DryRefl0),
                texture_binding(6, canvas, StateTexture::DryRefl1),
                texture_binding(7, canvas, StateTexture::DryHeight),
                texture_binding(8, canvas, StateTexture::DryTopPig0),
                texture_binding(9, canvas, StateTexture::DryTopPig1),
            ],
        );
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("dry"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.dry_pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(
            DRY_WORKGROUPS_PER_TILE_SIDE,
            DRY_WORKGROUPS_PER_TILE_SIDE,
            tile_list.len() as u32,
        );
        drop(pass);
        for tile in tiles {
            let x0 = (tile.x * TILE_SIZE) as f32;
            let y0 = (tile.y * TILE_SIZE) as f32;
            let last = (TILE_SIZE - 1) as f32;
            canvas.dirty.mark_rect(x0, y0, x0 + last, y0 + last);
        }
    }

    fn rag_bind_group(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        dabs: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        self.rag_compute.bind_group(
            gpu,
            &[
                dab_binding(0, dabs),
                texture_binding(1, canvas, StateTexture::WetVol),
            ],
        )
    }

    fn exchange_bind_group(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        dabs: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        self.exchange_compute.bind_group(
            gpu,
            &[
                dab_binding(0, dabs),
                texture_binding(1, canvas, StateTexture::WetPig0),
                texture_binding(2, canvas, StateTexture::WetPig1),
                texture_binding(3, canvas, StateTexture::WetVol),
                texture_binding(4, canvas, StateTexture::DryHeight),
                buffer_binding(5, &self.tip),
                buffer_binding(6, &self.tip_delta),
                buffer_binding(7, &self.contact_reference),
            ],
        )
    }

    fn fit_scratch(&mut self, gpu: &GpuContext, canvas: &Canvas) {
        let side = max_dab_region_side();
        let size = [canvas.spec.width.min(side), canvas.spec.height.min(side)];
        if self
            .scratch
            .as_ref()
            .is_none_or(|scratch| scratch.size != size)
        {
            self.scratch = Some(Scratch::new(gpu, size));
        }
    }

    fn spreading_bind_groups(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        dabs: &wgpu::Buffer,
    ) -> (wgpu::BindGroup, wgpu::BindGroup) {
        let scratch = self
            .scratch
            .as_ref()
            .expect("scratch surfaces are fitted before spreading");
        let scratch_views = [&scratch.pig0.view, &scratch.pig1.view, &scratch.vol.view];
        let motion_view = &scratch.motion.view;
        let wet_views = [
            canvas.view(StateTexture::WetPig0),
            canvas.view(StateTexture::WetPig1),
            canvas.view(StateTexture::WetVol),
        ];
        let group =
            |compute: &ComputeStage, from: [&wgpu::TextureView; 3], to: [&wgpu::TextureView; 3]| {
                let views = from.into_iter().chain(to).chain([motion_view]);
                let entries: Vec<wgpu::BindGroupEntry> = std::iter::once(dab_binding(0, dabs))
                    .chain(
                        (1..)
                            .zip(views)
                            .map(|(binding, view)| wgpu::BindGroupEntry {
                                binding,
                                resource: wgpu::BindingResource::TextureView(view),
                            }),
                    )
                    .collect();
                compute.bind_group(gpu, &entries)
            };
        (
            group(&self.copy_compute, wet_views, scratch_views),
            group(&self.advect_compute, scratch_views, wet_views),
        )
    }

    fn apply_bind_group(&self, gpu: &GpuContext, dabs: &wgpu::Buffer) -> wgpu::BindGroup {
        self.apply_compute.bind_group(
            gpu,
            &[
                dab_binding(0, dabs),
                buffer_binding(1, &self.tip),
                buffer_binding(2, &self.tip_delta),
            ],
        )
    }

    fn refill_bind_group(&self, gpu: &GpuContext, dabs: &wgpu::Buffer) -> wgpu::BindGroup {
        self.refill_compute.bind_group(
            gpu,
            &[
                dab_binding(0, dabs),
                buffer_binding(1, &self.tip),
                buffer_binding(2, &self.tip_delta),
                buffer_binding(3, &self.body),
                buffer_binding(4, &self.wanted),
                buffer_binding(5, &self.capillary_rows),
            ],
        )
    }
}

fn blade_shares(half_length: f32) -> Vec<(usize, f32)> {
    let reach = half_length.ceil() as i32 + 1;
    let covers: Vec<(usize, f32)> = (-reach..=reach)
        .map(|offset| {
            let cell = (TIP_HALF as i32 + offset).clamp(0, BLADE_CELLS as i32 - 1) as usize;
            (cell, blade_cell_cover(offset, half_length))
        })
        .filter(|(_, cover)| *cover > 0.0)
        .collect();
    let total_cover: f32 = covers.iter().map(|(_, cover)| cover).sum();
    covers
        .into_iter()
        .map(|(cell, cover)| (cell, cover / total_cover))
        .collect()
}

fn blade_room(load: &[f32], shares: &[(usize, f32)]) -> f32 {
    shares
        .iter()
        .map(|&(cell, share)| {
            (KNIFE_LAID_CAPACITY_UM - load[cell * TIP_CHANNELS + VOLUME_CHANNEL]).max(0.0) / share
        })
        .fold(f32::INFINITY, f32::min)
}

fn add_to_blade(load: &mut [f32], shares: &[(usize, f32)], taken: &PaintTotals) {
    for &(cell, share) in shares {
        let share = f64::from(share);
        let values = &mut load[cell * TIP_CHANNELS..(cell + 1) * TIP_CHANNELS];
        for (slot, amount) in taken.pigment.iter().enumerate() {
            values[slot] += (amount * share) as f32;
        }
        values[VOLUME_CHANNEL] += (taken.volume * share) as f32;
    }
}

fn level_after(held_um: f32, capacity_um: f32, taken: &PaintTotals) -> f32 {
    if capacity_um > 0.0 {
        ((held_um + taken.volume as f32) / capacity_um).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn holds_paint(totals: &PaintTotals) -> bool {
    totals.volume.is_finite() && totals.volume > 0.0
}

fn dip_thickness(mixture: &Mixture) -> Option<f32> {
    let thickness = mixture.volume_um;
    (thickness.is_finite() && thickness > 0.0).then(|| thickness.min(MAX_TIP_THICKNESS_UM))
}

fn dip_fractions(mixture: &Mixture) -> [f32; PIGMENT_CHANNELS] {
    mixture.fractions.map(|fraction| {
        if fraction.is_finite() {
            fraction.clamp(0.0, 1.0)
        } else {
            0.0
        }
    })
}

fn disk_texels(radius: f32) -> u32 {
    let radius_squared = radius * radius;
    let offsets = -(TIP_HALF as i32)..(TIP_SIZE - TIP_HALF) as i32;
    offsets
        .clone()
        .map(|y| {
            offsets
                .clone()
                .filter(|&x| {
                    let (x, y) = (x as f32, y as f32);
                    x * x + y * y <= radius_squared
                })
                .count() as u32
        })
        .sum()
}

fn tip_mean_from_sums(sums: &[f32]) -> Mixture {
    let volume = sums[VOLUME_CHANNEL];
    let texels = sums[TIP_TEXEL_COUNT_SUM];
    if volume < MIN_TIP_MEAN_VOLUME_UM || texels <= 0.0 {
        return Mixture {
            volume_um: 0.0,
            fractions: [0.0; PIGMENT_CHANNELS],
        };
    }
    Mixture {
        volume_um: volume / texels,
        fractions: std::array::from_fn(|slot| sums[slot] / volume),
    }
}

fn reserve_from_body(body: &[f32]) -> PaintTotals {
    let mut totals = PaintTotals::default();
    accumulate(&mut totals, &body[..TIP_CHANNELS]);
    totals
}

fn knife_from_load(load: &[f32]) -> PaintTotals {
    let mut totals = PaintTotals::default();
    for cell in load.as_chunks::<TIP_CHANNELS>().0 {
        accumulate(&mut totals, cell);
    }
    totals
}

fn accumulate(totals: &mut PaintTotals, amounts: &[f32]) {
    for (slot, amount) in amounts[..PIGMENT_CHANNELS].iter().enumerate() {
        totals.pigment[slot] += f64::from(*amount);
    }
    totals.volume += f64::from(amounts[VOLUME_CHANNEL]);
}

fn dab_binding(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer,
            offset: 0,
            size: wgpu::BufferSize::new(DAB_RECORD_SIZE),
        }),
    }
}

impl PaintEngine for WetOilEngine {
    fn begin_stroke(&mut self) {
        self.rest_pending = true;
        self.previous_dab = None;
    }

    #[expect(
        clippy::cognitive_complexity,
        clippy::excessive_nesting,
        clippy::too_many_lines
    )]
    fn encode_dabs(
        &mut self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        brush: &BrushSettings,
        dabs: &[Dab],
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let mut placements = self.place_all(canvas, brush, dabs);
        if placements.is_empty() {
            return;
        }
        self.take_first_rest(&mut placements);
        let dab_buffer = upload_dab_records(gpu, &placements);
        let exchange_group = self.exchange_bind_group(gpu, canvas, &dab_buffer);
        let apply_group = self.apply_bind_group(gpu, &dab_buffer);
        let refill_group = self.refill_bind_group(gpu, &dab_buffer);
        let rag_group = self.rag_bind_group(gpu, canvas, &dab_buffer);
        let blade_group = self.blade_bind_group(gpu, canvas, &dab_buffer);
        self.fit_scratch(gpu, canvas);
        debug_assert!(placements.iter().all(|placement| {
            let scratch_size = self.scratch.as_ref().map_or([0; 2], |scratch| scratch.size);
            placement.fits_within(scratch_size)
        }));
        let (copy_group, advect_group) = self.spreading_bind_groups(gpu, canvas, &dab_buffer);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("dab_exchange"),
            timestamp_writes: None,
        });
        for (index, placement) in placements.iter().enumerate() {
            let offset = (index as u64 * DAB_RECORD_STRIDE) as wgpu::DynamicOffset;
            if placement.profile.wipes {
                pass.set_bind_group(0, &rag_group, &[offset]);
                pass.set_pipeline(&self.rag_pipeline);
                pass.dispatch_workgroups(placement.canvas_groups[0], placement.canvas_groups[1], 1);
                continue;
            }
            if placement.profile.blades {
                pass.set_bind_group(0, &copy_group, &[offset]);
                pass.set_pipeline(&self.copy_pipeline);
                pass.dispatch_workgroups(placement.canvas_groups[0], placement.canvas_groups[1], 1);
                pass.set_bind_group(0, &advect_group, &[offset]);
                pass.set_pipeline(&self.advect_pipeline);
                pass.dispatch_workgroups(placement.spread_groups[0], placement.spread_groups[1], 1);
                let blade = &self.blade_pipelines;
                let [width, height] = placement.canvas_groups;
                pass.set_bind_group(0, &blade_group, &[offset]);
                for (pipeline, groups) in [
                    (&blade.rest, [1, 1]),
                    (&blade.want, [width, height]),
                    (&blade.grant, [BLADE_CELL_GROUPS, 1]),
                    (&blade.lay, [width, height]),
                    (&blade.settle, [1, 1]),
                    (&blade.spill, [width, height]),
                ] {
                    pass.set_pipeline(pipeline);
                    pass.dispatch_workgroups(groups[0], groups[1], 1);
                }
                continue;
            }
            pass.set_bind_group(0, &exchange_group, &[offset]);
            pass.set_pipeline(&self.reference_pipeline);
            pass.dispatch_workgroups(1, 1, 1);
            pass.set_pipeline(&self.exchange_pipeline);
            pass.dispatch_workgroups(placement.canvas_groups[0], placement.canvas_groups[1], 1);
            pass.set_bind_group(0, &apply_group, &[offset]);
            pass.set_pipeline(&self.apply_pipeline);
            pass.dispatch_workgroups(placement.tip_groups[0], placement.tip_groups[1], 1);
            pass.set_bind_group(0, &refill_group, &[offset]);
            let capillary = &self.capillary_pipelines;
            for (pipeline, groups) in [
                (&capillary.gather, placement.tip_groups),
                (&capillary.settle, [CAPILLARY_POOL_GROUPS, 1]),
                (&capillary.distribute, placement.tip_groups),
            ] {
                pass.set_pipeline(pipeline);
                pass.dispatch_workgroups(groups[0], groups[1], 1);
            }
            if placement.profile.refills {
                pass.set_pipeline(&self.gather_pipeline);
                pass.dispatch_workgroups(placement.tip_groups[0], placement.tip_groups[1], 1);
                pass.set_pipeline(&self.settle_pipeline);
                pass.dispatch_workgroups(1, 1, 1);
                pass.set_pipeline(&self.distribute_pipeline);
                pass.dispatch_workgroups(placement.tip_groups[0], placement.tip_groups[1], 1);
            }
            if placement.profile.spreads {
                pass.set_bind_group(0, &copy_group, &[offset]);
                pass.set_pipeline(&self.copy_pipeline);
                pass.dispatch_workgroups(placement.canvas_groups[0], placement.canvas_groups[1], 1);
                pass.set_bind_group(0, &advect_group, &[offset]);
                pass.set_pipeline(&self.advect_pipeline);
                pass.dispatch_workgroups(placement.spread_groups[0], placement.spread_groups[1], 1);
            }
        }
        drop(pass);

        mark_dab_regions(canvas, &placements);
    }

    fn params(&self) -> &SimParams {
        &self.params
    }

    fn params_mut(&mut self) -> &mut SimParams {
        &mut self.params
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_nearly_full_blade_cell_limits_what_the_knife_draws_up() {
        let shares = blade_shares(4.0);
        let mut load = vec![0.0; BLADE_LOAD_VALUES];
        let empty_room = blade_room(&load, &shares);
        let (crowded, _) = shares[shares.len() / 2];
        load[crowded * TIP_CHANNELS + VOLUME_CHANNEL] = KNIFE_LAID_CAPACITY_UM - 10.0;
        let room = blade_room(&load, &shares);
        assert!(room < empty_room);
        let taken = PaintTotals {
            volume: f64::from(room),
            ..PaintTotals::default()
        };
        add_to_blade(&mut load, &shares, &taken);
        let fullest = load
            .chunks(TIP_CHANNELS)
            .map(|cell| cell[VOLUME_CHANNEL])
            .fold(0.0, f32::max);
        assert!(fullest <= KNIFE_LAID_CAPACITY_UM * (1.0 + 1e-6));
        assert!(
            (load[crowded * TIP_CHANNELS + VOLUME_CHANNEL] - KNIFE_LAID_CAPACITY_UM).abs() < 1e-2
        );
    }
}
