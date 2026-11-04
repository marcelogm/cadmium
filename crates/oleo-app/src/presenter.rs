use std::time::Instant;

use gtk::prelude::*;
use gtk::{gdk, glib};
use oleo_engine::present::readback_rgba8;

use crate::LOG_DOMAIN;
use crate::state::AppState;
use crate::view::clamp_zoom;

const PRESENT_MODE_VARIABLE: &str = "OLEO_PRESENT";
const RGBA8_BYTES_PER_PIXEL: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentMode {
    Readback,
    Dmabuf,
}

impl PresentMode {
    fn from_name(name: &str) -> Option<PresentMode> {
        match name {
            "readback" => Some(PresentMode::Readback),
            "dmabuf" | "linear" => Some(PresentMode::Dmabuf),
            _ => None,
        }
    }

    fn requested() -> PresentMode {
        let requested = std::env::var(PRESENT_MODE_VARIABLE).ok();
        match requested.as_deref().map(PresentMode::from_name) {
            Some(Some(PresentMode::Dmabuf)) => {
                glib::g_warning!(
                    LOG_DOMAIN,
                    "{PRESENT_MODE_VARIABLE}: dmabuf presentation is not available yet; using readback"
                );
                PresentMode::Readback
            }
            Some(None) => {
                glib::g_warning!(
                    LOG_DOMAIN,
                    "ignoring {PRESENT_MODE_VARIABLE}={:?}",
                    requested.unwrap_or_default()
                );
                PresentMode::Readback
            }
            Some(Some(mode)) => mode,
            None => PresentMode::Readback,
        }
    }
}

pub struct FramePresenter {
    mode: PresentMode,
    target: Option<wgpu::Texture>,
}

impl Default for FramePresenter {
    fn default() -> Self {
        FramePresenter {
            mode: PresentMode::requested(),
            target: None,
        }
    }
}

impl FramePresenter {
    pub fn matches_size(&self, width_px: u32, height_px: u32) -> bool {
        self.target
            .as_ref()
            .is_some_and(|target| target.width() == width_px && target.height() == height_px)
    }

    pub fn present(&mut self, state: &mut AppState, width_px: u32, height_px: u32) -> gdk::Texture {
        let started = Instant::now();
        if !self.matches_size(width_px, height_px) {
            self.target = Some(
                state
                    .session
                    .presenter
                    .create_target(&state.gpu, width_px, height_px),
            );
        }
        let target = self
            .target
            .as_ref()
            .expect("present target was just created");
        state.view.zoom = clamp_zoom(state.view.zoom);
        let gpu = state.gpu.clone();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("oleo_frame"),
            });
        state.encode_frame(&mut encoder);
        state.session.presenter.encode_present(
            &gpu,
            &state.session.canvas,
            state.view,
            target,
            &mut encoder,
        );
        gpu.queue.submit([encoder.finish()]);
        let pixels = match self.mode {
            PresentMode::Readback | PresentMode::Dmabuf => readback_rgba8(&gpu, target),
        };
        let texture = gdk::MemoryTexture::new(
            width_px as i32,
            height_px as i32,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(pixels),
            width_px as usize * RGBA8_BYTES_PER_PIXEL,
        );
        glib::g_debug!(
            LOG_DOMAIN,
            "frame {width_px}×{height_px} presented by {:?} in {:.2} ms",
            self.mode,
            started.elapsed().as_secs_f64() * 1000.0
        );
        texture.upcast()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_mode_names_follow_the_environment_contract() {
        assert_eq!(
            PresentMode::from_name("readback"),
            Some(PresentMode::Readback)
        );
        assert_eq!(PresentMode::from_name("dmabuf"), Some(PresentMode::Dmabuf));
        assert_eq!(PresentMode::from_name("linear"), Some(PresentMode::Dmabuf));
        assert_eq!(PresentMode::from_name("fast"), None);
    }
}
