//! Runs the strand solve of `bristle_solve.wgsl` on single strands over floor planes, so
//! its mechanics can be checked against the closed forms in `mechanics`.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use super::layout::BRISTLE_VERTICES;
use crate::compute::{ComputeStage, buffer_binding, read_buffer, storage_buffer, storage_entry};
use crate::engine::shader::{LAB_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

const HINGES: usize = BRISTLE_VERTICES - 1;
/// Per strand: the vertices (xyz, normal force), the total, the friction on each vertex
/// (xyz, anchor w), each hinge's plastic shift (xyz, excess w), each vertex's anchor, and
/// each hinge's summed excess over the yield (x).
const OUT_VECTORS: usize = 3 * BRISTLE_VERTICES + 1 + 2 * HINGES;
const VECTOR_FLOATS: usize = 4;
/// Hinge compliances a `LabStrand` carries: two vec4 in the shader.
pub(super) const LAB_HINGE_SLOTS: usize = 8;

const _: () = assert!(BRISTLE_VERTICES - 1 <= LAB_HINGE_SLOTS);

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(super) struct LabStrand {
    root: [f32; 4],
    clamp: [f32; 4],
    curl: [f32; 4],
    tip_force: [f32; 4],
    floor_slope: [f32; 4],
    step: [f32; 4],
    /// μₛ, μₖ, anisotropy C_η, creep per second.
    friction: [f32; 4],
    /// xy: how far the drag phase moves the strand, px; z: its steps; w: 1 drops the tip
    /// load for the drag phase.
    drag: [f32; 4],
    /// x: seconds each press step lasts, y: each drag step, for the creep; z: the steps of a
    /// settle phase after the drag, w: how far it raises the strand, px.
    timing: [f32; 4],
    compliances: [f32; LAB_HINGE_SLOTS],
}

impl LabStrand {
    pub(super) fn new(
        root: [f32; 3],
        clamp: [f32; 3],
        segment_px: f32,
        compliances: [f32; LAB_HINGE_SLOTS],
    ) -> LabStrand {
        let [x, y, z] = root;
        let [cx, cy, cz] = clamp;
        LabStrand {
            root: [x, y, z, segment_px],
            clamp: [cx, cy, cz, 0.0],
            curl: [0.0; 4],
            tip_force: [0.0, 0.0, 0.0, 1.0],
            floor_slope: [0.0; 4],
            step: [0.0; 4],
            friction: [0.0; 4],
            drag: [0.0; 4],
            timing: [0.0; 4],
            compliances,
        }
    }

    pub(super) fn with_floor(mut self, height_px: f32) -> LabStrand {
        self.clamp[3] = height_px;
        self
    }

    /// Tilts the floor into the plane z = height + slope · xy.
    pub(super) fn with_floor_slope(mut self, slope: [f32; 2]) -> LabStrand {
        self.floor_slope[..2].copy_from_slice(&slope);
        self
    }

    /// Replaces the floor with a step `height_px` high for x > `edge_px`, on which the
    /// strand rests as a hair `radius_px` thick.
    pub(super) fn with_step(mut self, edge_px: f32, height_px: f32, radius_px: f32) -> LabStrand {
        self.step = [edge_px, height_px, radius_px, 1.0];
        self
    }

    pub(super) fn with_curl(mut self, curl: [f32; 3]) -> LabStrand {
        self.curl[..3].copy_from_slice(&curl);
        self
    }

    pub(super) fn with_tip_force(mut self, force_n: [f32; 3]) -> LabStrand {
        self.tip_force[..3].copy_from_slice(&force_n);
        self
    }

    /// Moves the whole strand by `slide_px` along x and y over the press.
    pub(super) fn with_slide(mut self, slide_px: [f32; 2]) -> LabStrand {
        self.floor_slope[2..].copy_from_slice(&slide_px);
        self
    }

    /// Lowers the whole strand by `press_px` in `steps`, solving after each.
    pub(super) fn with_press(mut self, press_px: f32, steps: u32) -> LabStrand {
        self.curl[3] = press_px;
        self.tip_force[3] = steps as f32;
        self
    }

    pub(super) fn with_friction(
        mut self,
        static_mu: f32,
        kinetic_mu: f32,
        anisotropy: f32,
    ) -> LabStrand {
        self.friction[..3].copy_from_slice(&[static_mu, kinetic_mu, anisotropy]);
        self
    }

    /// Creeps the plastic shifts at `per_second`, each step lasting `step_seconds`.
    pub(super) fn with_creep(mut self, per_second: f32, step_seconds: f32) -> LabStrand {
        self.friction[3] = per_second;
        self.timing[0] = step_seconds;
        self.timing[1] = step_seconds;
        self
    }

    /// How long each drag step lasts, for the creep.
    pub(super) fn with_drag_seconds(mut self, step_seconds: f32) -> LabStrand {
        self.timing[1] = step_seconds;
        self
    }

    /// After the press, moves the whole strand by `drag_px` in `steps`, solving after each.
    pub(super) fn with_drag(mut self, drag_px: [f32; 2], steps: u32) -> LabStrand {
        self.drag[..2].copy_from_slice(&drag_px);
        self.drag[2] = steps as f32;
        self
    }

    /// After the drag, raises the whole strand by `rise_px` in `steps`, solving after each.
    pub(super) fn with_settle(mut self, rise_px: f32, steps: u32) -> LabStrand {
        self.timing[2] = steps as f32;
        self.timing[3] = rise_px;
        self
    }

    /// Drops the tip load for the drag phase.
    pub(super) fn released_for_the_drag(mut self) -> LabStrand {
        self.drag[3] = 1.0;
        self
    }
}

pub(super) struct LabRun {
    pub(super) chain_px: Vec<[f32; 3]>,
    /// The canvas's normal force on each vertex, N; the root's is 0.
    pub(super) vertex_forces_n: Vec<f32>,
    pub(super) normal_force_n: f32,
    /// The canvas's friction on each vertex, N, world; the root's is 0.
    pub(super) friction_n: Vec<[f32; 3]>,
    /// Each vertex's anchor mode: 0 free, > 0 stuck, < 0 sliding (the normal force).
    pub(super) anchor_modes: Vec<f32>,
    /// Each hinge's plastic shift, world, rad.
    pub(super) plastic: Vec<[f32; 3]>,
    /// How far each hinge's turn stands from its rest turn plus shift, rad.
    pub(super) excess_rad: Vec<f32>,
    /// Where friction holds each vertex, px; zero where it holds none.
    pub(super) anchors_px: Vec<[f32; 3]>,
    /// Each hinge's excess over the yield, rad, summed over the steps that crept.
    pub(super) crept_excess_rad: Vec<f32>,
}

fn lab_stage(gpu: &GpuContext) -> (ComputeStage, wgpu::ComputePipeline) {
    let source = shader_source(
        &LAB_SNIPPETS,
        include_str!("../../shaders/bristle_lab.wgsl"),
    );
    let stage = ComputeStage::new(
        gpu,
        "bristle_lab",
        &source,
        &[storage_entry(0, true), storage_entry(1, false)],
    );
    let pipeline = stage.pipeline(gpu, "bristle_lab", "solve_lab");
    (stage, pipeline)
}

pub(super) fn run(gpu: &GpuContext, strands: &[LabStrand]) -> Vec<LabRun> {
    let (stage, pipeline) = lab_stage(gpu);
    let input = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bristle_lab_strands"),
            contents: bytemuck::cast_slice(strands),
            usage: wgpu::BufferUsages::STORAGE,
        });
    let out_bytes =
        (strands.len() * OUT_VECTORS * VECTOR_FLOATS * std::mem::size_of::<f32>()) as u64;
    let output = storage_buffer(gpu, "bristle_lab_out", out_bytes);
    let bind_group = stage.bind_group(
        gpu,
        &[buffer_binding(0, &input), buffer_binding(1, &output)],
    );
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(strands.len() as u32, 1, 1);
    }
    gpu.queue.submit([encoder.finish()]);
    let floats = read_buffer(gpu, &output).unwrap();
    floats
        .chunks(OUT_VECTORS * VECTOR_FLOATS)
        .map(lab_run)
        .collect()
}

fn lab_run(floats: &[f32]) -> LabRun {
    let vectors: Vec<&[f32]> = floats.chunks(VECTOR_FLOATS).collect();
    let friction = &vectors[BRISTLE_VERTICES + 1..2 * BRISTLE_VERTICES + 1];
    let plastic = &vectors[2 * BRISTLE_VERTICES + 1..2 * BRISTLE_VERTICES + 1 + HINGES];
    let anchors = &vectors[2 * BRISTLE_VERTICES + 1 + HINGES..3 * BRISTLE_VERTICES + 1 + HINGES];
    let crept_excess = &vectors[3 * BRISTLE_VERTICES + 1 + HINGES..];
    LabRun {
        chain_px: vectors[..BRISTLE_VERTICES]
            .iter()
            .map(|v| [v[0], v[1], v[2]])
            .collect(),
        vertex_forces_n: vectors[..BRISTLE_VERTICES].iter().map(|v| v[3]).collect(),
        normal_force_n: vectors[BRISTLE_VERTICES][0],
        friction_n: friction.iter().map(|v| [v[0], v[1], v[2]]).collect(),
        anchor_modes: friction.iter().map(|v| v[3]).collect(),
        plastic: plastic.iter().map(|v| [v[0], v[1], v[2]]).collect(),
        excess_rad: plastic.iter().map(|v| v[3]).collect(),
        anchors_px: anchors.iter().map(|v| [v[0], v[1], v[2]]).collect(),
        crept_excess_rad: crept_excess.iter().map(|v| v[0]).collect(),
    }
}

mod tests;
