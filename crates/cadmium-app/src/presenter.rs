use std::time::{Duration, Instant};

use cadmium_engine::gpu::{GpuContext, GpuLost, TexelRows, read_mapped, staging_buffer};
use cadmium_engine::present::{ViewTransform, encode_rgba8_copy, rgba8_rows};
use cadmium_engine::surface::{TRAY_HEIGHT, TRAY_WIDTH};
use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::LOG_DOMAIN;
use crate::dmabuf::{DmabufError, DmabufPool, ModifierPolicy};
use crate::state::AppState;
use crate::view::clamp_zoom;

const PRESENT_MODE_VARIABLE: &str = "CADMIUM_PRESENT";
const DEBUG_VARIABLE: &str = "CADMIUM_DEBUG";
const DEBUG_ENABLED: &str = "1";
const VULKAN_RENDERER_MARK: &str = "Vulkan";
const RESIZE_SETTLE: Duration = Duration::from_millis(150);
const MILLISECONDS_PER_SECOND: f64 = 1000.0;

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
                    &state.session.canvas,
                    state.view,
                    target,
                    encoder,
                );
            }
            PresentSource::Tray => {
                state.encode_tray_frame(encoder);
                let view = tray_view((target.width(), target.height()));
                state.session.presenter.encode_present(
                    gpu,
                    &state.session.tray,
                    view,
                    target,
                    encoder,
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentMode {
    Readback,
    Dmabuf(ModifierPolicy),
}

impl PresentMode {
    pub fn from_name(name: &str) -> Option<PresentMode> {
        match name {
            "readback" => Some(PresentMode::Readback),
            "dmabuf" => Some(PresentMode::Dmabuf(ModifierPolicy::Any)),
            "dmabuf-nodcc" => Some(PresentMode::Dmabuf(ModifierPolicy::NoCompression)),
            "linear" => Some(PresentMode::Dmabuf(ModifierPolicy::LinearOnly)),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            PresentMode::Readback => "readback",
            PresentMode::Dmabuf(ModifierPolicy::Any) => "dmabuf",
            PresentMode::Dmabuf(ModifierPolicy::NoCompression) => "dmabuf-nodcc",
            PresentMode::Dmabuf(ModifierPolicy::LinearOnly) => "linear",
        }
    }

    fn forced_by_environment() -> Option<PresentMode> {
        let requested = std::env::var(PRESENT_MODE_VARIABLE).ok()?;
        let mode = PresentMode::from_name(&requested);
        if mode.is_none() {
            glib::g_warning!(LOG_DOMAIN, "ignoring {PRESENT_MODE_VARIABLE}={requested:?}");
        }
        mode
    }
}

pub fn choose_mode(
    forced: Option<PresentMode>,
    gpu_exports_dmabuf: bool,
    renderer_name: &str,
) -> PresentMode {
    match forced {
        Some(mode) => mode,
        None if gpu_exports_dmabuf && renderer_name.contains(VULKAN_RENDERER_MARK) => {
            PresentMode::Dmabuf(ModifierPolicy::Any)
        }
        None => PresentMode::Readback,
    }
}

fn mode_reason(forced: bool, gpu_exports_dmabuf: bool, renderer_name: &str) -> String {
    if forced {
        format!("forced by {PRESENT_MODE_VARIABLE}")
    } else if !gpu_exports_dmabuf {
        "the GPU cannot export dmabufs".into()
    } else if !renderer_name.contains(VULKAN_RENDERER_MARK) {
        format!("GTK renders with {renderer_name}, not Vulkan")
    } else {
        format!("GTK renders with {renderer_name}")
    }
}

fn describe_modifier(modifier: Option<u64>) -> String {
    modifier.map_or_else(
        || "none".to_string(),
        |modifier| format!("0x{modifier:016x}"),
    )
}

#[derive(Debug, Default)]
struct ResizeDebounce {
    pending: Option<((u32, u32), Instant)>,
}

impl ResizeDebounce {
    fn has_settled(&mut self, size: (u32, u32), now: Instant) -> bool {
        match self.pending {
            Some((pending, since)) if pending == size => now.duration_since(since) >= RESIZE_SETTLE,
            _ => {
                self.pending = Some((size, now));
                false
            }
        }
    }

    fn clear(&mut self) {
        self.pending = None;
    }
}

struct ReadbackTarget {
    texture: wgpu::Texture,
    staging: wgpu::Buffer,
    rows: TexelRows,
}

impl ReadbackTarget {
    fn new(state: &AppState, width: u32, height: u32) -> ReadbackTarget {
        let texture = state
            .session
            .presenter
            .create_target(&state.gpu, width, height);
        let rows = rgba8_rows(&texture);
        let staging = staging_buffer(&state.gpu, "present_readback", rows.padded_size());
        ReadbackTarget {
            texture,
            staging,
            rows,
        }
    }

    fn size(&self) -> (u32, u32) {
        (self.texture.width(), self.texture.height())
    }

    fn download(&self, gpu: &GpuContext) -> Result<gdk::Texture, GpuLost> {
        let pixels = read_mapped(gpu, &self.staging, <[u8]>::to_vec)?;
        let (width, height) = self.size();
        Ok(gdk::MemoryTexture::new(
            width as i32,
            height as i32,
            gdk::MemoryFormat::R8g8b8a8,
            &glib::Bytes::from_owned(pixels),
            self.rows.padded_row_bytes as usize,
        )
        .upcast())
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
                match self.present_by_dmabuf(state, native, policy, width_px, height_px) {
                    Ok(texture) => texture,
                    Err(error) => {
                        self.fall_back(policy, &error);
                        self.present_by_readback(state, width_px, height_px)
                    }
                }
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

    fn present_by_dmabuf(
        &mut self,
        state: &mut AppState,
        native: &gtk::Native,
        policy: ModifierPolicy,
        width_px: u32,
        height_px: u32,
    ) -> Result<Option<gdk::Texture>, DmabufError> {
        let size = (width_px, height_px);
        let gpu = state.gpu.clone();
        let display = native.display();
        let replace_pool = match self.pool.as_ref().map(DmabufPool::size) {
            None => true,
            Some(pool_size) if pool_size == size => {
                self.resize.clear();
                false
            }
            Some(_) => self.resize.has_settled(size, Instant::now()),
        };
        if replace_pool {
            self.pool = None;
            self.resize.clear();
            let pool = DmabufPool::new(&gpu, &display, width_px, height_px, policy)?;
            if !self.pool_announced {
                glib::g_message!(
                    LOG_DOMAIN,
                    "presenting by {} with modifier 0x{:016x}: {}",
                    PresentMode::Dmabuf(policy).name(),
                    pool.modifier(),
                    self.mode_reason
                );
                self.pool_announced = true;
            }
            self.last_modifier = Some(pool.modifier());
            self.pool = Some(pool);
        }
        let Some(pool) = self.pool.as_mut() else {
            return Ok(None);
        };
        let source = self.source;
        pool.present_with(&gpu, &display, |encoder, target| {
            source.encode(state, &gpu, target, encoder);
        })
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

    fn present_by_readback(
        &mut self,
        state: &mut AppState,
        width_px: u32,
        height_px: u32,
    ) -> Option<gdk::Texture> {
        let size = (width_px, height_px);
        if self.readback.as_ref().map(ReadbackTarget::size) != Some(size) {
            self.readback = Some(ReadbackTarget::new(state, width_px, height_px));
        }
        let target = self
            .readback
            .as_ref()
            .expect("readback target was just created");
        let gpu = state.gpu.clone();
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cadmium_frame"),
            });
        self.source
            .encode(state, &gpu, &target.texture, &mut encoder);
        encode_rgba8_copy(&target.texture, &target.staging, &mut encoder);
        gpu.queue.submit([encoder.finish()]);
        target.download(&gpu).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VULKAN_RENDERER: &str = "GskVulkanRenderer";
    const GL_RENDERER: &str = "GskGLRenderer";

    #[test]
    fn tray_view_fits_the_whole_tray_in_the_widget() {
        assert_eq!(tray_view((384, 400)).zoom, 0.25);
        assert_eq!(tray_view((768, 256)).zoom, 0.25);
        assert_eq!(
            crate::view::viewport_to_canvas(tray_view((384, 256)), 384.0, 256.0),
            (1536.0, 1024.0)
        );
    }

    #[test]
    fn present_mode_names_follow_the_environment_contract() {
        for name in ["readback", "dmabuf", "dmabuf-nodcc", "linear"] {
            assert_eq!(
                PresentMode::from_name(name).map(PresentMode::name),
                Some(name)
            );
        }
        assert_eq!(
            PresentMode::from_name("linear"),
            Some(PresentMode::Dmabuf(ModifierPolicy::LinearOnly))
        );
        assert_eq!(PresentMode::from_name("fast"), None);
    }

    #[test]
    fn vulkan_gtk_on_an_exporting_gpu_presents_by_dmabuf() {
        assert_eq!(
            choose_mode(None, true, VULKAN_RENDERER),
            PresentMode::Dmabuf(ModifierPolicy::Any)
        );
    }

    #[test]
    fn gl_gtk_or_a_gpu_without_export_presents_by_readback() {
        assert_eq!(choose_mode(None, true, GL_RENDERER), PresentMode::Readback);
        assert_eq!(
            choose_mode(None, false, VULKAN_RENDERER),
            PresentMode::Readback
        );
    }

    #[test]
    fn the_environment_overrides_the_automatic_choice() {
        assert_eq!(
            choose_mode(Some(PresentMode::Readback), true, VULKAN_RENDERER),
            PresentMode::Readback
        );
        assert_eq!(
            choose_mode(
                Some(PresentMode::Dmabuf(ModifierPolicy::LinearOnly)),
                true,
                GL_RENDERER
            ),
            PresentMode::Dmabuf(ModifierPolicy::LinearOnly)
        );
    }

    #[test]
    fn fallback_reason_names_the_gl_renderer() {
        assert!(mode_reason(false, true, GL_RENDERER).contains(GL_RENDERER));
        assert!(mode_reason(true, true, VULKAN_RENDERER).contains(PRESENT_MODE_VARIABLE));
    }

    #[test]
    fn fallback_log_names_the_last_modifier_or_none() {
        assert_eq!(describe_modifier(Some(0x20)), "0x0000000000000020");
        assert_eq!(describe_modifier(None), "none");
    }

    #[test]
    fn pool_is_rebuilt_only_after_the_size_holds_still() {
        let start = Instant::now();
        let mut resize = ResizeDebounce::default();
        assert!(!resize.has_settled((800, 600), start));
        assert!(!resize.has_settled((800, 600), start + RESIZE_SETTLE / 2));
        assert!(resize.has_settled((800, 600), start + RESIZE_SETTLE));
    }

    #[test]
    fn a_new_size_during_a_drag_restarts_the_wait() {
        let start = Instant::now();
        let mut resize = ResizeDebounce::default();
        assert!(!resize.has_settled((800, 600), start));
        assert!(!resize.has_settled((820, 600), start + RESIZE_SETTLE));
        assert!(!resize.has_settled((820, 600), start + RESIZE_SETTLE * 3 / 2));
        assert!(resize.has_settled((820, 600), start + RESIZE_SETTLE * 2));
    }
}
