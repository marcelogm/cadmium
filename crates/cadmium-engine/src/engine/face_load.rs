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
const FACE_CAPACITY_UM: f32 = 1000.0;

/// The face's exchange rates for one batch of dabs, read from the parameters.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct FaceSettings {
    pickup_rate: f32,
    lay_rate: f32,
    smear: f32,
    capacity_um: f32,
}

impl FaceSettings {
    pub(super) fn of(params: &SimParams) -> FaceSettings {
        FaceSettings {
            pickup_rate: params.get("face_pickup_rate"),
            lay_rate: params.get("face_lay_rate"),
            smear: params.get("face_smear"),
            capacity_um: FACE_CAPACITY_UM,
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
