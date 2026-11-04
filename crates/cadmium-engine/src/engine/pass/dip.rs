use wgpu::util::DeviceExt;

use super::dispatch;
use crate::compute::{ComputeStage, WORKGROUP_SIZE, buffer_binding, storage_entry, uniform_entry};
use crate::engine::reservoir::DipRecord;
use crate::engine::shader::{EYEDROPPER_SNIPPETS, shader_source};
use crate::engine::{Buffers, TIP_SIZE};
use crate::gpu::{GpuContext, new_encoder};

const TIP_GROUPS: u32 = TIP_SIZE.div_ceil(WORKGROUP_SIZE);

pub(in crate::engine) struct DipPass {
    compute: ComputeStage,
    fill_body: wgpu::ComputePipeline,
    fill_tip: wgpu::ComputePipeline,
    sum_tip: wgpu::ComputePipeline,
}

impl DipPass {
    pub(in crate::engine) fn new(gpu: &GpuContext) -> DipPass {
        let compute = ComputeStage::new(
            gpu,
            "brush_dip",
            &shader_source(
                &EYEDROPPER_SNIPPETS,
                include_str!("../../shaders/brush_dip.wgsl"),
            ),
            &[
                uniform_entry(0, None),
                storage_entry(1, false),
                storage_entry(2, false),
                storage_entry(3, false),
            ],
        );
        DipPass {
            fill_body: compute.pipeline(gpu, "brush_fill_body", "fill_body"),
            fill_tip: compute.pipeline(gpu, "brush_fill_tip", "fill_tip"),
            sum_tip: compute.pipeline(gpu, "brush_sum_tip", "sum_tip"),
            compute,
        }
    }

    pub(in crate::engine) fn submit_fill(
        &self,
        gpu: &GpuContext,
        record: &DipRecord,
        buffers: &Buffers,
        fills_tip: bool,
    ) {
        let bind_group = self.bind_group(gpu, record, buffers);
        let mut encoder = new_encoder(gpu, "brush_dip");
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("brush_dip"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &bind_group, &[]);
        dispatch(&mut pass, &self.fill_body, [1, 1]);
        if fills_tip {
            dispatch(&mut pass, &self.fill_tip, [TIP_GROUPS, TIP_GROUPS]);
        }
        drop(pass);
        gpu.queue.submit([encoder.finish()]);
    }

    pub(in crate::engine) fn encode_sum_tip(
        &self,
        gpu: &GpuContext,
        tip_radius: f32,
        buffers: &Buffers,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let bind_group = self.bind_group(gpu, &DipRecord::measuring(tip_radius), buffers);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("brush_sum_tip"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &bind_group, &[]);
        dispatch(&mut pass, &self.sum_tip, [1, 1]);
    }

    fn bind_group(
        &self,
        gpu: &GpuContext,
        record: &DipRecord,
        buffers: &Buffers,
    ) -> wgpu::BindGroup {
        let uniform = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("brush_dip"),
                contents: bytemuck::bytes_of(record),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        self.compute.bind_group(
            gpu,
            &[
                buffer_binding(0, &uniform),
                buffer_binding(1, &buffers.body),
                buffer_binding(2, &buffers.tip),
                buffer_binding(3, &buffers.tip_sums),
            ],
        )
    }
}
