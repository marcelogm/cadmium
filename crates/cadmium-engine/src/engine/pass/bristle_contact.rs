use super::{DabResources, PlacedDab, bind_at_dab, dab_record_binding, dab_record_entry, dispatch};
use crate::compute::{ComputeStage, buffer_binding, storage_buffer, storage_entry};
use crate::engine::TIP_TEXELS;
use crate::engine::bristles::{BRISTLE_VERTICES, CELL_GRID_SIDE, MAX_BRISTLES};
use crate::engine::placement::Placement;
use crate::engine::shader::{CONTACT_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

pub(crate) const CONTACT_WORKGROUP_SIZE: u32 = 64;
pub(crate) const CONTACT_EXTENT_SCALE: f32 = 256.0;
pub(crate) const CONTACT_EXTENT_LOW: usize = 0;
pub(crate) const CONTACT_EXTENT_HIGH: usize = 1;
pub(crate) const CONTACT_ALONG_LOW: usize = 2;
pub(crate) const CONTACT_ALONG_HIGH: usize = 3;
pub(crate) const CONTACT_BOX_FIRST: usize = 4;
pub(crate) const CONTACT_BOX_LAST: usize = 6;
pub(crate) const CONTACT_EXTENT_WORDS: usize = 8;
pub(crate) const BRISTLE_SEGMENTS: usize = BRISTLE_VERTICES - 1;
pub(crate) const SWEPT_VECTORS: usize = 2;
pub(crate) const CONTACT_BRISTLE_BITS: u32 = 16;
pub(crate) const CONTACT_BIN_BITS: u32 = 4;
const CONTACT_COVERAGE_BITS: u32 = u32::BITS - CONTACT_BRISTLE_BITS - 2 * CONTACT_BIN_BITS;
pub(crate) const CONTACT_LEVELS: u32 = (1 << CONTACT_COVERAGE_BITS) - 1;
const VECTOR_BYTES: u64 = 4 * std::mem::size_of::<f32>() as u64;
const SWEPT_BYTES: u64 = (MAX_BRISTLES * BRISTLE_SEGMENTS * SWEPT_VECTORS) as u64 * VECTOR_BYTES;
const WORD_BYTES: u64 = std::mem::size_of::<u32>() as u64;
const LOAD_BYTES: u64 = TIP_TEXELS as u64 * WORD_BYTES;
const EXTENT_BYTES: u64 = CONTACT_EXTENT_WORDS as u64 * WORD_BYTES;
const FIRST_MASK_WORDS: u64 = 64 * 64;
const RASTER_GROUPS: u32 = (MAX_BRISTLES * BRISTLE_SEGMENTS) as u32;
const _: () = assert!(MAX_BRISTLES <= 1 << CONTACT_BRISTLE_BITS);
const _: () = assert!(CELL_GRID_SIDE <= 1 << CONTACT_BIN_BITS);

pub(crate) struct ContactPass {
    compute: ComputeStage,
    clear_mask: wgpu::ComputePipeline,
    clear_loads: wgpu::ComputePipeline,
    rasterize: wgpu::ComputePipeline,
    census: wgpu::ComputePipeline,
    pub(crate) mask: wgpu::Buffer,
    summed_coverage: wgpu::Buffer,
    pub(crate) extent: wgpu::Buffer,
    mask_words: u64,
    pub(crate) loads: wgpu::Buffer,
    swept: wgpu::Buffer,
}

impl ContactPass {
    pub(super) fn new(gpu: &GpuContext) -> ContactPass {
        let compute = ComputeStage::new(
            gpu,
            "bristle_contact",
            &shader_source(
                &CONTACT_SNIPPETS,
                include_str!("../../shaders/bristle_contact.wgsl"),
            ),
            &[
                dab_record_entry(),
                storage_entry(1, true),
                storage_entry(2, true),
                storage_entry(3, false),
                storage_entry(4, false),
                storage_entry(5, false),
                storage_entry(6, true),
                storage_entry(7, false),
                storage_entry(8, false),
            ],
        );
        ContactPass {
            clear_mask: compute.pipeline(gpu, "contact_clear_mask", "clear_mask"),
            clear_loads: compute.pipeline(gpu, "contact_clear_loads", "clear_loads"),
            rasterize: compute.pipeline(gpu, "contact_rasterize", "rasterize"),
            census: compute.pipeline(gpu, "contact_census", "census"),
            compute,
            mask: storage_buffer(gpu, "contact_mask", FIRST_MASK_WORDS * WORD_BYTES),
            summed_coverage: storage_buffer(
                gpu,
                "contact_summed_coverage",
                FIRST_MASK_WORDS * WORD_BYTES,
            ),
            extent: storage_buffer(gpu, "contact_extent", EXTENT_BYTES),
            mask_words: FIRST_MASK_WORDS,
            loads: storage_buffer(gpu, "contact_texel_load", LOAD_BYTES),
            swept: storage_buffer(gpu, "contact_swept", SWEPT_BYTES),
        }
    }

    pub(super) fn fit(&mut self, gpu: &GpuContext, placements: &[Placement]) {
        let needed_mask = placements
            .iter()
            .map(|placement| mask_words(placement.record.canvas_size))
            .max()
            .unwrap_or(0);
        if needed_mask > self.mask_words {
            self.mask = storage_buffer(gpu, "contact_mask", needed_mask * WORD_BYTES);
            self.summed_coverage =
                storage_buffer(gpu, "contact_summed_coverage", needed_mask * WORD_BYTES);
            self.mask_words = needed_mask;
        }
    }

    pub(super) fn bind_group(&self, resources: &DabResources<'_>) -> wgpu::BindGroup {
        let bristles = &resources.buffers.bristles;
        self.compute.bind_group(
            resources.gpu,
            &[
                dab_record_binding(resources.dab_records),
                buffer_binding(1, &bristles.rests),
                buffer_binding(2, &bristles.states),
                buffer_binding(3, &self.mask),
                buffer_binding(4, &self.loads),
                buffer_binding(5, &self.swept),
                buffer_binding(6, &bristles.cell_grids),
                buffer_binding(7, &self.extent),
                buffer_binding(8, &self.summed_coverage),
            ],
        )
    }

    pub(super) fn encode(
        &self,
        pass: &mut wgpu::ComputePass<'_>,
        bind_group: &wgpu::BindGroup,
        dab: PlacedDab<'_>,
    ) {
        bind_at_dab(pass, bind_group, dab);
        let placement = dab.placement;
        for (pipeline, groups) in [
            (&self.clear_mask, placement.canvas_groups),
            (&self.clear_loads, placement.tip_groups),
            (&self.rasterize, [RASTER_GROUPS, 1]),
            (&self.census, placement.canvas_groups),
        ] {
            dispatch(pass, pipeline, groups);
        }
    }
}

fn mask_words([width, height]: [u32; 2]) -> u64 {
    u64::from(width) * u64::from(height)
}
