use wgpu::util::DeviceExt;

use crate::canvas::{Canvas, PaintTotals, StateTexture};
use crate::engine::{
    WORKGROUP_SIZE, buffer_binding, clamped_region, compute_pipeline, layouts, read_buffer,
    region_rect, shader_module, storage_buffer, storage_entry, texture_binding, texture_entry,
    uniform_entry,
};
use crate::gpu::{GpuContext, GpuLost};
use crate::params::SimParams;
use crate::pigments::MAX_SLOTS;

const DRAW_UP_INVOCATIONS: u32 = 256;
const DRAWN_GRANTED_SHARE: usize = 0;
const DRAWN_VOLUME: usize = DRAWN_GRANTED_SHARE + 1;
const DRAWN_PIGMENT: usize = DRAWN_VOLUME + 1;
const DRAWN_BYTES: u64 = ((DRAWN_PIGMENT + MAX_SLOTS) * std::mem::size_of::<f32>()) as u64;
const FOOTPRINT_APRON_PX: i32 = 0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawUpRequest {
    pub center: [f32; 2],
    pub radius: f32,
    pub pressure: f32,
    pub seconds: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DrawnUp {
    pub taken: PaintTotals,
    pub level: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawUpRecord {
    center: [f32; 2],
    radius: f32,
    share: f32,
    room: f32,
    _pad: f32,
    origin: [i32; 2],
    size: [u32; 2],
}

pub(crate) fn drawn_share(params: &SimParams, request: &DrawUpRequest) -> f32 {
    (params.get("draw_up_rate") * request.pressure.clamp(0.0, 1.0) * request.seconds.max(0.0))
        .clamp(0.0, 1.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DrawUpRegion {
    pub origin: [i32; 2],
    pub size: [u32; 2],
    pub share: f32,
}

impl DrawUpRegion {
    pub fn rect(&self) -> [f32; 4] {
        region_rect(self.origin, self.size)
    }
}

pub(crate) fn draw_up_region(
    canvas: &Canvas,
    params: &SimParams,
    request: &DrawUpRequest,
) -> Option<DrawUpRegion> {
    let finite = request.center.iter().all(|v| v.is_finite()) && request.radius.is_finite();
    if !finite || request.radius <= 0.0 {
        return None;
    }
    let share = drawn_share(params, request);
    if share.is_nan() || share <= 0.0 {
        return None;
    }
    let first = request
        .center
        .map(|centre| (centre - request.radius).floor() as i32);
    let last = request
        .center
        .map(|centre| (centre + request.radius).ceil() as i32);
    let limits = [canvas.spec.width as i32, canvas.spec.height as i32];
    let (origin, size) = clamped_region(first, last, FOOTPRINT_APRON_PX, limits)?;
    Some(DrawUpRegion {
        origin,
        size,
        share,
    })
}

pub(crate) struct Measured {
    group: wgpu::BindGroup,
    region: DrawUpRegion,
    pub taken: PaintTotals,
}

pub(crate) struct DrawUp {
    layout: wgpu::BindGroupLayout,
    measure: wgpu::ComputePipeline,
    take: wgpu::ComputePipeline,
    drawn: wgpu::Buffer,
}

impl DrawUp {
    pub(crate) fn new(gpu: &GpuContext) -> DrawUp {
        use wgpu::StorageTextureAccess::{ReadOnly, ReadWrite};
        use wgpu::TextureFormat::{R32Float, Rgba16Float};
        let (layout, pipeline_layout) = layouts(
            gpu,
            "draw_up",
            &[
                uniform_entry(0, None),
                texture_entry(1, ReadOnly, Rgba16Float),
                texture_entry(2, ReadOnly, Rgba16Float),
                texture_entry(3, ReadWrite, R32Float),
                storage_entry(4, false),
            ],
        );
        let invocations = format!("const DRAW_UP_INVOCATIONS: u32 = {DRAW_UP_INVOCATIONS}u;");
        let module = shader_module(
            gpu,
            "draw_up",
            &[&invocations, include_str!("shaders/slot_lane.wgsl")],
            include_str!("shaders/draw_up.wgsl"),
        );
        DrawUp {
            layout,
            measure: compute_pipeline(gpu, "draw_up_measure", &module, &pipeline_layout, "measure"),
            take: compute_pipeline(gpu, "draw_up_take", &module, &pipeline_layout, "take"),
            drawn: storage_buffer(gpu, "drawn", DRAWN_BYTES),
        }
    }

    #[expect(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(crate) fn measure(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        request: &DrawUpRequest,
        region: &DrawUpRegion,
        room_um: f32,
    ) -> Result<Measured, GpuLost> {
        let record = DrawUpRecord {
            center: request.center,
            radius: request.radius,
            share: region.share,
            room: room_um,
            _pad: 0.0,
            origin: region.origin,
            size: region.size,
        };
        let uniform = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("draw_up"),
                contents: bytemuck::bytes_of(&record),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("draw_up"),
            layout: &self.layout,
            entries: &[
                buffer_binding(0, &uniform),
                texture_binding(1, canvas, StateTexture::WetPig0),
                texture_binding(2, canvas, StateTexture::WetPig1),
                texture_binding(3, canvas, StateTexture::WetVol),
                buffer_binding(4, &self.drawn),
            ],
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("draw_up_measure"),
            });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("draw_up_measure"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &group, &[]);
        pass.set_pipeline(&self.measure);
        pass.dispatch_workgroups(1, 1, 1);
        drop(pass);
        gpu.queue.submit([encoder.finish()]);
        let drawn = read_buffer(gpu, &self.drawn)?;
        let mut taken = PaintTotals {
            volume: f64::from(drawn[DRAWN_VOLUME]),
            ..PaintTotals::default()
        };
        for (slot, amount) in drawn[DRAWN_PIGMENT..].iter().enumerate() {
            taken.pigment[slot] = f64::from(*amount);
        }
        Ok(Measured {
            group,
            region: *region,
            taken,
        })
    }

    pub(crate) fn encode_take(
        &self,
        canvas: &mut Canvas,
        measured: &Measured,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let region = &measured.region;
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("draw_up_take"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &measured.group, &[]);
        pass.set_pipeline(&self.take);
        pass.dispatch_workgroups(
            region.size[0].div_ceil(WORKGROUP_SIZE),
            region.size[1].div_ceil(WORKGROUP_SIZE),
            1,
        );
        drop(pass);
        let [x0, y0, x1, y1] = region.rect();
        canvas.dirty.mark_rect(x0, y0, x1, y1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(pressure: f32, seconds: f32) -> DrawUpRequest {
        DrawUpRequest {
            center: [0.0, 0.0],
            radius: 10.0,
            pressure,
            seconds,
        }
    }

    #[test]
    fn share_grows_with_pressure_and_time() {
        let params = SimParams::default();
        let light = drawn_share(&params, &request(0.2, 0.05));
        assert!(drawn_share(&params, &request(1.0, 0.05)) > light);
        assert!(drawn_share(&params, &request(0.2, 0.1)) > light);
    }

    #[test]
    fn share_never_exceeds_everything_under_the_brush() {
        let params = SimParams::default();
        assert_eq!(drawn_share(&params, &request(1.0, 1e6)), 1.0);
        assert_eq!(drawn_share(&params, &request(1.0, f32::INFINITY)), 1.0);
    }
}
