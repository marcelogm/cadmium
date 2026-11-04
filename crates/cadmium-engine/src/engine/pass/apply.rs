use super::{DabResources, PlacedDab, bind_at_dab, dab_record_binding, dab_record_entry, dispatch};
use crate::compute::{ComputeStage, buffer_binding, storage_entry};
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
                dab_record_entry(),
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
                dab_record_binding(resources.dab_records),
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
        bind_at_dab(pass, bind_group, dab);
        dispatch(pass, &self.pipeline, dab.placement.tip_groups);
    }
}
