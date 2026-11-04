use cadmium_engine::gpu::GpuContext;

const WORKGROUP_SIZE: u32 = 8;
const BUSY_ITERATIONS: u32 = 20000;

const PATTERN_SHADER: &str = include_str!("pattern.wgsl");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FramePattern {
    red: u32,
    green_shift: u32,
    blue_shift: u32,
}

impl FramePattern {
    pub(super) fn for_frame(frame: u32) -> FramePattern {
        FramePattern {
            red: (40 + frame * 37) % 256,
            green_shift: (120 + frame * 11) % 256,
            blue_shift: (200 + frame * 53) % 256,
        }
    }

    pub(super) fn expected(&self, x: u32, y: u32) -> [u8; 4] {
        [
            self.red as u8,
            ((x + self.green_shift) % 256) as u8,
            ((y * 2 + self.blue_shift) % 256) as u8,
            255,
        ]
    }

    fn uniform(&self) -> [u32; 4] {
        [self.red, self.green_shift, self.blue_shift, BUSY_ITERATIONS]
    }
}

pub(super) struct PatternPass {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
}

impl PatternPass {
    pub(super) fn new(gpu: &GpuContext) -> PatternPass {
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("dmabuf_selftest_pattern"),
                source: wgpu::ShaderSource::Wgsl(PATTERN_SHADER.into()),
            });
        let layout = bind_group_layout(gpu);
        let pipeline = compute_pipeline(gpu, &module, &layout);
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dmabuf_selftest_pattern"),
            size: std::mem::size_of::<[u32; 4]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        PatternPass {
            pipeline,
            layout,
            uniform,
        }
    }

    pub(super) fn encode(
        &self,
        gpu: &GpuContext,
        pattern: FramePattern,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
    ) {
        let words = pattern.uniform();
        let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
        gpu.queue.write_buffer(&self.uniform, 0, &bytes);
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dmabuf_selftest_pattern"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("dmabuf_selftest_pattern"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let size = target.size();
        pass.dispatch_workgroups(
            size.width.div_ceil(WORKGROUP_SIZE),
            size.height.div_ceil(WORKGROUP_SIZE),
            1,
        );
    }
}

fn bind_group_layout(gpu: &GpuContext) -> wgpu::BindGroupLayout {
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("dmabuf_selftest_pattern"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        })
}

fn compute_pipeline(
    gpu: &GpuContext,
    module: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("dmabuf_selftest_pattern"),
            bind_group_layouts: &[Some(layout)],
            immediate_size: 0,
        });
    gpu.device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("dmabuf_selftest_pattern"),
            layout: Some(&pipeline_layout),
            module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
}
