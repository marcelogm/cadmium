use super::{DabResources, PlacedDab, dab_binding, dispatch};
use crate::canvas::StateTexture;
use crate::compute::{ComputeStage, texture_binding, texture_entry, uniform_entry};
use crate::engine::dab_record::DAB_RECORD_SIZE;
use crate::engine::shader::{DAB_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

pub(super) struct WipePass {
    compute: ComputeStage,
    pipeline: wgpu::ComputePipeline,
}

impl WipePass {
    pub(super) fn new(gpu: &GpuContext) -> WipePass {
        let compute = ComputeStage::new(
            gpu,
            "rag",
            &shader_source(&DAB_SNIPPETS, include_str!("../../shaders/rag.wgsl")),
            &[
                uniform_entry(0, Some(DAB_RECORD_SIZE)),
                texture_entry(
                    1,
                    wgpu::StorageTextureAccess::ReadWrite,
                    wgpu::TextureFormat::R32Float,
                ),
            ],
        );
        WipePass {
            pipeline: compute.pipeline(gpu, "rag", "main"),
            compute,
        }
    }

    pub(super) fn bind_group(&self, resources: &DabResources<'_>) -> wgpu::BindGroup {
        self.compute.bind_group(
            resources.gpu,
            &[
                dab_binding(0, resources.dab_records),
                texture_binding(1, resources.canvas, StateTexture::WetVol),
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
        dispatch(pass, &self.pipeline, dab.placement.canvas_groups);
    }
}
