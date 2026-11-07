use wgpu::util::DeviceExt;

use super::{DabResources, PlacedDab, bind_at_dab, dab_record_binding, dab_record_entry, dispatch};
use crate::canvas::StateTexture;
use crate::compute::{
    ComputeStage, buffer_binding, storage_buffer, storage_entry, texture_binding, texture_entry,
    uniform_entry,
};
use crate::engine::dab_record::{KNIFE_LAID_ALTITUDE_RAD, KNIFE_UPRIGHT_ALTITUDE_RAD};
use crate::engine::face_load::{
    FACE_CAPACITY_UM, FACE_CELLS, FACE_COLUMN_PX, FACE_DEPTH, FACE_GRANT_CHANNELS,
};
use crate::engine::shader::{BLADE_SNIPPETS, shader_source};
use crate::gpu::GpuContext;

const FACE_STATE_BYTES: u64 = 2 * std::mem::size_of::<f32>() as u64;
const FACE_GRANT_BYTES: u64 =
    (FACE_CELLS * FACE_GRANT_CHANNELS * std::mem::size_of::<f32>()) as u64;
const CELL_GROUP_SIDE: u32 = 8;
const COLUMN_GROUPS: u32 = (FACE_DEPTH as u32).div_ceil(CELL_GROUP_SIDE);
const PAIR_GROUPS: u32 = (FACE_DEPTH as u32 / 2).div_ceil(CELL_GROUP_SIDE);

struct FacePipelines {
    rest: wgpu::ComputePipeline,
    want: wgpu::ComputePipeline,
    grant: wgpu::ComputePipeline,
    exchange: wgpu::ComputePipeline,
    settle: wgpu::ComputePipeline,
    smear_across: [wgpu::ComputePipeline; 2],
    smear_along: [wgpu::ComputePipeline; 2],
}

impl FacePipelines {
    fn new(gpu: &GpuContext, compute: &ComputeStage) -> FacePipelines {
        let pipeline = |entry: &str| compute.pipeline(gpu, &format!("face_{entry}"), entry);
        FacePipelines {
            rest: pipeline("rest"),
            want: pipeline("want"),
            grant: pipeline("grant"),
            exchange: pipeline("exchange"),
            settle: pipeline("settle"),
            smear_across: ["smear_across_even", "smear_across_odd"].map(pipeline),
            smear_along: ["smear_along_even", "smear_along_odd"].map(pipeline),
        }
    }
}

fn face_shader() -> String {
    let constants = format!(
        "const FACE_DEPTH: u32 = {FACE_DEPTH}u;\n\
         const FACE_GRANT_CHANNELS: u32 = {FACE_GRANT_CHANNELS}u;\n\
         const FACE_CAPACITY_UM: f32 = {FACE_CAPACITY_UM:?};\n\
         const FACE_COLUMN_PX: f32 = {FACE_COLUMN_PX:?};\n\
         const FACE_LAID_ALTITUDE_RAD: f32 = {KNIFE_LAID_ALTITUDE_RAD:?};\n\
         const FACE_UPRIGHT_ALTITUDE_RAD: f32 = {KNIFE_UPRIGHT_ALTITUDE_RAD:?};\n"
    );
    shader_source(
        &BLADE_SNIPPETS,
        &(constants
            + include_str!("../../shaders/face_edge.wgsl")
            + include_str!("../../shaders/face.wgsl")),
    )
}

pub(super) struct FacePass {
    compute: ComputeStage,
    pipelines: FacePipelines,
    face_grant: wgpu::Buffer,
    face_state: wgpu::Buffer,
}

impl FacePass {
    pub(super) fn new(gpu: &GpuContext) -> FacePass {
        use wgpu::StorageTextureAccess::{ReadOnly, ReadWrite};
        use wgpu::TextureFormat::{R32Float, Rgba16Float};

        let compute = ComputeStage::new(
            gpu,
            "face",
            &face_shader(),
            &[
                dab_record_entry(),
                texture_entry(1, ReadWrite, Rgba16Float),
                texture_entry(2, ReadWrite, Rgba16Float),
                texture_entry(3, ReadWrite, R32Float),
                texture_entry(4, ReadOnly, R32Float),
                storage_entry(5, false),
                storage_entry(6, false),
                storage_entry(7, false),
                storage_entry(8, false),
                uniform_entry(9, None),
            ],
        );
        FacePass {
            pipelines: FacePipelines::new(gpu, &compute),
            face_grant: storage_buffer(gpu, "mixing_knife_face_grant", FACE_GRANT_BYTES),
            face_state: storage_buffer(gpu, "mixing_knife_face_state", FACE_STATE_BYTES),
            compute,
        }
    }

    pub(super) fn bind_group(&self, resources: &DabResources<'_>) -> wgpu::BindGroup {
        let canvas = resources.canvas;
        // A buffer of its own for every batch, as the dab records have: one submit
        // can carry several strokes, and a buffer rewritten in place would hand
        // them all the last stroke's seed.
        let settings = resources
            .gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mixing_knife_face_settings"),
                contents: bytemuck::bytes_of(&resources.face_settings),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        self.compute.bind_group(
            resources.gpu,
            &[
                dab_record_binding(resources.dab_records),
                texture_binding(1, canvas, StateTexture::WetPig0),
                texture_binding(2, canvas, StateTexture::WetPig1),
                texture_binding(3, canvas, StateTexture::WetVol),
                texture_binding(4, canvas, StateTexture::DryHeight),
                buffer_binding(5, &resources.buffers.face_load),
                buffer_binding(6, &resources.buffers.face_delta),
                buffer_binding(7, &self.face_grant),
                buffer_binding(8, &self.face_state),
                buffer_binding(9, &settings),
            ],
        )
    }

    pub(super) fn encode(
        &self,
        pass: &mut wgpu::ComputePass<'_>,
        bind_group: &wgpu::BindGroup,
        dab: PlacedDab<'_>,
    ) {
        let face = &self.pipelines;
        let [width, height] = dab.placement.canvas_groups;
        let rows = touched_rows(dab.placement.record.radius);
        let row_groups = rows.div_ceil(CELL_GROUP_SIDE);
        let pair_row_groups = rows.div_ceil(2).div_ceil(CELL_GROUP_SIDE);
        bind_at_dab(pass, bind_group, dab);
        for (pipeline, groups) in [
            (&face.rest, [1, 1]),
            (&face.want, [width, height]),
            (&face.grant, [COLUMN_GROUPS, row_groups]),
            (&face.exchange, [width, height]),
            (&face.settle, [COLUMN_GROUPS, row_groups]),
            (&face.smear_across[0], [PAIR_GROUPS, row_groups]),
            (&face.smear_across[1], [PAIR_GROUPS, row_groups]),
            (&face.smear_along[0], [COLUMN_GROUPS, pair_row_groups]),
            (&face.smear_along[1], [COLUMN_GROUPS, pair_row_groups]),
        ] {
            dispatch(pass, pipeline, groups);
        }
    }
}

/// The face rows a blade of `radius` can touch, as `touched_cell` keeps them in the
/// shader: its reach either side of the middle row, plus that row.
fn touched_rows(radius: f32) -> u32 {
    let reach = radius.ceil() as u32 + 1;
    2 * reach + 1
}
