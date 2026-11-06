use std::time::Instant;

use cadmium_engine::gpu::GpuContext;
use cadmium_engine::present::{PresentRequest, ViewTransform};
use cadmium_engine::surface::{TRAY_HEIGHT, TRAY_WIDTH};
use gtk::prelude::*;
use gtk::{gdk, glib};

use self::by_dmabuf::ResizeDebounce;
use self::by_readback::ReadbackTarget;
use self::mode::{PresentMode, choose_mode, describe_modifier, mode_reason};
use crate::LOG_DOMAIN;
use crate::dmabuf::{DmabufError, DmabufPool, ModifierPolicy};
use crate::state::AppState;
use crate::view::clamp_zoom;

const DEBUG_VARIABLE: &str = "CADMIUM_DEBUG";
const DEBUG_ENABLED: &str = "1";
const MILLISECONDS_PER_SECOND: f64 = 1000.0;

mod by_dmabuf;
mod by_readback;
pub(crate) mod mode;

pub fn tray_view(viewport_px: (u32, u32)) -> ViewTransform {
    let zoom =
        (viewport_px.0 as f32 / TRAY_WIDTH as f32).min(viewport_px.1 as f32 / TRAY_HEIGHT as f32);
    ViewTransform {
        origin_x: 0.0,
        origin_y: 0.0,
        zoom,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresentSource {
    #[default]
    Painting,
    Tray,
    /// The brush studio's 3D scene; presented only while the studio is open.
    Studio,
}

impl PresentSource {
    fn encode(
        self,
        state: &mut AppState,
        gpu: &GpuContext,
        target: &wgpu::Texture,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        match self {
            PresentSource::Painting => {
                state.encode_frame(encoder);
                state.session.presenter.encode_present(
                    gpu,
                    PresentRequest::of_canvas(&state.session.canvas, state.view),
                    target,
                    encoder,
                );
            }
            PresentSource::Tray => {
                state.encode_tray_frame(encoder);
                let view = tray_view((target.width(), target.height()));
                state.session.presenter.encode_present(
                    gpu,
                    PresentRequest::of_canvas(&state.session.tray, view),
                    target,
                    encoder,
                );
            }
            PresentSource::Studio => state.encode_studio_present(gpu, target, encoder),
        }
    }
}

pub struct FramePresenter {
    source: PresentSource,
    forced: Option<PresentMode>,
    mode: Option<PresentMode>,
    readback: Option<ReadbackTarget>,
    pool: Option<DmabufPool>,
    last_modifier: Option<u64>,
    resize: ResizeDebounce,
    mode_reason: String,
    pool_announced: bool,
    fallback_pending_notice: bool,
    debug_timing: bool,
}

impl Default for FramePresenter {
    fn default() -> Self {
        FramePresenter {
            source: PresentSource::default(),
            forced: PresentMode::forced_by_environment(),
            mode: None,
            readback: None,
            pool: None,
            last_modifier: None,
            resize: ResizeDebounce::default(),
            mode_reason: String::new(),
            pool_announced: false,
            fallback_pending_notice: false,
            debug_timing: std::env::var(DEBUG_VARIABLE).is_ok_and(|value| value == DEBUG_ENABLED),
        }
    }
}

impl FramePresenter {
    pub fn showing(source: PresentSource) -> FramePresenter {
        FramePresenter {
            source,
            ..FramePresenter::default()
        }
    }

    pub fn matches_size(&self, width_px: u32, height_px: u32) -> bool {
        let size = Some((width_px, height_px));
        match self.mode {
            Some(PresentMode::Dmabuf(_)) => self.pool.as_ref().map(DmabufPool::size) == size,
            Some(PresentMode::Readback) => self.readback.as_ref().map(ReadbackTarget::size) == size,
            None => false,
        }
    }

    pub fn release(&mut self) {
        self.pool = None;
        self.readback = None;
        self.resize.clear();
        if self.mode.is_some_and(|mode| mode != PresentMode::Readback) {
            self.mode = None;
        }
    }

    pub fn take_fallback_notice(&mut self) -> bool {
        std::mem::take(&mut self.fallback_pending_notice)
    }

    pub fn present(
        &mut self,
        state: &mut AppState,
        native: &gtk::Native,
        width_px: u32,
        height_px: u32,
    ) -> Option<gdk::Texture> {
        let started = Instant::now();
        if self.source == PresentSource::Painting {
            state.view.zoom = clamp_zoom(state.view.zoom);
        }
        let mode = match self.mode {
            Some(mode) => mode,
            None => self.decide_mode(state, native),
        };
        let texture = match mode {
            PresentMode::Readback => self.present_by_readback(state, width_px, height_px),
            PresentMode::Dmabuf(policy) => {
                self.present_by_dmabuf_or_fall_back(state, native, policy, (width_px, height_px))
            }
        };
        if self.debug_timing && texture.is_some() {
            glib::g_message!(
                LOG_DOMAIN,
                "{:?} frame {width_px}×{height_px} by {} in {:.2} ms, {} pen samples",
                self.source,
                self.mode.unwrap_or(mode).name(),
                started.elapsed().as_secs_f64() * MILLISECONDS_PER_SECOND,
                state.pen.samples_last_frame
            );
        }
        texture
    }

    fn decide_mode(&mut self, state: &AppState, native: &gtk::Native) -> PresentMode {
        let renderer_name = native
            .renderer()
            .map(|renderer| renderer.type_().name().to_string())
            .unwrap_or_default();
        let exports = state.gpu.supports_dmabuf_export;
        let mode = choose_mode(self.forced, exports, &renderer_name);
        self.mode_reason = mode_reason(self.forced.is_some(), exports, &renderer_name);
        if mode == PresentMode::Readback {
            glib::g_message!(LOG_DOMAIN, "presenting by readback: {}", self.mode_reason);
        }
        self.mode = Some(mode);
        mode
    }

    fn present_by_dmabuf_or_fall_back(
        &mut self,
        state: &mut AppState,
        native: &gtk::Native,
        policy: ModifierPolicy,
        size: (u32, u32),
    ) -> Option<gdk::Texture> {
        match self.present_by_dmabuf(state, native, policy, size) {
            Ok(texture) => texture,
            Err(error) => {
                self.fall_back(policy, &error);
                self.present_by_readback(state, size.0, size.1)
            }
        }
    }

    fn fall_back(&mut self, policy: ModifierPolicy, error: &DmabufError) {
        let modifier = describe_modifier(self.last_modifier);
        glib::g_warning!(
            LOG_DOMAIN,
            "dmabuf presentation ({}, modifier {modifier}) failed: {error}; presenting by readback",
            PresentMode::Dmabuf(policy).name()
        );
        self.pool = None;
        self.resize.clear();
        self.mode = Some(PresentMode::Readback);
        self.fallback_pending_notice = true;
    }
}

#[cfg(test)]
mod tests;
