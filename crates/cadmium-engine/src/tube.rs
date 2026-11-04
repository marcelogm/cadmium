use wgpu::util::DeviceExt;

use crate::canvas::{Canvas, StateTexture};
use crate::engine::{
    WORKGROUP_SIZE, buffer_binding, clamped_region, compute_pipeline, layouts, region_rect,
    shader_module, texture_binding, texture_entry, uniform_entry,
};
use crate::gpu::GpuContext;
use crate::params::SimParams;

const SQUEEZE_SNIPPETS: [&str; 2] = [
    include_str!("shaders/nearest_half.wgsl"),
    include_str!("shaders/slot_lane.wgsl"),
];
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

const STILL_NOZZLE_PX: f32 = 0.5;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SqueezeRecord {
    segment_start: [f32; 2],
    segment_end: [f32; 2],
    radius: f32,
    crest_um: f32,
    grown_um: f32,
    max_um: f32,
    slot: u32,
    blob: u32,
    start_cap: u32,
    _pad: u32,
    origin: [i32; 2],
    size: [u32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Squeezed {
    Blob { grown_um: f32 },
    Cord { crest_um: f32, start_cap: bool },
}

pub(crate) fn squeezed(
    nozzle: &Nozzle,
    params: &SimParams,
    continues_cord_at: Option<[f32; 2]>,
) -> Option<Squeezed> {
    let pressure = nozzle.pressure.clamp(0.0, 1.0);
    let seconds = nozzle.seconds.max(0.0);
    if pressure.is_nan() || seconds.is_nan() || pressure <= 0.0 || seconds <= 0.0 {
        return None;
    }
    let length = (nozzle.to[0] - nozzle.from[0]).hypot(nozzle.to[1] - nozzle.from[1]);
    if length < STILL_NOZZLE_PX {
        return Some(Squeezed::Blob {
            grown_um: params.get("tube_flow_um_per_s") * pressure * seconds,
        });
    }
    Some(Squeezed::Cord {
        crest_um: params.get("tube_cord_um") * pressure,
        start_cap: continues_cord_at != Some(nozzle.from),
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SqueezeRegion {
    pub origin: [i32; 2],
    pub size: [u32; 2],
    pub squeezed: Squeezed,
}

impl SqueezeRegion {
    pub fn rect(&self) -> [f32; 4] {
        region_rect(self.origin, self.size)
    }
}

pub(crate) fn squeeze_region(
    canvas: &Canvas,
    params: &SimParams,
    nozzle: &Nozzle,
    continues_cord_at: Option<[f32; 2]>,
) -> Option<SqueezeRegion> {
    let finite =
        nozzle.from.iter().chain(&nozzle.to).all(|v| v.is_finite()) && nozzle.radius.is_finite();
    let on_a_slot = nozzle.slot < canvas.spec.pigment_slots.len();
    if !finite || !on_a_slot || nozzle.radius <= 0.0 {
        return None;
    }
    let squeezed = squeezed(nozzle, params, continues_cord_at)?;
    let first =
        [0, 1].map(|axis| (nozzle.from[axis].min(nozzle.to[axis]) - nozzle.radius).floor() as i32);
    let last =
        [0, 1].map(|axis| (nozzle.from[axis].max(nozzle.to[axis]) + nozzle.radius).ceil() as i32);
    let limits = [canvas.spec.width as i32, canvas.spec.height as i32];
    let (origin, size) = clamped_region(first, last, NOZZLE_APRON_PX, limits)?;
    Some(SqueezeRegion {
        origin,
        size,
        squeezed,
    })
}

pub(crate) struct Tube {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
}

impl Tube {
    pub(crate) fn new(gpu: &GpuContext) -> Tube {
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
        let module = shader_module(
            gpu,
            "squeeze",
            &SQUEEZE_SNIPPETS,
            include_str!("shaders/squeeze.wgsl"),
        );
        let pipeline = compute_pipeline(gpu, "squeeze", &module, &pipeline_layout, "main");
        Tube { layout, pipeline }
    }

    pub(crate) fn encode_squeeze(
        &self,
        gpu: &GpuContext,
        canvas: &mut Canvas,
        params: &SimParams,
        nozzle: &Nozzle,
        region: &SqueezeRegion,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let (grown_um, crest_um, start_cap) = match region.squeezed {
            Squeezed::Blob { grown_um } => (grown_um, 0.0, true),
            Squeezed::Cord {
                crest_um,
                start_cap,
            } => (0.0, crest_um, start_cap),
        };
        let record = SqueezeRecord {
            segment_start: nozzle.from,
            segment_end: nozzle.to,
            radius: nozzle.radius,
            crest_um,
            grown_um,
            max_um: params.get("tube_max_um"),
            slot: nozzle.slot as u32,
            blob: u32::from(matches!(region.squeezed, Squeezed::Blob { .. })),
            start_cap: u32::from(start_cap),
            _pad: 0,
            origin: region.origin,
            size: region.size,
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
            region.size[0].div_ceil(WORKGROUP_SIZE),
            region.size[1].div_ceil(WORKGROUP_SIZE),
            1,
        );
        drop(pass);
        let [x0, y0, x1, y1] = region.rect();
        canvas.dirty.mark_rect(x0, y0, x1, y1);
        canvas.wet_tiles.mark_rect(x0, y0, x1, y1);
    }
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

    fn crest(squeezed: Option<Squeezed>) -> f32 {
        match squeezed {
            Some(Squeezed::Cord { crest_um, .. }) => crest_um,
            other => panic!("not a cord: {other:?}"),
        }
    }

    #[test]
    fn a_fast_and_a_slow_nozzle_raise_the_same_cord() {
        let params = SimParams::default();
        assert_eq!(
            crest(squeezed(&nozzle(5.0, 0.1), &params, None)),
            crest(squeezed(&nozzle(100.0, 0.01), &params, None))
        );
    }

    #[test]
    fn pressing_harder_raises_the_cord() {
        let params = SimParams::default();
        let soft = Nozzle {
            pressure: 0.3,
            ..nozzle(20.0, 0.1)
        };
        assert!(
            crest(squeezed(&nozzle(20.0, 0.1), &params, None))
                > crest(squeezed(&soft, &params, None))
        );
    }

    #[test]
    fn a_nozzle_that_barely_moved_grows_a_blob_with_time() {
        let params = SimParams::default();
        let grown = |seconds| match squeezed(&nozzle(0.2, seconds), &params, None) {
            Some(Squeezed::Blob { grown_um }) => grown_um,
            other => panic!("not a blob: {other:?}"),
        };
        assert!(grown(0.2) > grown(0.1));
    }

    #[test]
    fn a_cord_drops_its_start_cap_only_where_the_last_cord_ended() {
        let params = SimParams::default();
        let start_cap = |continues| match squeezed(&nozzle(20.0, 0.1), &params, continues) {
            Some(Squeezed::Cord { start_cap, .. }) => start_cap,
            other => panic!("not a cord: {other:?}"),
        };
        assert!(start_cap(None));
        assert!(start_cap(Some([5.0, 0.0])));
        assert!(!start_cap(Some([0.0, 0.0])));
    }

    #[test]
    fn no_time_squeezes_nothing() {
        let params = SimParams::default();
        assert_eq!(squeezed(&nozzle(0.0, 0.0), &params, None), None);
        assert_eq!(squeezed(&nozzle(30.0, 0.0), &params, None), None);
    }
}
