use super::{DabResources, PlacedDab, dab_binding, dispatch};
use crate::compute::{ComputeStage, buffer_binding, storage_entry, uniform_entry};
use crate::engine::dab_record::DAB_RECORD_SIZE;
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
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                storage_entry(1, false),
                storage_entry(2, false),
                storage_entry(3, false),
                storage_entry(4, false),
                storage_entry(5, false),
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
                dab_binding(0, resources.dab_records),
                buffer_binding(1, &buffers.tip),
                buffer_binding(2, &buffers.tip_delta),
                buffer_binding(3, &buffers.body),
                buffer_binding(4, &buffers.wanted),
                buffer_binding(5, &buffers.capillary_rows),
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
        dispatch(pass, &self.gather, dab.placement.tip_groups);
        dispatch(pass, &self.settle, [1, 1]);
        dispatch(pass, &self.distribute, dab.placement.tip_groups);
    }
}
