//! The brush in the scene: its simulated hairs, ferrule and handle, drawn
//! from the engine's live bristle buffers in the scene's pass.
//!
//! The shaders build on the engine's bristle view module. It holds only
//! compile-time constants and functions, no runtime parameters, so the
//! pipelines are built once, up front, and serve every engine.

use cadmium_engine::engine::BristleView;
use cadmium_engine::gpu::GpuContext;

use super::hairs::HairPass;
use super::meshes::MeshPass;
use super::view::SceneView;
use super::{COLOUR_FORMAT, layout_entry};
use crate::{SceneRequest, hair_tint};

/// Draws the brush.
pub(super) struct BrushPass {
    hairs: HairPass,
    meshes: MeshPass,
}

impl BrushPass {
    pub(super) fn new(gpu: &GpuContext, view: &SceneView) -> BrushPass {
        BrushPass {
            hairs: HairPass::new(gpu, view),
            meshes: MeshPass::new(gpu, view),
        }
    }

    /// Draws the player's brush, if the layer is on and the brush on show and
    /// in hand; the camera is already bound as group 0.
    pub(super) fn draw(
        &mut self,
        gpu: &GpuContext,
        pass: &mut wgpu::RenderPass<'_>,
        request: SceneRequest<'_>,
    ) {
        let player = request.player;
        if !request.layers.brush || !player.brush_visible() {
            return;
        }
        let bristles = player.session().engine.bristle_view();
        if bristles.count() == 0 || !bristles.in_hand() {
            return;
        }
        self.meshes.draw(gpu, pass, &bristles);
        let tint = hair_tint(&player.brush().hair);
        self.hairs.draw(gpu, pass, &bristles, tint);
    }
}

/// A read-only storage buffer, as the vertex stage may bind one.
pub(super) fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    let ty = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Storage { read_only: true },
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    layout_entry(binding, ty)
}

pub(super) fn buffer_entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

/// What a brush pipeline is built from: `body` follows the bristle module.
pub(super) struct BrushShader<'a> {
    pub(super) label: &'static str,
    pub(super) body: &'static str,
    pub(super) layout: &'a wgpu::PipelineLayout,
    pub(super) buffers: &'a [Option<wgpu::VertexBufferLayout<'a>>],
}

pub(super) fn brush_pipeline(gpu: &GpuContext, shader: BrushShader<'_>) -> wgpu::RenderPipeline {
    let source = format!("{}\n{}", BristleView::wgsl_source(), shader.body);
    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(shader.label),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(shader.label),
            layout: Some(shader.layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: shader.buffers,
            },
            primitive: super::primitive(),
            depth_stencil: Some(super::depth_state()),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(COLOUR_FORMAT.into())],
            }),
            multiview_mask: None,
            cache: None,
        })
}
