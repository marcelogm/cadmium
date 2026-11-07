use super::readout::cell_totals;
use super::{BLADE_CELLS, BLADE_DELTA_CHANNELS, TIP_CHANNELS, WetOilEngine};
use crate::canvas::PaintTotals;
use crate::compute::read_buffer;
use crate::gpu::{GpuContext, GpuLost};
use crate::params::SimParams;

/// Cells across the mixing knife's face, in the direction the blade travels.
pub(super) const FACE_DEPTH: usize = 64;
pub(super) const FACE_CELLS: usize = BLADE_CELLS * FACE_DEPTH;
/// Per cell: the volume laid per unit of cover, the share of a pickup the cell
/// still has room for, and the share of each pigment in what it holds.
pub(super) const FACE_GRANT_CHANNELS: usize = 2 + super::PIGMENT_CHANNELS;
pub(super) const FACE_LOAD_BYTES: u64 =
    (FACE_CELLS * TIP_CHANNELS * std::mem::size_of::<f32>()) as u64;
pub(super) const FACE_DELTA_BYTES: u64 =
    (FACE_CELLS * BLADE_DELTA_CHANNELS * std::mem::size_of::<i32>()) as u64;
/// How thick a layer a face cell can carry before it stops picking up: past it the
/// cell takes nothing more, so nothing has to spill.
pub(super) const FACE_CAPACITY_UM: f32 = 1000.0;
/// The width of a face column on the canvas, so a blade uses as many columns as
/// it is pixels wide (up to `FACE_DEPTH`).
pub(super) const FACE_COLUMN_PX: f32 = 1.0;
/// The face's totals as the GPU sums them for a readout: one value per channel.
pub(super) const FACE_SUMS_BYTES: u64 = (TIP_CHANNELS * std::mem::size_of::<f32>()) as u64;

/// The face's exchange rates, read from the parameters for each batch of dabs.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct FaceSettings {
    pickup_rate: f32,
    lay_rate: f32,
    smear: f32,
    edge_unevenness: f32,
    edge_seed: u32,
    padding: [u32; 3],
}

impl FaceSettings {
    /// The settings for a batch of the stroke seeded `edge_seed`, which picks the
    /// blade's uneven contact for that stroke.
    pub(super) fn of(params: &SimParams, edge_seed: u32) -> FaceSettings {
        FaceSettings {
            pickup_rate: params.get("face_pickup_rate"),
            lay_rate: params.get("face_lay_rate"),
            smear: params.get("face_smear"),
            edge_unevenness: params.get("face_edge_unevenness"),
            edge_seed,
            padding: [0; 3],
        }
    }
}

impl WetOilEngine {
    pub fn clean_face(&mut self, gpu: &GpuContext) {
        self.submit(gpu, "clean_face", |engine, encoder| {
            engine.encode_clean_face(encoder);
        });
    }

    pub fn encode_clean_face(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.clear_buffer(&self.buffers.face_load, 0, None);
        encoder.clear_buffer(&self.buffers.face_delta, 0, None);
    }

    /// The paint on the mixing knife's face.
    pub fn face_totals(&self, gpu: &GpuContext) -> Result<PaintTotals, GpuLost> {
        Ok(cell_totals(&read_buffer(gpu, &self.buffers.face_load)?))
    }
}
