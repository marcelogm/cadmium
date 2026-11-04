use wgpu::util::DeviceExt;

use super::dispatch;
use crate::canvas::{Canvas, StateTexture};
use crate::compute::{
    ComputeStage, buffer_binding, read_buffer, storage_buffer, storage_entry, texture_binding,
    texture_entry, uniform_entry,
};
use crate::engine::PIGMENT_CHANNELS;
use crate::engine::shader::{EYEDROPPER_SNIPPETS, shader_source};
use crate::gpu::{GpuContext, GpuLost};
use crate::pigments::Mixture;

const MAX_PICK_RADIUS_PX: f32 = 16.0;
const MIN_PICK_RADIUS_PX: f32 = 0.5;
const PICKED_PAINT_FOUND: usize = 0;
const PICKED_FRACTIONS: usize = 1;
const PICKED_BYTES: u64 =
    ((PICKED_FRACTIONS + PIGMENT_CHANNELS) * std::mem::size_of::<f32>()) as u64;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PickRecord {
    center: [f32; 2],
    radius: f32,
    _pad: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PickRequest {
    pub(crate) center: [f32; 2],
    pub(crate) radius: f32,
}

pub(in crate::engine) struct PickPass {
    compute: ComputeStage,
    pipeline: wgpu::ComputePipeline,
    picked: wgpu::Buffer,
}

impl PickPass {
    pub(in crate::engine) fn new(gpu: &GpuContext) -> PickPass {
        use wgpu::StorageTextureAccess::ReadOnly;
        use wgpu::TextureFormat::{R32Float, Rgba16Float};

        let compute = ComputeStage::new(
            gpu,
            "eyedropper",
            &shader_source(
                &EYEDROPPER_SNIPPETS,
                include_str!("../../shaders/eyedropper.wgsl"),
            ),
            &[
                uniform_entry(0, None),
                texture_entry(1, ReadOnly, Rgba16Float),
                texture_entry(2, ReadOnly, Rgba16Float),
                texture_entry(3, ReadOnly, R32Float),
                texture_entry(4, ReadOnly, Rgba16Float),
                texture_entry(5, ReadOnly, Rgba16Float),
                storage_entry(6, false),
            ],
        );
        PickPass {
            pipeline: compute.pipeline(gpu, "eyedropper", "sample"),
            picked: storage_buffer(gpu, "eyedropper_picked", PICKED_BYTES),
            compute,
        }
    }

    pub(in crate::engine) fn encode(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        request: PickRequest,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        encoder.clear_buffer(&self.picked, 0, None);
        let Some(record) = pick_record(canvas, request) else {
            return;
        };
        let pick = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("eyedropper"),
                contents: bytemuck::bytes_of(&record),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind_group = self.compute.bind_group(
            gpu,
            &[
                buffer_binding(0, &pick),
                texture_binding(1, canvas, StateTexture::WetPig0),
                texture_binding(2, canvas, StateTexture::WetPig1),
                texture_binding(3, canvas, StateTexture::WetVol),
                texture_binding(4, canvas, StateTexture::DryTopPig0),
                texture_binding(5, canvas, StateTexture::DryTopPig1),
                buffer_binding(6, &self.picked),
            ],
        );
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("eyedropper"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &bind_group, &[]);
        dispatch(&mut pass, &self.pipeline, [1, 1]);
    }

    pub(in crate::engine) fn picked_mixture(
        &self,
        gpu: &GpuContext,
        volume_um: f32,
    ) -> Result<Option<Mixture>, GpuLost> {
        let picked = read_buffer(gpu, &self.picked)?;
        if picked[PICKED_PAINT_FOUND].to_bits() == 0 {
            return Ok(None);
        }
        Ok(Some(Mixture {
            volume_um,
            fractions: std::array::from_fn(|slot| picked[PICKED_FRACTIONS + slot]),
        }))
    }
}

fn pick_record(canvas: &Canvas, request: PickRequest) -> Option<PickRecord> {
    let [x, y] = request.center;
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    let radius = if request.radius.is_finite() {
        request.radius.clamp(MIN_PICK_RADIUS_PX, MAX_PICK_RADIUS_PX)
    } else {
        MIN_PICK_RADIUS_PX
    };
    let (width, height) = (canvas.spec.width as f32, canvas.spec.height as f32);
    if x + radius < 0.0 || y + radius < 0.0 || x - radius > width || y - radius > height {
        return None;
    }
    Some(PickRecord {
        center: [x, y],
        radius,
        _pad: 0.0,
    })
}
