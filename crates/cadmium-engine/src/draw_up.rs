use wgpu::util::DeviceExt;

use crate::canvas::{Canvas, PaintTotals, StateTexture};
use crate::compute::{
    ComputeStage, WORKGROUP_SIZE, buffer_binding, read_buffer, storage_buffer, storage_entry,
    texture_binding, texture_entry, uniform_entry,
};
use crate::engine::shader::shader_source;
use crate::gpu::{GpuContext, GpuLost, new_encoder};
use crate::params::SimParams;
use crate::pigments::MAX_SLOTS;
use crate::region::{clamped_region, region_rect};

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
    pub center: [f32; 2],
    pub radius: f32,
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
        center: request.center,
        radius: request.radius,
    })
}

pub(crate) struct Measured {
    group: wgpu::BindGroup,
    region: DrawUpRegion,
    pub taken: PaintTotals,
}

pub(crate) struct DrawUp {
    compute: ComputeStage,
    measure: wgpu::ComputePipeline,
    take: wgpu::ComputePipeline,
    drawn: wgpu::Buffer,
}

impl DrawUp {
    pub(crate) fn new(gpu: &GpuContext) -> DrawUp {
        use wgpu::StorageTextureAccess::{ReadOnly, ReadWrite};
        use wgpu::TextureFormat::{R32Float, Rgba16Float};
        let invocations = format!("const DRAW_UP_INVOCATIONS: u32 = {DRAW_UP_INVOCATIONS}u;");
        let compute = ComputeStage::new(
            gpu,
            "draw_up",
            &shader_source(
                &[&invocations, include_str!("shaders/slot_lane.wgsl")],
                include_str!("shaders/draw_up.wgsl"),
            ),
            &[
                uniform_entry(0, None),
                texture_entry(1, ReadOnly, Rgba16Float),
                texture_entry(2, ReadOnly, Rgba16Float),
                texture_entry(3, ReadWrite, R32Float),
                storage_entry(4, false),
            ],
        );
        DrawUp {
            measure: compute.pipeline(gpu, "draw_up_measure", "measure"),
            take: compute.pipeline(gpu, "draw_up_take", "take"),
            drawn: storage_buffer(gpu, "drawn", DRAWN_BYTES),
            compute,
        }
    }

    pub(crate) fn measure(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        region: &DrawUpRegion,
        room_um: f32,
    ) -> Result<Measured, GpuLost> {
        let record = DrawUpRecord {
            center: region.center,
            radius: region.radius,
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
        let group = self.measure_bind_group(gpu, canvas, &uniform);
        self.submit_measure(gpu, &group);
        let drawn = read_buffer(gpu, &self.drawn)?;
        Ok(Measured {
            group,
            region: *region,
            taken: drawn_totals(&drawn),
        })
    }

    fn measure_bind_group(
        &self,
        gpu: &GpuContext,
        canvas: &Canvas,
        uniform: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        self.compute.bind_group(
            gpu,
            &[
                buffer_binding(0, uniform),
                texture_binding(1, canvas, StateTexture::WetPig0),
                texture_binding(2, canvas, StateTexture::WetPig1),
                texture_binding(3, canvas, StateTexture::WetVol),
                buffer_binding(4, &self.drawn),
            ],
        )
    }

    fn submit_measure(&self, gpu: &GpuContext, group: &wgpu::BindGroup) {
        let mut encoder = new_encoder(gpu, "draw_up_measure");
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("draw_up_measure"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, group, &[]);
        pass.set_pipeline(&self.measure);
        pass.dispatch_workgroups(1, 1, 1);
        drop(pass);
        gpu.queue.submit([encoder.finish()]);
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

fn drawn_totals(drawn: &[f32]) -> PaintTotals {
    let mut taken = PaintTotals {
        volume: f64::from(drawn[DRAWN_VOLUME]),
        ..PaintTotals::default()
    };
    for (slot, amount) in drawn[DRAWN_PIGMENT..].iter().enumerate() {
        taken.pigment[slot] = f64::from(*amount);
    }
    taken
}

#[cfg(test)]
mod tests;
