use wgpu::util::DeviceExt;

use super::cells::CELL_GRID_BINS;
use super::clusters::CLUSTER_INDEX_WORDS;
use super::layout::{BRISTLE_VERTICES, BristleLayout, BristleRest, MAX_BRISTLES};
use super::owners::RANKS_PER_TEXEL;
use super::tuning::{BRISTLE_TUNING_BYTES, BristleTuning};
use crate::compute::{read_buffer, storage_buffer};
use crate::engine::{TIP_CHANNELS, TIP_TEXELS, VOLUME_CHANNEL, WetOilEngine};
use crate::gpu::{GpuContext, GpuLost};

pub(crate) const BRISTLE_WORKGROUP_SIZE: u32 = 512;
pub(crate) const BRISTLES_PER_THREAD: u32 = MAX_BRISTLES as u32 / BRISTLE_WORKGROUP_SIZE;
/// Each vertex's position and the floor under it.
pub(crate) const BRISTLE_STATE_VECTORS: usize = BRISTLE_VERTICES;
pub(crate) const BRISTLE_FLARE_SHARE: f32 = 0.015;
pub(crate) const FAN_REST_SPLAY_SHARE: f32 = 1.0;
pub(crate) const MIN_SEGMENT_SPAN_PX: f32 = 1e-4;
const TOUCH_TOLERANCE_PX: f32 = 0.01;
pub const RELIEF_UM_PER_PX: f32 = 40.0;
const VECTOR_FLOATS: usize = 4;
const STATE_FLOATS: usize = BRISTLE_STATE_VECTORS * VECTOR_FLOATS;
const STATE_BYTES: u64 = (MAX_BRISTLES * STATE_FLOATS * std::mem::size_of::<f32>()) as u64;
const REST_BYTES: u64 = (MAX_BRISTLES * std::mem::size_of::<BristleRest>()) as u64;
const OWNER_BYTES: u64 = (TIP_TEXELS * std::mem::size_of::<u32>()) as u64;
const CELL_GRID_BYTES: u64 = (MAX_BRISTLES * CELL_GRID_BINS * std::mem::size_of::<u32>()) as u64;
const RANK_BYTES: u64 = RANKS_PER_TEXEL as u64 * OWNER_BYTES;
const CLUSTER_BYTES: u64 = (CLUSTER_INDEX_WORDS * std::mem::size_of::<u32>()) as u64;
const HANDLE_STATE_BYTES: u64 = 64;
const HAIR_LOAD_BYTES: u64 = (MAX_BRISTLES * std::mem::size_of::<f32>()) as u64;
const _: () = assert!(MAX_BRISTLES as u32 == BRISTLES_PER_THREAD * BRISTLE_WORKGROUP_SIZE);
const _: () = assert!(std::mem::size_of::<BristleRest>() == 64);

pub(crate) struct BristleBuffers {
    pub(crate) rests: wgpu::Buffer,
    pub(crate) states: wgpu::Buffer,
    pub(crate) handle: wgpu::Buffer,
    pub(crate) hair_loads: wgpu::Buffer,
    /// The canvas's normal force on each strand at the end of the last dab, N.
    pub(crate) normal_forces: wgpu::Buffer,
    pub(crate) owners: wgpu::Buffer,
    pub(crate) ranks: wgpu::Buffer,
    pub(crate) cell_grids: wgpu::Buffer,
    pub(crate) clusters: wgpu::Buffer,
    pub(crate) tuning: wgpu::Buffer,
}

impl BristleBuffers {
    pub(crate) fn new(gpu: &GpuContext) -> BristleBuffers {
        BristleBuffers {
            rests: storage_buffer(gpu, "bristle_rests", REST_BYTES),
            states: storage_buffer(gpu, "bristle_states", STATE_BYTES),
            handle: storage_buffer(gpu, "bristle_handle", HANDLE_STATE_BYTES),
            hair_loads: storage_buffer(gpu, "bristle_hair_loads", HAIR_LOAD_BYTES),
            normal_forces: storage_buffer(gpu, "bristle_normal_forces", HAIR_LOAD_BYTES),
            owners: storage_buffer(gpu, "bristle_owners", OWNER_BYTES),
            ranks: storage_buffer(gpu, "bristle_owner_ranks", RANK_BYTES),
            cell_grids: storage_buffer(gpu, "bristle_cell_grids", CELL_GRID_BYTES),
            clusters: storage_buffer(gpu, "bristle_clusters", CLUSTER_BYTES),
            tuning: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("bristle_tuning"),
                size: BRISTLE_TUNING_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }

    pub(super) fn upload(
        &self,
        gpu: &GpuContext,
        laid: &BristleLayout,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        copy_into(
            gpu,
            encoder,
            &self.rests,
            bytemuck::cast_slice(&laid.bristles),
        );
        copy_into(
            gpu,
            encoder,
            &self.owners,
            bytemuck::cast_slice(&laid.owners),
        );
        copy_into(gpu, encoder, &self.ranks, bytemuck::cast_slice(&laid.ranks));
        copy_into(
            gpu,
            encoder,
            &self.cell_grids,
            bytemuck::cast_slice(&laid.cell_grids),
        );
        copy_into(
            gpu,
            encoder,
            &self.clusters,
            bytemuck::cast_slice(&laid.cluster_index),
        );
    }

    pub(crate) fn tune(
        &self,
        gpu: &GpuContext,
        tuning: &BristleTuning,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        copy_into(gpu, encoder, &self.tuning, bytemuck::bytes_of(tuning));
    }
}

fn copy_into(
    gpu: &GpuContext,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::Buffer,
    bytes: &[u8],
) {
    if bytes.is_empty() {
        return;
    }
    let staging = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bristle_upload"),
            contents: bytes,
            usage: wgpu::BufferUsages::COPY_SRC,
        });
    encoder.copy_buffer_to_buffer(&staging, 0, target, 0, bytes.len() as u64);
}

#[derive(Clone, Debug, PartialEq)]
pub struct BristleSnapshot {
    pub tips_px: Vec<[f32; 3]>,
    pub roots_px: Vec<[f32; 2]>,
    pub touching: Vec<bool>,
    pub lowest_clearance_px: f32,
    /// Each bristle's plastic lean; [0, 0] until phase 2 of #62 brings plasticity back.
    pub rest_bends: Vec<[f32; 2]>,
    pub chains_px: Vec<Vec<[f32; 3]>>,
    /// Per bristle and vertex, the height over the floor under it, px.
    pub clearances_px: Vec<Vec<f32>>,
    /// The canvas's normal force on each bristle at the end of the last dab, N.
    pub normal_forces_n: Vec<f32>,
    pub radii_px: Vec<f32>,
    pub clusters: Vec<u32>,
}

impl BristleSnapshot {
    fn of(floats: &[f32], forces: &[f32], radii_px: &[f32], clusters: &[u32]) -> BristleSnapshot {
        let states: Vec<&[f32]> = floats
            .as_chunks::<STATE_FLOATS>()
            .0
            .iter()
            .take(radii_px.len())
            .map(|state| state.as_slice())
            .collect();
        BristleSnapshot {
            tips_px: states.iter().copied().map(tip).collect(),
            roots_px: states.iter().copied().map(root).collect(),
            touching: states
                .iter()
                .zip(forces)
                .map(|(state, &force)| touches(state, force))
                .collect(),
            lowest_clearance_px: states
                .iter()
                .flat_map(|state| clearances(state, 0))
                .fold(f32::INFINITY, f32::min),
            rest_bends: vec![[0.0; 2]; states.len()],
            chains_px: states.iter().copied().map(chain).collect(),
            clearances_px: states
                .iter()
                .map(|state| clearances(state, 0).collect())
                .collect(),
            normal_forces_n: forces[..states.len()].to_vec(),
            radii_px: radii_px.to_vec(),
            clusters: clusters.to_vec(),
        }
    }
}

fn vertex(state: &[f32], index: usize) -> [f32; 3] {
    let start = index * VECTOR_FLOATS;
    [state[start], state[start + 1], state[start + 2]]
}

fn chain(state: &[f32]) -> Vec<[f32; 3]> {
    (0..BRISTLE_VERTICES)
        .map(|index| vertex(state, index))
        .collect()
}

fn tip(state: &[f32]) -> [f32; 3] {
    vertex(state, BRISTLE_VERTICES - 1)
}

fn root(state: &[f32]) -> [f32; 2] {
    let [x, y, _] = vertex(state, 0);
    [x, y]
}

fn clearances(state: &[f32], first: usize) -> impl Iterator<Item = f32> {
    (first..BRISTLE_VERTICES).map(move |index| {
        let start = index * VECTOR_FLOATS;
        state[start + 2] - state[start + 3]
    })
}

/// The canvas pushes on the strand, or a vertex rests on it. A pushed vertex can end a
/// little over its floor: the solve lands it on the linearised step.
fn touches(state: &[f32], normal_force_n: f32) -> bool {
    normal_force_n > 0.0 || clearances(state, 1).any(|clearance| clearance <= TOUCH_TOLERANCE_PX)
}

impl WetOilEngine {
    pub fn bristle_snapshot(&self, gpu: &GpuContext) -> Result<BristleSnapshot, GpuLost> {
        let floats = read_buffer(gpu, &self.buffers.bristles.states)?;
        let forces = read_buffer(gpu, &self.buffers.bristles.normal_forces)?;
        Ok(BristleSnapshot::of(
            &floats,
            &forces,
            self.bristles.radii_px(),
            self.bristles.clusters(),
        ))
    }

    pub fn bristle_cells(&self, gpu: &GpuContext) -> Result<Vec<BristleCell>, GpuLost> {
        let owners = read_buffer(gpu, &self.buffers.bristles.owners)?;
        let tip = read_buffer(gpu, &self.buffers.tip)?;
        Ok(bristle_cells(&owners, &tip, self.bristles.radii_px().len()))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BristleCell {
    pub volume_um_texels: f32,
    pub texels: u32,
}

fn bristle_cells(owners: &[f32], tip: &[f32], count: usize) -> Vec<BristleCell> {
    let mut cells = vec![BristleCell::default(); count];
    for (texel, owner) in owners.iter().enumerate() {
        if let Some(cell) = cells.get_mut(owner.to_bits() as usize) {
            cell.volume_um_texels += tip[texel * TIP_CHANNELS + VOLUME_CHANNEL];
            cell.texels += 1;
        }
    }
    cells
}
