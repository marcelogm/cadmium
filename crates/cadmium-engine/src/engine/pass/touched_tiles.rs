use super::bristle_contact::ContactPass;
use super::{DabResources, PlacedDab, bind_at_dab, dab_record_binding, dab_record_entry, dispatch};
use crate::compute::{ComputeStage, buffer_binding, storage_entry};
use crate::engine::shader::{TOUCHED_TILE_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

pub(super) struct TouchedTilesPass {
    compute: ComputeStage,
    pipeline: wgpu::ComputePipeline,
}

impl TouchedTilesPass {
    pub(super) fn new(gpu: &GpuContext) -> TouchedTilesPass {
        let compute = ComputeStage::new(
            gpu,
            "touched_tiles",
            &shader_source(
                &TOUCHED_TILE_SNIPPETS,
                include_str!("../../shaders/touched_tiles.wgsl"),
            ),
            &[
                dab_record_entry(),
                storage_entry(1, true),
                storage_entry(2, false),
            ],
        );
        TouchedTilesPass {
            pipeline: compute.pipeline(gpu, "touched_tiles", "main"),
            compute,
        }
    }

    pub(super) fn bind_group(
        &self,
        resources: &DabResources<'_>,
        contact: &ContactPass,
    ) -> wgpu::BindGroup {
        self.compute.bind_group(
            resources.gpu,
            &[
                dab_record_binding(resources.dab_records),
                buffer_binding(1, &contact.mask),
                buffer_binding(2, resources.canvas.tile_changes().buffer()),
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
