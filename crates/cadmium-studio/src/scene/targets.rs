//! The scene's render targets and how much they supersample.

use cadmium_engine::gpu::GpuContext;

use super::{COLOUR_FORMAT, DEPTH_FORMAT};

/// The most the scene supersamples, per axis.
const MAX_SUPERSAMPLE: f32 = 2.0;
/// The largest target side the supersampling may reach, on any device.
const MAX_TARGET_SIDE: u32 = 4096;

/// The render targets, at one displayed size.
pub(super) struct Targets {
    pub(super) size: (u32, u32),
    pub(super) supersample: f32,
    pub(super) colour: wgpu::TextureView,
    pub(super) depth: wgpu::TextureView,
}

impl Targets {
    pub(super) fn new(gpu: &GpuContext, size: (u32, u32)) -> Targets {
        let max_side = largest_side(gpu);
        let supersample = supersample_for(size, max_side);
        let scaled = |side: u32| ((side as f32 * supersample).ceil() as u32).clamp(1, max_side);
        let extent = (scaled(size.0), scaled(size.1));
        let colour_usage =
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let depth_usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        Targets {
            size,
            supersample,
            colour: target(gpu, extent, COLOUR_FORMAT, colour_usage),
            depth: target(gpu, extent, DEPTH_FORMAT, depth_usage),
        }
    }
}

/// The largest target side: 4096 px, or less if the device cannot.
fn largest_side(gpu: &GpuContext) -> u32 {
    gpu.device
        .limits()
        .max_texture_dimension_2d
        .min(MAX_TARGET_SIDE)
}

/// `min(2, max_side / max(w, h))`, so the target never grows past `max_side`.
fn supersample_for(size: (u32, u32), max_side: u32) -> f32 {
    let longest = size.0.max(size.1).max(1) as f32;
    MAX_SUPERSAMPLE.min(max_side as f32 / longest)
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
