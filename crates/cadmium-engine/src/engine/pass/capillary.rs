use super::{PlacedDab, dispatch};
use crate::compute::ComputeStage;
use crate::engine::CAPILLARY_POOLS;
use crate::engine::shader::{DAB_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

const CAPILLARY_POOL_GROUP_SIZE: u32 = 64;
const CAPILLARY_POOL_GROUPS: u32 = (CAPILLARY_POOLS as u32).div_ceil(CAPILLARY_POOL_GROUP_SIZE);

pub(super) struct CapillaryPass {
    gather: wgpu::ComputePipeline,
    settle: wgpu::ComputePipeline,
    distribute: wgpu::ComputePipeline,
}

impl CapillaryPass {
    pub(super) fn new(gpu: &GpuContext, refill_compute: &ComputeStage) -> CapillaryPass {
        let compute = refill_compute.with_shader(
            gpu,
            "tip_capillary",
            &shader_source(
                &DAB_SNIPPETS,
                include_str!("../../shaders/tip_capillary.wgsl"),
            ),
        );
        CapillaryPass {
            gather: compute.pipeline(gpu, "tip_capillary_gather", "gather"),
            settle: compute.pipeline(gpu, "tip_capillary_settle", "settle"),
            distribute: compute.pipeline(gpu, "tip_capillary_distribute", "distribute"),
        }
    }

    pub(super) fn encode(
        &self,
        pass: &mut wgpu::ComputePass<'_>,
        bind_group: &wgpu::BindGroup,
        dab: PlacedDab<'_>,
    ) {
        pass.set_bind_group(0, bind_group, &[dab.offset]);
        for (pipeline, groups) in [
            (&self.gather, dab.placement.tip_groups),
            (&self.settle, [CAPILLARY_POOL_GROUPS, 1]),
            (&self.distribute, dab.placement.tip_groups),
        ] {
            dispatch(pass, pipeline, groups);
        }
    }
}
