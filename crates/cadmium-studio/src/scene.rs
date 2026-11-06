//! The 3D scene: the studio canvas as a plane, seen by the orbit camera.
//!
//! The scene renders into its own linear `Rgba16Float` target, supersampled,
//! for the engine's presenter to downsample and encode for display.

mod plane;
mod targets;

use cadmium_engine::gpu::GpuContext;
use cadmium_engine::present::{PresentRequest, ViewTransform};

use crate::{OrbitCamera, StudioPlayer};
use plane::PlanePass;
use targets::Targets;

const COLOUR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
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
    /// The brush's hairs, ferrule and handle: drawn from Task 8b; ignored by
    /// the plane.
    pub brush: bool,
}

/// One frame of the scene.
#[derive(Clone, Copy)]
pub struct SceneRequest<'a> {
    pub player: &'a StudioPlayer,
    pub camera: &'a OrbitCamera,
    /// The displayed size, in px; the target is this times the supersampling.
    pub size: (u32, u32),
    pub layers: SceneLayers,
}

/// A rendered frame: the linear colour target, supersampled.
pub struct SceneFrame<'a> {
    pub colour: &'a wgpu::TextureView,
    /// Target texels per displayed pixel, per axis.
    pub supersample: f32,
}

impl<'a> SceneFrame<'a> {
    /// Presents the frame at its displayed size, from the target's origin.
    pub fn present_request(&self) -> PresentRequest<'a> {
        PresentRequest {
            source: self.colour,
            view: ViewTransform {
                origin_x: 0.0,
                origin_y: 0.0,
                zoom: 1.0 / self.supersample,
            },
        }
    }
}

/// Triangle lists, both faces drawn: the camera's mirror flips the winding.
fn primitive() -> wgpu::PrimitiveState {
    wgpu::PrimitiveState {
        cull_mode: None,
        ..Default::default()
    }
}

/// Reverse-Z (see the camera): nearer is larger, so `Greater` wins and the
/// depth clears to 0, infinity.
///
/// The canvas top face lies at z = 0 and everything drawn on it (the hairs)
/// stays at z ≥ 0, so a hair touching the canvas is never hidden by it.
fn depth_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(true),
        depth_compare: Some(wgpu::CompareFunction::Greater),
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

    /// Encodes one frame into `encoder` and returns it, for the presenter.
    ///
    /// The player's compute frames were already submitted, so queue order
    /// has the canvas ready. A player that is not ready shows only the
    /// backdrop.
    pub fn render(
        &mut self,
        gpu: &GpuContext,
        request: SceneRequest<'_>,
        encoder: &mut wgpu::CommandEncoder,
    ) -> SceneFrame<'_> {
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
        SceneFrame {
            colour: &self.targets.colour,
            supersample: self.targets.supersample,
        }
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
                load: wgpu::LoadOp::Clear(0.0),
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
