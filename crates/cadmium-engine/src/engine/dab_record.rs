use wgpu::util::DeviceExt;

use super::placement::Placement;
use super::{DELTA_SCALE, KNIFE_LAID_CAPACITY_UM, MAX_TIP_THICKNESS_UM};
use crate::gpu::GpuContext;
use crate::params::SimParams;

pub(super) const DAB_RECORD_STRIDE: u64 = 256;
const CAPILLARY_POOL_BUDGET: f32 = 4.0e9;
const KNIFE_LAID_ALTITUDE_RAD: f32 = std::f32::consts::FRAC_PI_4;
const KNIFE_UPRIGHT_ALTITUDE_RAD: f32 = 80.0 * std::f32::consts::PI / 180.0;
const KNIFE_UPRIGHT_CAPACITY_UM: f32 = 200.0;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct DabRecord {
    pub(super) center: [f32; 2],
    pub(super) radius: f32,
    pub(super) pressure: f32,
    pub(super) altitude: f32,
    pub(super) azimuth: f32,
    pub(super) direction: f32,
    pub(super) speed: f32,
    pub(super) orientation: [f32; 2],
    pub(super) elongation: f32,
    pub(super) tool_mode: u32,
    pub(super) canvas_origin: [i32; 2],
    pub(super) canvas_size: [u32; 2],
    pub(super) tip_origin: [i32; 2],
    pub(super) tip_size: [u32; 2],
    pub(super) transfer_fraction: f32,
    pub(super) velocity_cutoff: f32,
    pub(super) residual_um: f32,
    pub(super) penetration_um: f32,
    pub(super) bristle_contact: u32,
    pub(super) full_reserve_volume: f32,
    pub(super) refill_rate: f32,
    pub(super) loaded_thickness_um: f32,
    pub(super) advection: f32,
    pub(super) squeeze: f32,
    pub(super) diffusion: f32,
    pub(super) blade_gap_um: f32,
    pub(super) spread_origin: [i32; 2],
    pub(super) spread_size: [u32; 2],
    pub(super) rag_strength: f32,
    pub(super) rest_follow: f32,
    pub(super) blade_capacity_um: f32,
    pub(super) blade_spill_share: f32,
    pub(super) step_ratio: f32,
    pub(super) sweep: [f32; 2],
    pub(super) capillary_rate: f32,
    pub(super) wicking_rate: f32,
    pub(super) capillary_scale: f32,
    pub(super) handle_height_px: f32,
    pub(super) handle_axis: [f32; 3],
    pub(super) handle_rest_height_px: f32,
    pub(super) ferrule_radius_px: f32,
    pub(super) dt_seconds: f32,
    pub(super) substeps: u32,
    pub(super) bristle_total: u32,
    pub(super) bristle_stiffness: f32,
    pub(super) bristle_damping: f32,
    pub(super) bristle_reset: u32,
    pub(super) bristle_orientation: [f32; 2],
    pub(super) ferrule_press_share: f32,
    pub(super) slump_rate: f32,
    pub(super) slump_yield_um2_per_px: f32,
    pub(super) hair_load_limit: f32,
}

pub(super) const DAB_RECORD_SIZE: u64 = std::mem::size_of::<DabRecord>() as u64;
const _: () = assert!(DAB_RECORD_SIZE == 256 && DAB_RECORD_SIZE <= DAB_RECORD_STRIDE);

pub(super) fn upload_dab_records(gpu: &GpuContext, placements: &[Placement]) -> wgpu::Buffer {
    let records: Vec<DabRecord> = placements
        .iter()
        .map(|placement| placement.record)
        .collect();
    upload_records(gpu, &records)
}

pub(super) fn upload_records(gpu: &GpuContext, records: &[DabRecord]) -> wgpu::Buffer {
    let mut bytes = vec![0u8; records.len() * DAB_RECORD_STRIDE as usize];
    for (slot, record) in bytes
        .as_chunks_mut::<{ DAB_RECORD_STRIDE as usize }>()
        .0
        .iter_mut()
        .zip(records)
    {
        slot[..DAB_RECORD_SIZE as usize].copy_from_slice(bytemuck::bytes_of(record));
    }
    gpu.device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("dab_records"),
            contents: &bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        })
}

fn upright_share(altitude: f32) -> f32 {
    let t = ((altitude - KNIFE_LAID_ALTITUDE_RAD)
        / (KNIFE_UPRIGHT_ALTITUDE_RAD - KNIFE_LAID_ALTITUDE_RAD))
        .clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn blade_gap_um(params: &SimParams, pressure: f32, altitude: f32) -> f32 {
    let residual = params.get("residual_um");
    let lift = (1.0 - pressure.clamp(0.0, 1.0)) * (1.0 - upright_share(altitude));
    residual + (params.get("knife_gap_um") - residual).max(0.0) * lift
}

pub(super) fn blade_capacity_um(altitude: f32) -> f32 {
    KNIFE_LAID_CAPACITY_UM
        + (KNIFE_UPRIGHT_CAPACITY_UM - KNIFE_LAID_CAPACITY_UM) * upright_share(altitude)
}

pub(super) fn rate_over_step(reference_rate: f32, step_ratio: f32) -> f32 {
    1.0 - (1.0 - reference_rate.clamp(0.0, 1.0)).powf(step_ratio)
}

pub(super) fn capillary_scale(tip_size: [u32; 2]) -> f32 {
    let texels = (tip_size[0] * tip_size[1]).max(1) as f32;
    let fitting = CAPILLARY_POOL_BUDGET / (texels * MAX_TIP_THICKNESS_UM);
    fitting.min(DELTA_SCALE).log2().floor().exp2()
}

#[cfg(test)]
mod tests;
