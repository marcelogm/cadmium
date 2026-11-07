use super::{DabResources, PlacedDab, bind_at_dab, dab_record_binding, dab_record_entry, dispatch};
use crate::canvas::StateTexture;
use crate::compute::{ComputeStage, buffer_binding, texture_binding, texture_entry, uniform_entry};
use crate::engine::shader::{DAB_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

const SURFACE_TEXTURES: [(u32, StateTexture, wgpu::StorageTextureAccess); 3] = [
    (
        1,
        StateTexture::Graphite,
        wgpu::StorageTextureAccess::ReadWrite,
    ),
    (
        2,
        StateTexture::DryHeight,
        wgpu::StorageTextureAccess::ReadOnly,
    ),
    (
        3,
        StateTexture::WetVol,
        wgpu::StorageTextureAccess::ReadOnly,
    ),
];
const LEAD_BINDING: u32 = 4;

pub(super) struct GraphitePass {
    compute: ComputeStage,
    pipeline: wgpu::ComputePipeline,
}

impl GraphitePass {
    pub(super) fn new(gpu: &GpuContext) -> GraphitePass {
        let mut entries = vec![dab_record_entry()];
        entries.extend(SURFACE_TEXTURES.map(|(binding, _, access)| {
            texture_entry(binding, access, wgpu::TextureFormat::R32Float)
        }));
        entries.push(uniform_entry(LEAD_BINDING, None));
        let compute = ComputeStage::new(
            gpu,
            "pencil",
            &shader_source(&DAB_SNIPPETS, include_str!("../../shaders/pencil.wgsl")),
            &entries,
        );
        GraphitePass {
            pipeline: compute.pipeline(gpu, "pencil", "main"),
            compute,
        }
    }

    pub(super) fn bind_group(&self, resources: &DabResources<'_>) -> wgpu::BindGroup {
        let mut entries = vec![dab_record_binding(resources.dab_records)];
        entries.extend(
            SURFACE_TEXTURES
                .map(|(binding, which, _)| texture_binding(binding, resources.canvas, which)),
        );
        entries.push(buffer_binding(LEAD_BINDING, &resources.buffers.pencil));
        self.compute.bind_group(resources.gpu, &entries)
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
