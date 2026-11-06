//! The 3D scene: the studio canvas as a plane, seen by the orbit camera.
//!
//! The scene renders into its own linear `Rgba16Float` target, supersampled,
//! for the engine's presenter to downsample and encode for display.

mod plane;

use cadmium_engine::gpu::GpuContext;

use crate::{OrbitCamera, StudioPlayer};
use plane::PlanePass;

const COLOUR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// The most the scene supersamples, per axis.
const MAX_SUPERSAMPLE: f32 = 2.0;
/// The largest target side the supersampling may reach.
const MAX_TARGET_SIDE: f32 = 4096.0;
/// The backdrop, in linear sRGB.
const CLEAR_COLOUR: wgpu::Color = wgpu::Color {
    r: 0.02,
    g: 0.02,
    b: 0.025,
    a: 1.0,
};

/// What the scene draws besides the canvas.
#[derive(Clone, Copy, Debug)]
pub struct SceneLayers {
    pub brush: bool,
}

/// One frame of the scene.
pub struct SceneRequest<'a> {
    pub player: &'a StudioPlayer,
    pub camera: &'a OrbitCamera,
    /// The displayed size, in px; the target is this times the supersampling.
    pub size: (u32, u32),
    pub layers: SceneLayers,
}

/// The render targets, at one size.
struct Targets {
    size: (u32, u32),
    supersample: f32,
    colour: wgpu::TextureView,
    depth: wgpu::TextureView,
}

impl Targets {
    fn new(gpu: &GpuContext, size: (u32, u32)) -> Targets {
        let supersample = supersample_for(size);
        let scaled = |side: u32| ((side as f32 * supersample).round() as u32).max(1);
        let extent = (scaled(size.0), scaled(size.1));
        let colour_usage =
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        Targets {
            size,
            supersample,
            colour: target(gpu, extent, COLOUR_FORMAT, colour_usage),
            depth: target(
                gpu,
                extent,
                DEPTH_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
        }
    }
}

/// `min(2, 4096 / max(w, h))`, so the target never grows past 4096 px.
fn supersample_for(size: (u32, u32)) -> f32 {
    let longest = size.0.max(size.1).max(1) as f32;
    MAX_SUPERSAMPLE.min(MAX_TARGET_SIDE / longest)
}

fn target(
    gpu: &GpuContext,
    (width, height): (u32, u32),
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::TextureView {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene_target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Triangle lists, both faces drawn: the camera's mirror flips the winding.
fn primitive() -> wgpu::PrimitiveState {
    wgpu::PrimitiveState {
        cull_mode: None,
        ..Default::default()
    }
}

fn depth_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(true),
        depth_compare: Some(wgpu::CompareFunction::Less),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

/// Renders the studio scene into a supersampled linear target.
pub struct SceneRenderer {
    targets: Targets,
    plane: PlanePass,
}

impl SceneRenderer {
    pub fn new(gpu: &GpuContext) -> SceneRenderer {
        SceneRenderer {
            targets: Targets::new(gpu, (1, 1)),
            plane: PlanePass::new(gpu),
        }
    }

    /// The supersampling of the last rendered size, per axis.
    pub fn supersample(&self) -> f32 {
        self.targets.supersample
    }

    /// Encodes one frame into `encoder` and returns the linear colour target.
    ///
    /// The player's compute frames were already submitted, so queue order
    /// has the canvas ready. A player that is not ready shows only the
    /// backdrop.
    pub fn render(
        &mut self,
        gpu: &GpuContext,
        request: SceneRequest<'_>,
        encoder: &mut wgpu::CommandEncoder,
    ) -> &wgpu::TextureView {
        let size = (request.size.0.max(1), request.size.1.max(1));
        if self.targets.size != size {
            self.targets = Targets::new(gpu, size);
        }
        let mut pass = self.begin_pass(encoder);
        if request.player.is_ready() {
            let canvas = &request.player.session().canvas;
            let view_proj = request.camera.view_proj();
            self.plane.draw(gpu, &mut pass, canvas, view_proj);
        }
        drop(pass);
        &self.targets.colour
    }

    fn begin_pass<'e>(&self, encoder: &'e mut wgpu::CommandEncoder) -> wgpu::RenderPass<'e> {
        let colour = wgpu::RenderPassColorAttachment {
            view: &self.targets.colour,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(CLEAR_COLOUR),
                store: wgpu::StoreOp::Store,
            },
        };
        let depth = wgpu::RenderPassDepthStencilAttachment {
            view: &self.targets.depth,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Discard,
            }),
            stencil_ops: None,
        };
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &[Some(colour)],
            depth_stencil_attachment: Some(depth),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }
}
