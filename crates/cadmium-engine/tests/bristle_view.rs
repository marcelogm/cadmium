pub mod support;

use cadmium_engine::brush::HeadSpec;
use cadmium_engine::engine::{BristleSnapshot, BristleView, ferrule_radius_for};
use cadmium_engine::gpu::{GpuContext, read_mapped, staging_buffer};
use cadmium_engine::session::PaintSession;
use cadmium_engine::stroke::Tool;
use support::{session, stroke, without_wgpu_errors};

const VERTEX_FLOATS: usize = 4;
const VERTEX_BYTES: u64 = 16;
const WORKGROUP_SIZE: u32 = 64;
const EXTRA_SLOTS: usize = 3;
const PROBE: &str = r#"
const EXTRA_SLOTS: u32 = 3u;
@group(0) @binding(0) var<storage, read> states: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> handle: HandleState;
@group(0) @binding(2) var<storage, read_write> out: array<vec4<f32>>;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x == 0u) {
        let pose = pose_of(handle);
        out[0] = vec4<f32>(pose.ferrule, 0.0);
        out[1] = vec4<f32>(pose.axis, 0.0);
        out[2] = vec4<f32>(f32(BRISTLE_VERTICES), 0.0, 0.0, 0.0);
    }
    if (id.x >= arrayLength(&out) - EXTRA_SLOTS) { return; }
    out[EXTRA_SLOTS + id.x] = bristle_vertex(id.x / BRISTLE_VERTICES, id.x % BRISTLE_VERTICES);
}
"#;
const AXIS_TOLERANCE: f32 = 1e-5;
const ROOT_TOLERANCE_PX: f32 = 1e-3;
const LARGE_DIAMETER_PX: f32 = 120.0;
const SMALL_DIAMETER_PX: f32 = 12.0;

struct Probe {
    ferrule_px: [f32; 3],
    axis: [f32; 3],
    vertices_per_bristle: usize,
    vertices: Vec<[f32; 4]>,
}

fn painted(gpu: &GpuContext, head: HeadSpec) -> PaintSession {
    let mut s = session(gpu);
    s.brush.head = head;
    stroke(gpu, &mut s, 128.0, 100.0, 400.0);
    s
}

fn storage(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn probe_layout(gpu: &GpuContext) -> wgpu::BindGroupLayout {
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bristle_view_probe"),
            entries: &[storage(0, true), storage(1, true), storage(2, false)],
        })
}

fn probe_pipeline(
    gpu: &GpuContext,
    view: &BristleView<'_>,
    bind_layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bristle_view_probe"),
            bind_group_layouts: &[Some(bind_layout)],
            immediate_size: 0,
        });
    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bristle_view_probe"),
            source: wgpu::ShaderSource::Wgsl(format!("{}\n{PROBE}", view.wgsl()).into()),
        });
    gpu.device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("bristle_view_probe"),
            layout: Some(&layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
}

fn probe_floats(gpu: &GpuContext, view: &BristleView<'_>, per_bristle: usize) -> Vec<f32> {
    let bind_layout = probe_layout(gpu);
    let pipeline = probe_pipeline(gpu, view, &bind_layout);
    let vertices = view.count() * per_bristle as u32;
    let bytes = (u64::from(vertices) + EXTRA_SLOTS as u64) * VERTEX_BYTES;
    let out = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bristle_view_out"),
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let staging = staging_buffer(gpu, "bristle_view_staging", bytes);
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bristle_view_probe"),
        layout: &bind_layout,
        entries: &[
            entry(0, view.states()),
            entry(1, view.handle()),
            entry(2, &out),
        ],
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let mut pass = encoder.begin_compute_pass(&Default::default());
    pass.set_pipeline(&pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.dispatch_workgroups(vertices.div_ceil(WORKGROUP_SIZE), 1, 1);
    drop(pass);
    encoder.copy_buffer_to_buffer(&out, 0, &staging, 0, bytes);
    gpu.queue.submit([encoder.finish()]);
    read_mapped(gpu, &staging, |bytes| bytemuck::cast_slice(bytes).to_vec()).unwrap()
}

fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

fn probe(gpu: &GpuContext, view: &BristleView<'_>, per_bristle: usize) -> Probe {
    let floats = without_wgpu_errors(gpu, || probe_floats(gpu, view, per_bristle));
    let (slots, _) = floats.as_chunks::<VERTEX_FLOATS>();
    let xyz = |slot: [f32; 4]| [slot[0], slot[1], slot[2]];
    Probe {
        ferrule_px: xyz(slots[0]),
        axis: xyz(slots[1]),
        vertices_per_bristle: slots[2][0] as usize,
        vertices: slots[EXTRA_SLOTS..].to_vec(),
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn assert_vertices_match(probe: &Probe, snapshot: &BristleSnapshot) {
    let per_bristle = probe.vertices_per_bristle;
    for (b, chain) in snapshot.chains_px.iter().enumerate() {
        assert_eq!(chain.len(), per_bristle);
        for (v, expected) in chain.iter().enumerate() {
            let [x, y, z, _] = probe.vertices[b * per_bristle + v];
            assert_eq!(&[x, y, z], expected, "bristle {b} vertex {v}");
        }
    }
    let lowest_clearance_px = probe
        .vertices
        .iter()
        .map(|[_, _, height, floor]| height - floor)
        .fold(f32::INFINITY, f32::min);
    assert_eq!(lowest_clearance_px, snapshot.lowest_clearance_px);
}

fn assert_roots_hang_from_the_pose(probe: &Probe, snapshot: &BristleSnapshot, radius_px: f32) {
    let axis = probe.axis;
    assert!(
        (dot(axis, axis).sqrt() - 1.0).abs() < AXIS_TOLERANCE,
        "{axis:?}"
    );
    assert!(axis[2] > 0.0, "{axis:?}");
    for chain in &snapshot.chains_px {
        let offset = [0, 1, 2].map(|i| chain[0][i] - probe.ferrule_px[i]);
        assert!(dot(offset, axis).abs() < ROOT_TOLERANCE_PX, "{offset:?}");
        assert!(dot(offset, offset).sqrt() <= radius_px + ROOT_TOLERANCE_PX);
    }
}

fn assert_view_matches_snapshot(gpu: &GpuContext, s: &PaintSession) {
    let snapshot = s.engine.bristle_snapshot(gpu).unwrap();
    let view = s.engine.bristle_view();
    assert!(view.count() > 0);
    assert_eq!(view.count() as usize, snapshot.radii_px.len());
    let probe = probe(gpu, &view, snapshot.chains_px[0].len());
    assert_vertices_match(&probe, &snapshot);
    assert_roots_hang_from_the_pose(&probe, &snapshot, view.ferrule_radius_px());
}

fn painted_at(gpu: &GpuContext, s: &mut PaintSession, diameter_px: f32) {
    s.brush.geometry.max_diameter_px = diameter_px;
    s.reload_brush(gpu);
    stroke(gpu, s, 128.0, 100.0, 400.0);
}

#[test]
fn the_view_module_reads_the_same_bristles_as_the_snapshot() {
    let gpu = GpuContext::new().unwrap();
    let s = painted(&gpu, HeadSpec::ROUND);
    assert_view_matches_snapshot(&gpu, &s);
}

#[test]
fn the_view_follows_a_refit_to_fewer_bristles() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    painted_at(&gpu, &mut s, LARGE_DIAMETER_PX);
    let large = s.engine.bristle_view().count();
    painted_at(&gpu, &mut s, SMALL_DIAMETER_PX);
    assert!(s.engine.bristle_view().count() < large);
    assert_view_matches_snapshot(&gpu, &s);
}

#[test]
fn the_view_reports_the_brush_ferrule_radius() {
    let gpu = GpuContext::new().unwrap();
    let s = painted(&gpu, HeadSpec::ROUND);
    let expected = ferrule_radius_for(s.brush.geometry.max_diameter_px);
    assert_eq!(s.engine.bristle_view().ferrule_radius_px(), expected);
}

#[test]
fn a_flat_head_roots_narrower_across_than_a_round_one() {
    let gpu = GpuContext::new().unwrap();
    let round = painted(&gpu, HeadSpec::ROUND);
    let flat = painted(&gpu, HeadSpec::FLAT);
    let ferrule_px = round.engine.bristle_view().ferrule_radius_px();
    let [round_x, round_y] = round.engine.bristle_view().root_half_extent_px();
    let [flat_x, flat_y] = flat.engine.bristle_view().root_half_extent_px();
    for extent in [round_x, round_y, flat_x, flat_y] {
        assert!(extent > 0.0, "{extent}");
    }
    assert!(round_x <= ferrule_px && flat_x <= ferrule_px);
    assert!(flat_y < round_y, "flat {flat_y} round {round_y}");
}

#[test]
fn the_bristles_leave_the_hand_when_a_knife_paints() {
    let gpu = GpuContext::new().unwrap();
    let mut s = session(&gpu);
    assert!(!s.engine.bristle_view().in_hand());
    stroke(&gpu, &mut s, 128.0, 100.0, 400.0);
    assert!(s.engine.bristle_view().in_hand());
    s.brush.tool = Tool::Knife;
    stroke(&gpu, &mut s, 64.0, 100.0, 400.0);
    assert!(!s.engine.bristle_view().in_hand());
}
