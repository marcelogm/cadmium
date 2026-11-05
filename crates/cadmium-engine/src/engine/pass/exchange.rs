use super::bristle_contact::ContactPass;
use super::{DabResources, PlacedDab, bind_at_dab, dab_record_binding, dab_record_entry, dispatch};
use crate::canvas::StateTexture;
use crate::compute::{
    ComputeStage, buffer_binding, storage_buffer, storage_entry, texture_binding, texture_entry,
};
use crate::engine::shader::{EXCHANGE_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

const CONTACT_REFERENCE_BYTES: u64 = std::mem::size_of::<f32>() as u64;

pub(super) struct ExchangePass {
    compute: ComputeStage,
    reference: wgpu::ComputePipeline,
    exchange: wgpu::ComputePipeline,
    contact_reference: wgpu::Buffer,
}

impl ExchangePass {
    pub(super) fn new(gpu: &GpuContext) -> ExchangePass {
        use wgpu::StorageTextureAccess::{ReadOnly, ReadWrite};
        use wgpu::TextureFormat::{R32Float, Rgba16Float};

        let compute = ComputeStage::new(
            gpu,
            "dab_exchange",
            &shader_source(
                &EXCHANGE_SNIPPETS,
                include_str!("../../shaders/dab_exchange.wgsl"),
            ),
            &[
                dab_record_entry(),
                texture_entry(1, ReadWrite, Rgba16Float),
                texture_entry(2, ReadWrite, Rgba16Float),
                texture_entry(3, ReadWrite, R32Float),
                texture_entry(4, ReadOnly, R32Float),
                storage_entry(5, true),
                storage_entry(6, false),
                storage_entry(7, false),
                storage_entry(8, true),
                storage_entry(9, true),
                storage_entry(10, true),
                storage_entry(11, true),
                storage_entry(12, true),
            ],
        );
        ExchangePass {
            reference: compute.pipeline(gpu, "dab_reference", "reference"),
            exchange: compute.pipeline(gpu, "dab_exchange", "main"),
            contact_reference: storage_buffer(
                gpu,
                "dab_contact_reference",
                CONTACT_REFERENCE_BYTES,
            ),
            compute,
        }
    }

    pub(super) fn bind_group(
        &self,
        resources: &DabResources<'_>,
        contact: &ContactPass,
    ) -> wgpu::BindGroup {
        let canvas = resources.canvas;
        self.compute.bind_group(
            resources.gpu,
            &[
                dab_record_binding(resources.dab_records),
                texture_binding(1, canvas, StateTexture::WetPig0),
                texture_binding(2, canvas, StateTexture::WetPig1),
                texture_binding(3, canvas, StateTexture::WetVol),
                texture_binding(4, canvas, StateTexture::DryHeight),
                buffer_binding(5, &resources.buffers.tip),
                buffer_binding(6, &resources.buffers.tip_delta),
                buffer_binding(7, &self.contact_reference),
                buffer_binding(8, &contact.mask),
                buffer_binding(9, &contact.loads),
                buffer_binding(10, &resources.buffers.bristles.cell_grids),
                buffer_binding(11, &resources.buffers.bristles.hair_loads),
                buffer_binding(12, &contact.extent),
            ],
        )
    }

    pub(super) fn contact_reference(&self) -> &wgpu::Buffer {
        &self.contact_reference
    }

    pub(super) fn encode(
        &self,
        pass: &mut wgpu::ComputePass<'_>,
        bind_group: &wgpu::BindGroup,
        dab: PlacedDab<'_>,
    ) {
        bind_at_dab(pass, bind_group, dab);
        dispatch(pass, &self.reference, [1, 1]);
        dispatch(pass, &self.exchange, dab.placement.canvas_groups);
    }
}
