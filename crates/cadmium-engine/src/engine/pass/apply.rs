use super::{DabResources, PlacedDab, dab_binding, dispatch};
use crate::compute::{ComputeStage, buffer_binding, storage_entry, uniform_entry};
use crate::engine::dab_record::DAB_RECORD_SIZE;
use crate::engine::shader::{DAB_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

pub(super) struct ApplyPass {
    compute: ComputeStage,
    pipeline: wgpu::ComputePipeline,
}

impl ApplyPass {
    pub(super) fn new(gpu: &GpuContext) -> ApplyPass {
        let compute = ComputeStage::new(
            gpu,
            "tip_apply",
            &shader_source(&DAB_SNIPPETS, include_str!("../../shaders/tip_apply.wgsl")),
            &[
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                storage_entry(1, false),
                storage_entry(2, false),
            ],
        );
        ApplyPass {
            pipeline: compute.pipeline(gpu, "tip_apply", "main"),
            compute,
        }
    }

    pub(super) fn bind_group(&self, resources: &DabResources<'_>) -> wgpu::BindGroup {
        self.compute.bind_group(
            resources.gpu,
            &[
                dab_binding(0, resources.dab_records),
                buffer_binding(1, &resources.buffers.tip),
                buffer_binding(2, &resources.buffers.tip_delta),
            ],
        )
    }

    pub(super) fn encode(
        &self,
        pass: &mut wgpu::ComputePass<'_>,
        bind_group: &wgpu::BindGroup,
        dab: PlacedDab<'_>,
    ) {
        pass.set_bind_group(0, bind_group, &[dab.offset]);
        dispatch(pass, &self.pipeline, dab.placement.tip_groups);
    }
}
