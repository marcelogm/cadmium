use super::{PlacedDab, bind_at_dab, dispatch};
use crate::compute::ComputeStage;
use crate::engine::CAPILLARY_POOLS;
use crate::engine::shader::{DAB_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

const CAPILLARY_POOL_GROUP_SIZE: u32 = 64;
const CAPILLARY_POOL_GROUPS: u32 = (CAPILLARY_POOLS as u32).div_ceil(CAPILLARY_POOL_GROUP_SIZE);

pub(super) struct CapillaryPass {
    wick_give: wgpu::ComputePipeline,
    wick_take: wgpu::ComputePipeline,
    wick_clear: wgpu::ComputePipeline,
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
            wick_give: compute.pipeline(gpu, "tip_capillary_wick_give", "wick_give"),
            wick_take: compute.pipeline(gpu, "tip_capillary_wick_take", "wick_take"),
            wick_clear: compute.pipeline(gpu, "tip_capillary_wick_clear", "wick_clear"),
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
        bind_at_dab(pass, bind_group, dab);
        for (pipeline, groups) in [
            (&self.wick_give, dab.placement.tip_groups),
            (&self.wick_take, dab.placement.tip_groups),
            (&self.wick_clear, dab.placement.tip_groups),
            (&self.gather, dab.placement.tip_groups),
            (&self.settle, [CAPILLARY_POOL_GROUPS, 1]),
            (&self.distribute, dab.placement.tip_groups),
        ] {
            dispatch(pass, pipeline, groups);
        }
    }
}
