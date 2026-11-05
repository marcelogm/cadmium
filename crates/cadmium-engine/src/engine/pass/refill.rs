use super::{DabResources, PlacedDab, bind_at_dab, dab_record_binding, dab_record_entry, dispatch};
use crate::compute::{ComputeStage, buffer_binding, storage_entry};
use crate::engine::shader::{DAB_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

pub(super) struct RefillPass {
    pub(super) compute: ComputeStage,
    gather: wgpu::ComputePipeline,
    settle: wgpu::ComputePipeline,
    distribute: wgpu::ComputePipeline,
}

impl RefillPass {
    pub(super) fn new(gpu: &GpuContext) -> RefillPass {
        let compute = ComputeStage::new(
            gpu,
            "tip_refill",
            &shader_source(&DAB_SNIPPETS, include_str!("../../shaders/tip_refill.wgsl")),
            &[
                dab_record_entry(),
                storage_entry(1, false),
                storage_entry(2, false),
                storage_entry(3, false),
                storage_entry(4, false),
                storage_entry(5, false),
                storage_entry(6, true),
                storage_entry(7, true),
            ],
        );
        RefillPass {
            gather: compute.pipeline(gpu, "tip_refill_gather", "gather"),
            settle: compute.pipeline(gpu, "tip_refill_settle", "settle"),
            distribute: compute.pipeline(gpu, "tip_refill_distribute", "distribute"),
            compute,
        }
    }

    pub(super) fn bind_group(&self, resources: &DabResources<'_>) -> wgpu::BindGroup {
        let buffers = resources.buffers;
        self.compute.bind_group(
            resources.gpu,
            &[
                dab_record_binding(resources.dab_records),
                buffer_binding(1, &buffers.tip),
                buffer_binding(2, &buffers.tip_delta),
                buffer_binding(3, &buffers.body),
                buffer_binding(4, &buffers.wanted),
                buffer_binding(5, &buffers.capillary_pools),
                buffer_binding(6, &buffers.bristles.owners),
                buffer_binding(7, &buffers.bristles.ranks),
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
        for (pipeline, groups) in [
            (&self.gather, dab.placement.tip_groups),
            (&self.settle, [1, 1]),
            (&self.distribute, dab.placement.tip_groups),
        ] {
            dispatch(pass, pipeline, groups);
        }
    }
}
