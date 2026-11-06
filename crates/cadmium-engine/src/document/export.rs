use std::path::Path;

use crate::gpu::{GpuContext, GpuLost, new_encoder};
use crate::present::{PresentRequest, ViewTransform, readback_rgba8};
use crate::render::ViewMode;
use crate::session::PaintSession;

use super::DocumentError;
use super::write::write_replacing;

const RGBA_CHANNELS: usize = 4;
const ALPHA_CHANNEL: usize = 3;
const OPAQUE_ALPHA: u8 = 255;
const UNSCALED_VIEW: ViewTransform = ViewTransform {
    origin_x: 0.0,
    origin_y: 0.0,
    zoom: 1.0,
    rotation: 0.0,
};

pub struct ExportedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl ExportedImage {
    pub fn render(gpu: &GpuContext, session: &mut PaintSession) -> Result<ExportedImage, GpuLost> {
        let (width, height) = (session.canvas.spec.width, session.canvas.spec.height);
        let chosen_mode = session.view_mode;
        session.view_mode = ViewMode::Color;
        session.canvas.dirty.mark_all();
        let target = session.presenter.create_target(gpu, width, height);
        let mut encoder = new_encoder(gpu, "export_png");
        session.encode_still_frame(gpu, &mut encoder);
        session.presenter.encode_present(
            gpu,
            PresentRequest::of_canvas(&session.canvas, UNSCALED_VIEW),
            &target,
            &mut encoder,
        );
        gpu.queue.submit([encoder.finish()]);
        let rgba = readback_rgba8(gpu, &target);
        session.view_mode = chosen_mode;
        if chosen_mode != ViewMode::Color {
            session.canvas.dirty.mark_all();
        }
        let mut rgba = rgba?;
        for pixel in rgba.as_chunks_mut::<RGBA_CHANNELS>().0 {
            pixel[ALPHA_CHANNEL] = OPAQUE_ALPHA;
        }
        Ok(ExportedImage {
            width,
            height,
            rgba,
        })
    }

    pub fn write_png(&self, path: &Path) -> Result<(), DocumentError> {
        write_replacing(path, |file| {
            let mut encoder = png::Encoder::new(file, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&self.rgba)?;
            writer.finish()?;
            Ok(())
        })
    }
}
