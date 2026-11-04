use crate::canvas::{Canvas, StateTexture};
use crate::gpu::{GpuContext, GpuLost, read_mapped, staging_buffer};

pub(crate) const WORKGROUP_SIZE: u32 = 8;

pub(crate) struct ComputeStage {
    label: &'static str,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    module: wgpu::ShaderModule,
}

impl ComputeStage {
    pub(crate) fn new(
        gpu: &GpuContext,
        label: &'static str,
        source: &str,
        entries: &[wgpu::BindGroupLayoutEntry],
    ) -> ComputeStage {
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let module = shader_module(gpu, label, source);
        ComputeStage {
            label,
            layout,
            pipeline_layout,
            module,
        }
    }

    pub(crate) fn with_shader(&self, gpu: &GpuContext, label: &str, source: &str) -> ComputeStage {
        ComputeStage {
            label: self.label,
            layout: self.layout.clone(),
            pipeline_layout: self.pipeline_layout.clone(),
            module: shader_module(gpu, label, source),
        }
    }

    pub(crate) fn pipeline(
        &self,
        gpu: &GpuContext,
        label: &str,
        entry_point: &str,
    ) -> wgpu::ComputePipeline {
        compute_pipeline(
            gpu,
            label,
            &self.module,
            Some(&self.pipeline_layout),
            entry_point,
        )
    }

    pub(crate) fn bind_group(
        &self,
        gpu: &GpuContext,
        entries: &[wgpu::BindGroupEntry],
    ) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(self.label),
            layout: &self.layout,
            entries,
        })
    }
}

pub(crate) struct AutoLayoutStage {
    label: &'static str,
    pipeline: wgpu::ComputePipeline,
}

impl AutoLayoutStage {
    pub(crate) fn new(gpu: &GpuContext, label: &'static str, source: &str) -> AutoLayoutStage {
        let module = shader_module(gpu, label, source);
        AutoLayoutStage {
            label,
            pipeline: compute_pipeline(gpu, label, &module, None, "main"),
        }
    }

    pub(crate) fn bind_group(
        &self,
        gpu: &GpuContext,
        entries: &[wgpu::BindGroupEntry],
    ) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(self.label),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries,
        })
    }

    pub(crate) fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bind_group: &wgpu::BindGroup,
        [x, y, z]: [u32; 3],
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(self.label),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(x, y, z);
    }
}

fn shader_module(gpu: &GpuContext, label: &str, source: &str) -> wgpu::ShaderModule {
    gpu.device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        })
}

fn compute_pipeline(
    gpu: &GpuContext,
    label: &str,
    module: &wgpu::ShaderModule,
    layout: Option<&wgpu::PipelineLayout>,
    entry_point: &str,
) -> wgpu::ComputePipeline {
    gpu.device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(label),
            layout,
            module,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            cache: None,
        })
}

pub(crate) fn uniform_entry(binding: u32, dynamic_size: Option<u64>) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic_size.is_some(),
            min_binding_size: dynamic_size.and_then(wgpu::BufferSize::new),
        },
        count: None,
    }
}

pub(crate) fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

pub(crate) fn texture_entry(
    binding: u32,
    access: wgpu::StorageTextureAccess,
    format: wgpu::TextureFormat,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

pub(crate) fn storage_buffer(gpu: &GpuContext, label: &str, size: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(crate) fn read_buffer(gpu: &GpuContext, buffer: &wgpu::Buffer) -> Result<Vec<f32>, GpuLost> {
    let staging = staging_buffer(gpu, "brush_readback", buffer.size());
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("brush_readback"),
        });
    encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, buffer.size());
    gpu.queue.submit([encoder.finish()]);
    read_mapped(gpu, &staging, |bytes| bytemuck::cast_slice(bytes).to_vec())
}

pub(crate) fn buffer_binding(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

pub(crate) fn texture_binding(
    binding: u32,
    canvas: &Canvas,
    which: StateTexture,
) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::TextureView(canvas.view(which)),
    }
}
