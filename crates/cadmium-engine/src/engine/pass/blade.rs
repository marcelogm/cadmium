use super::{DabResources, PlacedDab, bind_at_dab, dab_record_binding, dab_record_entry, dispatch};
use crate::canvas::StateTexture;
use crate::compute::{
    ComputeStage, buffer_binding, storage_buffer, storage_entry, texture_binding, texture_entry,
};
use crate::engine::shader::{DAB_SNIPPETS, shader_source};
use crate::engine::{
    BLADE_CELL_GROUP_SIZE, BLADE_CELLS, BLADE_ENDS, BLADE_LOAD_BYTES, BLADE_SPILL_VALUES,
};
use crate::gpu::GpuContext;

const BLADE_STATE_VALUES: usize = 2 + BLADE_ENDS + BLADE_SPILL_VALUES;
const BLADE_STATE_BYTES: u64 = (BLADE_STATE_VALUES * std::mem::size_of::<f32>()) as u64;
const BLADE_CELL_GROUPS: u32 = (BLADE_CELLS as u32).div_ceil(BLADE_CELL_GROUP_SIZE);

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

pub(super) struct BladePass {
    compute: ComputeStage,
    pipelines: BladePipelines,
    blade_grant: wgpu::Buffer,
    blade_state: wgpu::Buffer,
}

impl BladePass {
    pub(super) fn new(gpu: &GpuContext) -> BladePass {
        use wgpu::StorageTextureAccess::{ReadOnly, ReadWrite};
        use wgpu::TextureFormat::{R32Float, Rgba16Float};

        let compute = ComputeStage::new(
            gpu,
            "blade",
            &shader_source(&DAB_SNIPPETS, include_str!("../../shaders/blade.wgsl")),
            &[
                dab_record_entry(),
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
        BladePass {
            pipelines: BladePipelines::new(gpu, &compute),
            blade_grant: storage_buffer(gpu, "knife_blade_grant", BLADE_LOAD_BYTES),
            blade_state: storage_buffer(gpu, "knife_blade_state", BLADE_STATE_BYTES),
            compute,
        }
    }

    pub(super) fn bind_group(&self, resources: &DabResources<'_>) -> wgpu::BindGroup {
        let canvas = resources.canvas;
        self.compute.bind_group(
            resources.gpu,
            &[
                dab_record_binding(resources.dab_records),
                texture_binding(1, canvas, StateTexture::WetPig0),
                texture_binding(2, canvas, StateTexture::WetPig1),
                texture_binding(3, canvas, StateTexture::WetVol),
                texture_binding(4, canvas, StateTexture::DryHeight),
                buffer_binding(5, &resources.buffers.blade_load),
                buffer_binding(6, &resources.buffers.blade_delta),
                buffer_binding(7, &self.blade_grant),
                buffer_binding(8, &self.blade_state),
            ],
        )
    }

    pub(super) fn encode(
        &self,
        pass: &mut wgpu::ComputePass<'_>,
        bind_group: &wgpu::BindGroup,
        dab: PlacedDab<'_>,
    ) {
        let blade = &self.pipelines;
        let [width, height] = dab.placement.canvas_groups;
        bind_at_dab(pass, bind_group, dab);
        for (pipeline, groups) in [
            (&blade.rest, [1, 1]),
            (&blade.want, [width, height]),
            (&blade.grant, [BLADE_CELL_GROUPS, 1]),
            (&blade.lay, [width, height]),
            (&blade.settle, [1, 1]),
            (&blade.spill, [width, height]),
        ] {
            dispatch(pass, pipeline, groups);
        }
    }
}
