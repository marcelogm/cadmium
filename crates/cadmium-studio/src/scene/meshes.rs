//! The ferrule and the handle, posed from the engine's handle state.

use cadmium_engine::engine::BristleView;
use cadmium_engine::gpu::GpuContext;
use wgpu::util::DeviceExt;

use super::brush::{BrushShader, brush_pipeline, buffer_entry, storage_entry};
use super::view::SceneView;
use super::{layout_entry, uniform_binding};
use crate::{Mesh, MeshVertex, ferrule_mesh, handle_mesh};

/// Metallic grey with a tight, bright highlight.
const FERRULE: MeshParams = MeshParams {
    colour: [0.6, 0.6, 0.62, 1.0],
    highlight: [0.8, 48.0, 0.0, 0.0],
};
/// Dark red lacquer with a soft sheen.
const HANDLE: MeshParams = MeshParams {
    colour: [0.25, 0.03, 0.02, 1.0],
    highlight: [0.3, 24.0, 0.0, 0.0],
};
const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshParams {
    colour: [f32; 4],
    highlight: [f32; 4],
}

/// One mesh on the device, with its look.
struct MeshBuffers {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    params: wgpu::Buffer,
}

impl MeshBuffers {
    fn new(gpu: &GpuContext, mesh: &Mesh, params: MeshParams) -> MeshBuffers {
        let buffer = |label, contents, usage| {
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents,
                    usage,
                })
        };
        MeshBuffers {
            vertices: buffer(
                "scene_mesh_vertices",
                bytemuck::cast_slice(&mesh.vertices),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: buffer(
                "scene_mesh_indices",
                bytemuck::cast_slice(&mesh.indices),
                wgpu::BufferUsages::INDEX,
            ),
            index_count: mesh.indices.len() as u32,
            params: buffer(
                "scene_mesh_params",
                bytemuck::bytes_of(&params),
                wgpu::BufferUsages::UNIFORM,
            ),
        }
    }
}

/// The meshes for one layout's sizes: root half extents and ferrule radius.
struct Shapes {
    sizes: ([f32; 2], f32),
    ferrule: MeshBuffers,
    handle: MeshBuffers,
}

impl Shapes {
    fn new(gpu: &GpuContext, (extent, radius): ([f32; 2], f32)) -> Shapes {
        Shapes {
            sizes: (extent, radius),
            ferrule: MeshBuffers::new(gpu, &ferrule_mesh(extent, radius), FERRULE),
            handle: MeshBuffers::new(gpu, &handle_mesh(radius), HANDLE),
        }
    }
}

/// Draws the ferrule and the handle.
pub(super) struct MeshPass {
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    pipeline: Option<wgpu::RenderPipeline>,
    shapes: Option<Shapes>,
}

impl MeshPass {
    pub(super) fn new(gpu: &GpuContext, view: &SceneView) -> MeshPass {
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("scene_meshes"),
                entries: &[
                    layout_entry(0, uniform_binding()),
                    storage_entry(1),
                    storage_entry(2),
                ],
            });
        MeshPass {
            pipeline_layout: super::pipeline_layout(gpu, view, &layout),
            layout,
            pipeline: None,
            shapes: None,
        }
    }

    /// Draws the ferrule and the handle where the handle state holds them.
    pub(super) fn draw(
        &mut self,
        gpu: &GpuContext,
        pass: &mut wgpu::RenderPass<'_>,
        bristles: &BristleView<'_>,
    ) {
        let sizes = (bristles.root_half_extent_px(), bristles.ferrule_radius_px());
        if self.shapes.as_ref().is_none_or(|s| s.sizes != sizes) {
            self.shapes = Some(Shapes::new(gpu, sizes));
        }
        let layout = &self.pipeline_layout;
        let pipeline = self.pipeline.get_or_insert_with(|| {
            let shader = BrushShader {
                label: "scene_meshes",
                body: include_str!("../shaders/mesh.wgsl"),
                layout,
                buffers: &[Some(vertex_layout())],
            };
            brush_pipeline(gpu, bristles, shader)
        });
        pass.set_pipeline(pipeline);
        let shapes = self.shapes.as_ref().expect("the shapes were just built");
        for mesh in [&shapes.ferrule, &shapes.handle] {
            let bind_group = bind_group(gpu, &self.layout, mesh, bristles);
            pass.set_bind_group(1, &bind_group, &[]);
            pass.set_vertex_buffer(0, mesh.vertices.slice(..));
            pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }
}

fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: size_of::<MeshVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &VERTEX_ATTRIBUTES,
    }
}

fn bind_group(
    gpu: &GpuContext,
    layout: &wgpu::BindGroupLayout,
    mesh: &MeshBuffers,
    bristles: &BristleView<'_>,
) -> wgpu::BindGroup {
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scene_meshes"),
        layout,
        entries: &[
            buffer_entry(0, &mesh.params),
            buffer_entry(1, bristles.handle()),
            buffer_entry(2, bristles.states()),
        ],
    })
}
