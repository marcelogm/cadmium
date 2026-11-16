use super::{DabResources, PlacedDab, bind_at_dab, dab_record_binding, dab_record_entry, dispatch};
use crate::canvas::StateTexture;
use crate::compute::{
    ComputeStage, buffer_binding, storage_entry, texture_binding, texture_entry, uniform_entry,
};
use crate::engine::bristles::{BRISTLE_SOLVE_WORKGROUP_SIZE, MAX_BRISTLES};
use crate::engine::shader::{BRISTLE_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

const SINGLE_WORKGROUP: [u32; 2] = [1, 1];
const SOLVE_WORKGROUPS: [u32; 2] = [
    MAX_BRISTLES.div_ceil(BRISTLE_SOLVE_WORKGROUP_SIZE as usize) as u32,
    1,
];
const FIRST_RECORD_OFFSET: wgpu::DynamicOffset = 0;

pub(super) struct BristleStepPass {
    compute: ComputeStage,
    touch: wgpu::ComputePipeline,
    solve: wgpu::ComputePipeline,
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
                uniform_entry(4, None),
                texture_entry(5, ReadOnly, R32Float),
                texture_entry(6, ReadOnly, R32Float),
                storage_entry(7, false),
                storage_entry(8, false),
            ],
        );
        BristleStepPass {
            touch: compute.pipeline(gpu, "bristle_touch", "touch"),
            solve: compute.pipeline(gpu, "bristle_solve", "solve"),
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
                buffer_binding(4, &bristles.tuning),
                texture_binding(5, canvas, StateTexture::DryHeight),
                texture_binding(6, canvas, StateTexture::WetVol),
                buffer_binding(7, &bristles.hair_loads),
                buffer_binding(8, &bristles.normal_forces),
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
        self.dispatch_both(pass);
    }

    pub(super) fn encode_first(
        &self,
        pass: &mut wgpu::ComputePass<'_>,
        bind_group: &wgpu::BindGroup,
    ) {
        pass.set_bind_group(0, bind_group, &[FIRST_RECORD_OFFSET]);
        self.dispatch_both(pass);
    }

    /// The touch height and the dab's path first, then one thread per strand.
    fn dispatch_both(&self, pass: &mut wgpu::ComputePass<'_>) {
        dispatch(pass, &self.touch, SINGLE_WORKGROUP);
        dispatch(pass, &self.solve, SOLVE_WORKGROUPS);
    }
}
