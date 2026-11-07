use super::{DabResources, PlacedDab, bind_at_dab, dab_record_binding, dab_record_entry, dispatch};
use crate::canvas::StateTexture;
use crate::compute::{ComputeStage, texture_binding, texture_entry};
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
                dab_record_entry(),
                texture_entry(
                    1,
                    wgpu::StorageTextureAccess::ReadWrite,
                    wgpu::TextureFormat::R32Float,
                ),
                texture_entry(
                    2,
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
                dab_record_binding(resources.dab_records),
                texture_binding(1, resources.canvas, StateTexture::WetVol),
                texture_binding(2, resources.canvas, StateTexture::Graphite),
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
        dispatch(pass, &self.pipeline, dab.placement.canvas_groups);
    }
}
