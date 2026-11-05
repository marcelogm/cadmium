use super::{DabResources, PlacedDab, bind_at_dab, dab_record_binding, dab_record_entry, dispatch};
use crate::canvas::StateTexture;
use crate::compute::{
    ComputeStage, buffer_binding, storage_entry, texture_binding, texture_entry, uniform_entry,
};
use crate::engine::shader::{BRISTLE_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

const SINGLE_WORKGROUP: [u32; 2] = [1, 1];
const FIRST_RECORD_OFFSET: wgpu::DynamicOffset = 0;

pub(super) struct BristleStepPass {
    compute: ComputeStage,
    step: wgpu::ComputePipeline,
}

impl BristleStepPass {
    pub(super) fn new(gpu: &GpuContext) -> BristleStepPass {
        use wgpu::StorageTextureAccess::ReadOnly;
        use wgpu::TextureFormat::R32Float;

        let compute = ComputeStage::new(
            gpu,
            "bristle_step",
            &shader_source(
                &BRISTLE_SNIPPETS,
                include_str!("../../shaders/bristle_step.wgsl"),
            ),
            &[
                dab_record_entry(),
                storage_entry(1, true),
                storage_entry(2, false),
                storage_entry(3, false),
                storage_entry(4, true),
                storage_entry(5, true),
                uniform_entry(6, None),
                texture_entry(7, ReadOnly, R32Float),
                texture_entry(8, ReadOnly, R32Float),
                storage_entry(9, false),
            ],
        );
        BristleStepPass {
            step: compute.pipeline(gpu, "bristle_step", "step"),
            compute,
        }
    }

    pub(super) fn bind_group(&self, resources: &DabResources<'_>) -> wgpu::BindGroup {
        let bristles = &resources.buffers.bristles;
        let canvas = resources.canvas;
        self.compute.bind_group(
            resources.gpu,
            &[
                dab_record_binding(resources.dab_records),
                buffer_binding(1, &bristles.rests),
                buffer_binding(2, &bristles.states),
                buffer_binding(3, &bristles.handle),
                buffer_binding(4, &resources.buffers.tip),
                buffer_binding(5, &bristles.clusters),
                buffer_binding(6, &bristles.tuning),
                texture_binding(7, canvas, StateTexture::DryHeight),
                texture_binding(8, canvas, StateTexture::WetVol),
                buffer_binding(9, &bristles.hair_loads),
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
        dispatch(pass, &self.step, SINGLE_WORKGROUP);
    }

    pub(super) fn encode_first(
        &self,
        pass: &mut wgpu::ComputePass<'_>,
        bind_group: &wgpu::BindGroup,
    ) {
        pass.set_bind_group(0, bind_group, &[FIRST_RECORD_OFFSET]);
        dispatch(pass, &self.step, SINGLE_WORKGROUP);
    }
}
