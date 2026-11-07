use crate::compute::{AutoLayoutStage, buffer_binding};
use crate::engine::Buffers;
use crate::engine::shader::shader_source;
use crate::gpu::GpuContext;

/// Sums the face on the GPU so a readout copies back one value per channel
/// rather than the whole face.
pub(in crate::engine) struct FaceSumPass {
    stage: AutoLayoutStage,
}

impl FaceSumPass {
    pub(in crate::engine) fn new(gpu: &GpuContext) -> FaceSumPass {
        FaceSumPass {
            stage: AutoLayoutStage::new(
                gpu,
                "mixing_knife_face_sum",
                &shader_source(&[], include_str!("../../shaders/face_sum.wgsl")),
            ),
        }
    }

    pub(in crate::engine) fn encode(
        &self,
        gpu: &GpuContext,
        buffers: &Buffers,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let bind_group = self.stage.bind_group(
            gpu,
            &[
                buffer_binding(0, &buffers.face_load),
                buffer_binding(1, &buffers.face_sums),
            ],
        );
        self.stage.encode(encoder, &bind_group, [1, 1, 1]);
    }
}
