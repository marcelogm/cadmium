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
    pub still_reach_px: f32,
}

const STILL_REACH_OF_RADIUS: f32 = 0.25;
const BLOB_SETTLE_SECONDS: f32 = 0.25;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SqueezeRecord {
    segment_start: [f32; 2],
    segment_end: [f32; 2],
    radius: f32,
    height_um: f32,
    max_um: f32,
    slot: u32,
    origin: [i32; 2],
    size: [u32; 2],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Dwell {
    anchor: [f32; 2],
    seconds: f32,
}

pub(crate) fn still_reach_px(nozzle: &Nozzle) -> f32 {
    let of_radius = STILL_REACH_OF_RADIUS * nozzle.radius;
    if nozzle.still_reach_px.is_finite() && nozzle.still_reach_px > 0.0 {
        of_radius.max(nozzle.still_reach_px)
    } else {
        of_radius
    }
}

pub(crate) fn dwell_after(previous: Option<Dwell>, nozzle: &Nozzle) -> Dwell {
    let anchor = previous.map_or(nozzle.from, |dwell| dwell.anchor);
    let seconds = previous.map_or(0.0, |dwell| dwell.seconds);
    let away = (nozzle.to[0] - anchor[0]).hypot(nozzle.to[1] - anchor[1]);
    if away < still_reach_px(nozzle) {
        Dwell {
            anchor,
            seconds: seconds + nozzle.seconds.max(0.0),
        }
    } else {
        Dwell {
            anchor: nozzle.to,
            seconds: 0.0,
        }
    }
}

pub(crate) fn squeezed_height_um(nozzle: &Nozzle, params: &SimParams, dwell: Dwell) -> f32 {
    let pressure = nozzle.pressure.clamp(0.0, 1.0);
    let seconds = nozzle.seconds.max(0.0);
    if pressure.is_nan() || pressure <= 0.0 || seconds <= 0.0 {
        return 0.0;
    }
    let crest = params.get("tube_cord_um") * pressure;
    let settled = (dwell.seconds - BLOB_SETTLE_SECONDS).max(0.0);
    let grown = params.get("tube_flow_um_per_s") * pressure * settled;
    (crest + grown).min(params.get("tube_max_um"))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SqueezeRegion {
    pub origin: [i32; 2],
    pub size: [u32; 2],
    pub height_um: f32,
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
    dwell: Dwell,
) -> Option<SqueezeRegion> {
    let finite =
        nozzle.from.iter().chain(&nozzle.to).all(|v| v.is_finite()) && nozzle.radius.is_finite();
    let on_a_slot = nozzle.slot < canvas.spec.pigment_slots.len();
    if !finite || !on_a_slot || nozzle.radius <= 0.0 {
        return None;
    }
    let height_um = squeezed_height_um(nozzle, params, dwell);
    if height_um.is_nan() || height_um <= 0.0 {
        return None;
    }
    let first =
        [0, 1].map(|axis| (nozzle.from[axis].min(nozzle.to[axis]) - nozzle.radius).floor() as i32);
    let last =
        [0, 1].map(|axis| (nozzle.from[axis].max(nozzle.to[axis]) + nozzle.radius).ceil() as i32);
    let limits = [canvas.spec.width as i32, canvas.spec.height as i32];
    let (origin, size) = clamped_region(first, last, NOZZLE_APRON_PX, limits)?;
    Some(SqueezeRegion {
        origin,
        size,
        height_um,
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
        let record = SqueezeRecord {
            segment_start: nozzle.from,
            segment_end: nozzle.to,
            radius: nozzle.radius,
            height_um: region.height_um,
            max_um: params.get("tube_max_um"),
            slot: nozzle.slot as u32,
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

    fn nozzle(to: [f32; 2], seconds: f32) -> Nozzle {
        Nozzle {
            from: [0.0, 0.0],
            to,
            radius: 8.0,
            slot: 0,
            pressure: 1.0,
            seconds,
            still_reach_px: 0.0,
        }
    }

    fn held_for(seconds: f32) -> Dwell {
        Dwell {
            anchor: [0.0, 0.0],
            seconds,
        }
    }

    #[test]
    fn a_nozzle_within_a_quarter_radius_of_its_anchor_keeps_dwelling() {
        let dwell = dwell_after(Some(held_for(0.5)), &nozzle([1.9, 0.0], 0.1));
        assert_eq!(dwell.anchor, [0.0, 0.0]);
        assert!((dwell.seconds - 0.6).abs() < 1e-6);
    }

    #[test]
    fn a_nozzle_that_leaves_its_anchor_starts_dwelling_afresh_where_it_is() {
        let dwell = dwell_after(Some(held_for(0.5)), &nozzle([2.1, 0.0], 0.1));
        assert_eq!(dwell.anchor, [2.1, 0.0]);
        assert_eq!(dwell.seconds, 0.0);
    }

    #[test]
    fn the_still_reach_is_the_wider_of_a_quarter_radius_and_the_screen_reach() {
        let reach = |screen_reach: f32| {
            still_reach_px(&Nozzle {
                still_reach_px: screen_reach,
                ..nozzle([0.0, 0.0], 0.1)
            })
        };
        assert_eq!(reach(12.0), 12.0);
        assert_eq!(reach(0.5), 2.0);
        for unknown in [0.0, -3.0, f32::NAN, f32::INFINITY] {
            assert_eq!(reach(unknown), 2.0);
        }
    }

    #[test]
    fn the_first_squeeze_of_a_hold_dwells_from_where_it_touched() {
        let dwell = dwell_after(None, &nozzle([0.5, 0.0], 0.1));
        assert_eq!(dwell.anchor, [0.0, 0.0]);
        assert!((dwell.seconds - 0.1).abs() < 1e-6);
    }

    #[test]
    fn a_moving_nozzle_raises_the_cord_whatever_its_speed() {
        let params = SimParams::default();
        let cord = params.get("tube_cord_um");
        for to in [[3.0, 0.0], [100.0, 0.0]] {
            let moving = nozzle(to, 0.02);
            let height = squeezed_height_um(&moving, &params, dwell_after(None, &moving));
            assert_eq!(height, cord);
        }
    }

    #[test]
    fn a_blob_grows_only_after_the_nozzle_settles() {
        let params = SimParams::default();
        let still = nozzle([0.0, 0.0], 0.02);
        let height = |seconds| squeezed_height_um(&still, &params, held_for(seconds));
        assert_eq!(height(BLOB_SETTLE_SECONDS), params.get("tube_cord_um"));
        assert!(height(BLOB_SETTLE_SECONDS + 0.2) > height(BLOB_SETTLE_SECONDS + 0.1));
    }

    #[test]
    fn a_blob_never_rises_above_the_tube_height() {
        let params = SimParams::default();
        let still = nozzle([0.0, 0.0], 0.02);
        assert_eq!(
            squeezed_height_um(&still, &params, held_for(60.0)),
            params.get("tube_max_um")
        );
    }

    #[test]
    fn pressing_harder_raises_the_cord() {
        let params = SimParams::default();
        let hard = nozzle([20.0, 0.0], 0.1);
        let soft = Nozzle {
            pressure: 0.3,
            ..hard
        };
        assert!(
            squeezed_height_um(&hard, &params, held_for(0.0))
                > squeezed_height_um(&soft, &params, held_for(0.0))
        );
    }

    #[test]
    fn no_time_squeezes_nothing() {
        let params = SimParams::default();
        for to in [[0.0, 0.0], [30.0, 0.0]] {
            assert_eq!(
                squeezed_height_um(&nozzle(to, 0.0), &params, held_for(1.0)),
                0.0
            );
        }
    }
}
