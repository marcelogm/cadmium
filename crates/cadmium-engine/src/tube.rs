use wgpu::util::DeviceExt;

use crate::canvas::{Canvas, StateTexture};
use crate::engine::{
    WORKGROUP_SIZE, buffer_binding, clamped_region, compute_pipeline, layouts, shader_module,
    texture_binding, texture_entry, uniform_entry,
};
use crate::gpu::GpuContext;
use crate::params::SimParams;

const NOZZLE_APRON_PX: i32 = 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Nozzle {
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub radius: f32,
    pub slot: usize,
    pub pressure: f32,
    pub seconds: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SqueezeRecord {
    segment_start: [f32; 2],
    segment_end: [f32; 2],
    radius: f32,
    thickness_um: f32,
    max_um: f32,
    slot: u32,
    origin: [i32; 2],
    size: [u32; 2],
}

pub fn squeezed_thickness_um(nozzle: &Nozzle, flow_um_per_s: f32) -> f32 {
    let length = (nozzle.to[0] - nozzle.from[0]).hypot(nozzle.to[1] - nozzle.from[1]);
    let disc = std::f32::consts::PI * nozzle.radius * nozzle.radius;
    let capsule = disc + 2.0 * nozzle.radius * length;
    flow_um_per_s * nozzle.pressure.clamp(0.0, 1.0) * nozzle.seconds.max(0.0) * disc / capsule
}

pub fn squeeze_region(canvas: &Canvas, nozzle: &Nozzle) -> Option<([i32; 2], [u32; 2])> {
    let finite =
        nozzle.from.iter().chain(&nozzle.to).all(|v| v.is_finite()) && nozzle.radius.is_finite();
    let squeezing = nozzle.radius > 0.0 && nozzle.pressure > 0.0 && nozzle.seconds > 0.0;
    if !finite || !squeezing {
        return None;
    }
    let first =
        [0, 1].map(|axis| (nozzle.from[axis].min(nozzle.to[axis]) - nozzle.radius).floor() as i32);
    let last =
        [0, 1].map(|axis| (nozzle.from[axis].max(nozzle.to[axis]) + nozzle.radius).ceil() as i32);
    let limits = [canvas.spec.width as i32, canvas.spec.height as i32];
    clamped_region(first, last, NOZZLE_APRON_PX, limits)
}

pub struct Tube {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
}

impl Tube {
    pub fn new(gpu: &GpuContext) -> Tube {
        use wgpu::StorageTextureAccess::ReadWrite;
        use wgpu::TextureFormat::{R32Float, Rgba16Float};
        let (layout, pipeline_layout) = layouts(
            gpu,
            "squeeze",
            &[
                uniform_entry(0, None),
                texture_entry(1, ReadWrite, Rgba16Float),
                texture_entry(2, ReadWrite, Rgba16Float),
                texture_entry(3, ReadWrite, R32Float),
            ],
        );
        let module = shader_module(gpu, "squeeze", &[], include_str!("shaders/squeeze.wgsl"));
        let pipeline = compute_pipeline(gpu, "squeeze", &module, &pipeline_layout, "main");
        Tube { layout, pipeline }
    }

    pub fn encode_squeeze(
        &self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        params: &SimParams,
        nozzle: &Nozzle,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Option<[f32; 4]> {
        let (origin, size) = squeeze_region(canvas, nozzle)?;
        let thickness_um = squeezed_thickness_um(nozzle, params.get("tube_flow_um_per_s"));
        if thickness_um.is_nan() || thickness_um <= 0.0 {
            return None;
        }
        let record = SqueezeRecord {
            segment_start: nozzle.from,
            segment_end: nozzle.to,
            radius: nozzle.radius,
            thickness_um,
            max_um: params.get("tube_max_um"),
            slot: nozzle.slot as u32,
            origin,
            size,
        };
        let uniform = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("squeeze"),
                contents: bytemuck::bytes_of(&record),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("squeeze"),
            layout: &self.layout,
            entries: &[
                buffer_binding(0, &uniform),
                texture_binding(1, canvas, StateTexture::WetPig0),
                texture_binding(2, canvas, StateTexture::WetPig1),
                texture_binding(3, canvas, StateTexture::WetVol),
            ],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("squeeze"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &group, &[]);
        pass.set_pipeline(&self.pipeline);
        pass.dispatch_workgroups(
            size[0].div_ceil(WORKGROUP_SIZE),
            size[1].div_ceil(WORKGROUP_SIZE),
            1,
        );
        drop(pass);
        let rect = region_rect(origin, size);
        canvas.dirty.mark_rect(rect[0], rect[1], rect[2], rect[3]);
        canvas
            .wet_tiles
            .mark_rect(rect[0], rect[1], rect[2], rect[3]);
        Some(rect)
    }
}

pub(crate) fn region_rect(origin: [i32; 2], size: [u32; 2]) -> [f32; 4] {
    [
        origin[0] as f32,
        origin[1] as f32,
        (origin[0] + size[0] as i32 - 1) as f32,
        (origin[1] + size[1] as i32 - 1) as f32,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nozzle(length: f32, seconds: f32) -> Nozzle {
        Nozzle {
            from: [0.0, 0.0],
            to: [length, 0.0],
            radius: 10.0,
            slot: 0,
            pressure: 1.0,
            seconds,
        }
    }

    #[test]
    fn a_nozzle_dragged_far_spreads_the_same_squeeze_thinner() {
        assert!(
            squeezed_thickness_um(&nozzle(0.0, 0.1), 300.0)
                > squeezed_thickness_um(&nozzle(100.0, 0.1), 300.0)
        );
    }

    #[test]
    fn no_time_squeezes_nothing() {
        assert_eq!(squeezed_thickness_um(&nozzle(0.0, 0.0), 300.0), 0.0);
    }
}
