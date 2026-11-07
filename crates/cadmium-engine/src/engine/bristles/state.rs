use wgpu::util::DeviceExt;

use super::cells::CELL_GRID_BINS;
use super::clusters::{CLUSTER_INDEX_WORDS, MAX_CLUSTERS};
use super::layout::{BRISTLE_VERTICES, BristleLayout, BristleRest, MAX_BRISTLES};
use super::owners::RANKS_PER_TEXEL;
use super::tuning::{BRISTLE_TUNING_BYTES, BristleTuning};
use crate::compute::{read_buffer, storage_buffer};
use crate::engine::{TIP_CHANNELS, TIP_TEXELS, VOLUME_CHANNEL, WetOilEngine};
use crate::gpu::{GpuContext, GpuLost};

pub(crate) const BRISTLE_WORKGROUP_SIZE: u32 = 512;
pub(crate) const BRISTLES_PER_THREAD: u32 = MAX_BRISTLES as u32 / BRISTLE_WORKGROUP_SIZE;
pub(crate) const BRISTLE_STATE_VECTORS: usize = 3 * BRISTLE_VERTICES + 1;
pub(crate) const BRISTLE_SOLVER_ITERATIONS: u32 = 6;
pub(crate) const BRISTLE_TIP_STIFFNESS_SHARE: f32 = 0.3;
pub(crate) const BRISTLE_FLARE_SHARE: f32 = 0.015;
pub(crate) const FAN_REST_SPLAY_SHARE: f32 = 1.0;
pub(crate) const MIN_SEGMENT_SPAN_PX: f32 = 1e-4;
pub(crate) const PLASTIC_BEND_THRESHOLD_RAD: f32 = 0.35;
pub(crate) const MAX_PLASTIC_LEAN: f32 = 1.0;
pub(crate) const MIN_LEAN_DROP: f32 = 0.05;
pub(crate) const LANE_SAMPLES: u32 = 4;
pub(crate) const CLUSTER_CENTROIDS: usize = MAX_CLUSTERS * BRISTLE_VERTICES;
pub(crate) const TOUCH_TOLERANCE_PX: f32 = 0.01;
pub const RELIEF_UM_PER_PX: f32 = 40.0;
const VECTOR_FLOATS: usize = 4;
const BEND_VECTOR: usize = 3 * BRISTLE_VERTICES;
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
const _: () = assert!(std::mem::size_of::<BristleRest>() == 48);

pub(crate) struct BristleBuffers {
    pub(crate) rests: wgpu::Buffer,
    pub(crate) states: wgpu::Buffer,
    pub(crate) handle: wgpu::Buffer,
    pub(crate) hair_loads: wgpu::Buffer,
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
    pub rest_bends: Vec<[f32; 2]>,
    pub chains_px: Vec<Vec<[f32; 3]>>,
    pub radii_px: Vec<f32>,
    pub clusters: Vec<u32>,
}

impl BristleSnapshot {
    fn of(floats: &[f32], radii_px: &[f32], clusters: &[u32]) -> BristleSnapshot {
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
            touching: states.iter().copied().map(touches).collect(),
            lowest_clearance_px: states
                .iter()
                .flat_map(|state| clearances(state, 0))
                .fold(f32::INFINITY, f32::min),
            rest_bends: states.iter().copied().map(rest_bend).collect(),
            chains_px: states.iter().copied().map(chain).collect(),
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

fn rest_bend(state: &[f32]) -> [f32; 2] {
    let [x, y, _] = vertex(state, BEND_VECTOR);
    [x, y]
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

fn touches(state: &[f32]) -> bool {
    clearances(state, 1).any(|clearance| clearance <= TOUCH_TOLERANCE_PX)
}

impl WetOilEngine {
    pub fn bristle_snapshot(&self, gpu: &GpuContext) -> Result<BristleSnapshot, GpuLost> {
        let floats = read_buffer(gpu, &self.buffers.bristles.states)?;
        Ok(BristleSnapshot::of(
            &floats,
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
