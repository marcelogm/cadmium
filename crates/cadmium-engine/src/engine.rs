use crate::brush::{BrushSettings, DEFAULT_MAX_DIAMETER_PX, FULL_RESERVOIR_VOLUME_UM};
use crate::canvas::{Canvas, StateTexture, TileCoord};
use crate::compute::{
    ComputeStage, buffer_binding, storage_buffer, storage_entry, texture_binding, texture_entry,
    uniform_entry,
};
use crate::gpu::{GpuContext, GpuLost};
use crate::params::SimParams;
use crate::pigments::{MAX_SLOTS, Mixture};
use crate::stroke::Dab;

mod dab_record;
mod footprint;
mod knife_load;
mod pass;
mod placement;
mod readout;
mod reservoir;
pub(crate) mod shader;

pub use dab_record::blade_gap_um;
pub use footprint::BrushOutline;
pub use readout::BrushReadout;

pub(crate) use pass::pick::PickRequest;

use dab_record::{DAB_RECORD_SIZE, DAB_RECORD_STRIDE, upload_dab_records};
use pass::dip::DipPass;
use pass::dry::DryPass;
use pass::pick::PickPass;
use placement::{mark_dab_regions, max_dab_region_side};
use shader::{DAB_SNIPPETS, shader_source};

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
const CAPILLARY_POOLS: u64 = TIP_SIZE as u64 + 1;
const CAPILLARY_BYTES: u64 = (3 * CAPILLARY_POOLS * TIP_CHANNELS as u64 + 2 * CAPILLARY_POOLS)
    * std::mem::size_of::<u32>() as u64;
const CAPILLARY_POOL_GROUP_SIZE: u32 = 64;
const CAPILLARY_POOL_GROUPS: u32 = (CAPILLARY_POOLS as u32).div_ceil(CAPILLARY_POOL_GROUP_SIZE);
const KNIFE_DIP_LENGTH_PX: f32 = 380.0;
const KNIFE_LAID_CAPACITY_UM: f32 = MAX_TIP_THICKNESS_UM * KNIFE_DIP_LENGTH_PX;
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
const DRY_WORKGROUP_SIZE: u32 = 8;

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

const WANTED_BYTES: u64 = 2 * std::mem::size_of::<u32>() as u64;
const CONTACT_REFERENCE_BYTES: u64 = std::mem::size_of::<f32>() as u64;

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

struct CapillaryPipelines {
    gather: wgpu::ComputePipeline,
    settle: wgpu::ComputePipeline,
    distribute: wgpu::ComputePipeline,
}

struct Buffers {
    tip: wgpu::Buffer,
    tip_delta: wgpu::Buffer,
    body: wgpu::Buffer,
    wanted: wgpu::Buffer,
    capillary_rows: wgpu::Buffer,
    tip_sums: wgpu::Buffer,
    blade_load: wgpu::Buffer,
    blade_delta: wgpu::Buffer,
}

impl Buffers {
    fn new(gpu: &GpuContext) -> Buffers {
        Buffers {
            tip: storage_buffer(gpu, "brush_tip", TIP_BYTES),
            tip_delta: storage_buffer(gpu, "brush_tip_delta", TIP_BYTES),
            body: storage_buffer(gpu, "brush_body", BODY_BYTES),
            wanted: storage_buffer(gpu, "brush_refill_wanted", WANTED_BYTES),
            capillary_rows: storage_buffer(gpu, "brush_capillary_rows", CAPILLARY_BYTES),
            tip_sums: storage_buffer(gpu, "brush_tip_sums", TIP_SUMS_BYTES),
            blade_load: storage_buffer(gpu, "knife_blade_load", BLADE_LOAD_BYTES),
            blade_delta: storage_buffer(gpu, "knife_blade_delta", BLADE_DELTA_BYTES),
        }
    }
}

pub struct WetOilEngine {
    params: SimParams,
    tip_radius: f32,
    loaded_thickness_um: f32,
    buffers: Buffers,
    contact_reference: wgpu::Buffer,
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
    pick_pass: PickPass,
    dry_pass: DryPass,
    dip_pass: DipPass,
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

        WetOilEngine {
            params: SimParams::default(),
            tip_radius: DEFAULT_MAX_DIAMETER_PX / 2.0,
            loaded_thickness_um: 0.0,
            buffers: Buffers::new(gpu),
            contact_reference: storage_buffer(
                gpu,
                "dab_contact_reference",
                CONTACT_REFERENCE_BYTES,
            ),
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
            pick_pass: PickPass::new(gpu),
            dry_pass: DryPass::new(gpu),
            dip_pass: DipPass::new(gpu),
            blade_compute,
            exchange_compute,
            apply_compute,
            refill_compute,
            copy_compute,
            advect_compute,
            rag_compute,
            scratch: None,
        }
    }

    pub fn resize_tip(&mut self, radius_px: f32) {
        if radius_px.is_finite() {
            self.tip_radius = radius_px.clamp(MIN_DAB_RADIUS, MAX_TIP_RADIUS);
        }
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
                buffer_binding(5, &self.buffers.blade_load),
                buffer_binding(6, &self.buffers.blade_delta),
                buffer_binding(7, &self.blade_grant),
                buffer_binding(8, &self.blade_state),
            ],
        )
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
                buffer_binding(5, &self.buffers.tip),
                buffer_binding(6, &self.buffers.tip_delta),
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
                buffer_binding(1, &self.buffers.tip),
                buffer_binding(2, &self.buffers.tip_delta),
            ],
        )
    }

    fn refill_bind_group(&self, gpu: &GpuContext, dabs: &wgpu::Buffer) -> wgpu::BindGroup {
        self.refill_compute.bind_group(
            gpu,
            &[
                dab_binding(0, dabs),
                buffer_binding(1, &self.buffers.tip),
                buffer_binding(2, &self.buffers.tip_delta),
                buffer_binding(3, &self.buffers.body),
                buffer_binding(4, &self.buffers.wanted),
                buffer_binding(5, &self.buffers.capillary_rows),
            ],
        )
    }
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
