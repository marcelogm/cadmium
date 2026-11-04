use std::collections::VecDeque;

use cadmium_engine::gpu::GpuContext;
use gtk::prelude::*;
use gtk::{gdk, glib, graphene, gsk};

use crate::dmabuf::{DmabufPool, ModifierPolicy};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 128;
const FRAMES: u32 = 200;
const HELD_TEXTURES: usize = 2;
const EXHAUST_POOL_EVERY: u32 = 50;
const MAX_CONSECUTIVE_STALLS: u32 = 120;
const FD_SETTLE_TICKS: u32 = 120;
const MAX_STEADY_FD_GROWTH: usize = 32;
const SYNC_FILE_LINK: &str = "anon_inode:sync_file";
const DMABUF_LINK_PREFIX: &str = "/dmabuf:";
const CHANNEL_TOLERANCE: u8 = 1;
const WORKGROUP_SIZE: u32 = 8;
const BUSY_ITERATIONS: u32 = 20000;

const PATTERN_SHADER: &str = r#"
struct Pattern { base: vec4<u32> };
@group(0) @binding(0) var<uniform> pattern: Pattern;
@group(0) @binding(1) var target_image: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(target_image);
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let red = pattern.base.x;
    let green = (id.x + pattern.base.y) % 256u;
    let blue = (id.y * 2u + pattern.base.z) % 256u;
    var channels = vec3<f32>(f32(red), f32(green), f32(blue)) / 255.0;
    var busy = f32(id.x + id.y) * 0.001;
    for (var step = 0u; step < pattern.base.w; step++) {
        busy = fract(busy * 1.618 + 0.31);
    }
    if (busy < 0.0) {
        channels = vec3<f32>(0.0);
    }
    textureStore(target_image, vec2<i32>(id.xy), vec4<f32>(channels, 1.0));
}
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FramePattern {
    red: u32,
    green_shift: u32,
    blue_shift: u32,
}

impl FramePattern {
    fn for_frame(frame: u32) -> FramePattern {
        FramePattern {
            red: (40 + frame * 37) % 256,
            green_shift: (120 + frame * 11) % 256,
            blue_shift: (200 + frame * 53) % 256,
        }
    }

    fn expected(&self, x: u32, y: u32) -> [u8; 4] {
        [
            self.red as u8,
            ((x + self.green_shift) % 256) as u8,
            ((y * 2 + self.blue_shift) % 256) as u8,
            255,
        ]
    }

    fn uniform(&self) -> [u32; 4] {
        [self.red, self.green_shift, self.blue_shift, BUSY_ITERATIONS]
    }
}

struct PatternPass {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
}

impl PatternPass {
    fn new(gpu: &GpuContext) -> PatternPass {
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("dmabuf_selftest_pattern"),
                source: wgpu::ShaderSource::Wgsl(PATTERN_SHADER.into()),
            });
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("dmabuf_selftest_pattern"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("dmabuf_selftest_pattern"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("dmabuf_selftest_pattern"),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("dmabuf_selftest_pattern"),
            size: std::mem::size_of::<[u32; 4]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        PatternPass {
            pipeline,
            layout,
            uniform,
        }
    }

    fn encode(
        &self,
        gpu: &GpuContext,
        pattern: FramePattern,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
    ) {
        let words = pattern.uniform();
        let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
        gpu.queue.write_buffer(&self.uniform, 0, &bytes);
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dmabuf_selftest_pattern"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("dmabuf_selftest_pattern"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let size = target.size();
        pass.dispatch_workgroups(
            size.width.div_ceil(WORKGROUP_SIZE),
            size.height.div_ceil(WORKGROUP_SIZE),
            1,
        );
    }
}

enum Phase {
    Presenting,
    Settling { ticks: u32 },
}

pub enum SelftestProgress {
    Running,
    Passed(String),
    Failed(String),
}

pub struct DmabufSelftest {
    gpu: GpuContext,
    pass: PatternPass,
    pool: Option<DmabufPool>,
    held: VecDeque<gdk::Texture>,
    exhausted_pool_checks: u32,
    modifier: u64,
    renderer_name: String,
    phase: Phase,
    frames_checked: u32,
    consecutive_stalls: u32,
    total_stalls: u32,
    worst_download_error: u8,
    worst_render_error: u8,
    fds_before: FdCensus,
    fds_halfway: FdCensus,
    fds_after_frames: FdCensus,
    dmabufs_closed_by_drop: usize,
    pool_slots: usize,
}

impl DmabufSelftest {
    pub fn start(
        window: &impl IsA<gtk::Native>,
        policy: ModifierPolicy,
    ) -> Result<DmabufSelftest, String> {
        let gpu = GpuContext::new().map_err(|error| error.to_string())?;
        let pass = PatternPass::new(&gpu);
        let display = window.as_ref().display();
        let renderer_name = window
            .as_ref()
            .renderer()
            .map(|renderer| renderer.type_().name().to_string())
            .unwrap_or_else(|| "none".into());
        let fds_before = FdCensus::take();
        let pool = DmabufPool::new(&gpu, &display, WIDTH, HEIGHT, policy)
            .map_err(|error| error.to_string())?;
        if pool.size() != (WIDTH, HEIGHT) {
            return Err(format!("pool size {:?}", pool.size()));
        }
        let pool_slots = pool.slot_count();
        Ok(DmabufSelftest {
            modifier: pool.modifier(),
            gpu,
            pass,
            pool: Some(pool),
            held: VecDeque::new(),
            exhausted_pool_checks: 0,
            renderer_name,
            phase: Phase::Presenting,
            frames_checked: 0,
            consecutive_stalls: 0,
            total_stalls: 0,
            worst_download_error: 0,
            worst_render_error: 0,
            fds_before,
            fds_halfway: FdCensus::default(),
            fds_after_frames: FdCensus::default(),
            dmabufs_closed_by_drop: 0,
            pool_slots,
        })
    }

    pub fn tick(&mut self, window: &impl IsA<gtk::Native>) -> SelftestProgress {
        match self.phase {
            Phase::Presenting => self.present_frame(window.as_ref()),
            Phase::Settling { ticks } if ticks < FD_SETTLE_TICKS => {
                self.phase = Phase::Settling { ticks: ticks + 1 };
                SelftestProgress::Running
            }
            Phase::Settling { .. } => self.finish(),
        }
    }

    fn present_frame(&mut self, native: &gtk::Native) -> SelftestProgress {
        let Some(pool) = self.pool.as_mut() else {
            return SelftestProgress::Failed("pool vanished".into());
        };
        let display = native.display();
        let pattern = FramePattern::for_frame(self.frames_checked);
        let scope = self
            .gpu
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let gpu = &self.gpu;
        let pass = &self.pass;
        let presented = pool.present_with(gpu, &display, |encoder, target| {
            pass.encode(gpu, pattern, encoder, target)
        });
        if let Some(error) = pollster::block_on(scope.pop()) {
            return SelftestProgress::Failed(format!("wgpu validation: {error}"));
        }
        let texture = match presented {
            Ok(Some(texture)) => texture,
            Ok(None) => {
                self.consecutive_stalls += 1;
                self.total_stalls += 1;
                if self.consecutive_stalls > MAX_CONSECUTIVE_STALLS {
                    return SelftestProgress::Failed(format!(
                        "no slot came back for {MAX_CONSECUTIVE_STALLS} ticks"
                    ));
                }
                return SelftestProgress::Running;
            }
            Err(error) => return SelftestProgress::Failed(error.to_string()),
        };
        self.consecutive_stalls = 0;

        let downloaded = match check_pixels(&download(&texture), pattern) {
            Ok(error) => error,
            Err(mismatch) => {
                return SelftestProgress::Failed(format!(
                    "frame {} download: {mismatch}",
                    self.frames_checked
                ));
            }
        };
        let rendered = match render_through_gsk(native, &texture)
            .and_then(|pixels| check_pixels(&pixels, pattern))
        {
            Ok(error) => error,
            Err(mismatch) => {
                return SelftestProgress::Failed(format!(
                    "frame {} gsk render: {mismatch}",
                    self.frames_checked
                ));
            }
        };
        self.worst_download_error = self.worst_download_error.max(downloaded);
        self.worst_render_error = self.worst_render_error.max(rendered);
        self.held.push_back(texture);
        self.frames_checked += 1;
        if self.frames_checked.is_multiple_of(EXHAUST_POOL_EVERY)
            && let Err(reason) = self.expect_exhausted_pool(&display)
        {
            return SelftestProgress::Failed(reason);
        }
        while self.held.len() > HELD_TEXTURES {
            self.held.pop_front();
        }
        if self.frames_checked == FRAMES / 2 {
            self.fds_halfway = FdCensus::take();
        }
        if self.frames_checked == FRAMES {
            self.held.clear();
            self.fds_after_frames = FdCensus::take();
            self.pool = None;
            self.dmabufs_closed_by_drop = self
                .fds_after_frames
                .dmabufs
                .saturating_sub(FdCensus::take().dmabufs);
            self.phase = Phase::Settling { ticks: 0 };
        }
        SelftestProgress::Running
    }

    fn expect_exhausted_pool(&mut self, display: &gdk::Display) -> Result<(), String> {
        let Some(pool) = self.pool.as_mut() else {
            return Err("pool vanished".into());
        };
        let slots = pool.slot_count();
        let mut presented = Vec::new();
        loop {
            let encoded = std::cell::Cell::new(false);
            let gpu = &self.gpu;
            let pass = &self.pass;
            let pattern = FramePattern::for_frame(self.frames_checked);
            match pool.present_with(gpu, display, |encoder, target| {
                encoded.set(true);
                pass.encode(gpu, pattern, encoder, target)
            }) {
                Ok(Some(texture)) => presented.push(texture),
                Ok(None) if encoded.get() => {
                    return Err("busy pool still ran the encoder".into());
                }
                Ok(None) => break,
                Err(error) => return Err(error.to_string()),
            }
            if presented.len() + self.held.len() > slots {
                return Err(format!(
                    "pool handed out {} textures at once",
                    presented.len() + self.held.len()
                ));
            }
        }
        if presented.len() + self.held.len() != slots {
            return Err(format!(
                "pool ran dry after {} textures",
                presented.len() + self.held.len()
            ));
        }
        self.exhausted_pool_checks += 1;
        Ok(())
    }

    fn finish(&mut self) -> SelftestProgress {
        let fds_settled = FdCensus::take();
        let summary = format!(
            "modifier=0x{:016x} frames={} renderer={} stalls={} exhausted_pool_checks={} max_error(download={}, gsk={}) fds(before={}, halfway={}, after_frames={}, after_drop={}) dmabufs_closed_by_drop={}",
            self.modifier,
            self.frames_checked,
            self.renderer_name,
            self.total_stalls,
            self.exhausted_pool_checks,
            self.worst_download_error,
            self.worst_render_error,
            self.fds_before,
            self.fds_halfway,
            self.fds_after_frames,
            fds_settled,
            self.dmabufs_closed_by_drop
        );
        if self.dmabufs_closed_by_drop != self.pool_slots {
            return SelftestProgress::Failed(format!("pool drop kept dmabufs open: {summary}"));
        }
        if self.fds_after_frames.total > self.fds_halfway.total + MAX_STEADY_FD_GROWTH {
            return SelftestProgress::Failed(format!("file descriptors grew per frame: {summary}"));
        }
        SelftestProgress::Passed(summary)
    }
}

struct Pixels {
    bytes: glib::Bytes,
    stride: usize,
}

fn download(texture: &gdk::Texture) -> Pixels {
    let mut downloader = gdk::TextureDownloader::new(texture);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = downloader.download_bytes();
    Pixels { bytes, stride }
}

fn render_through_gsk(native: &gtk::Native, texture: &gdk::Texture) -> Result<Pixels, String> {
    let renderer = native
        .renderer()
        .ok_or_else(|| "window has no renderer".to_string())?;
    let bounds = graphene::Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32);
    let node = gsk::TextureNode::new(texture, &bounds);
    let rendered = renderer.render_texture(&node, Some(&bounds));
    if (rendered.width(), rendered.height()) != (WIDTH as i32, HEIGHT as i32) {
        return Err(format!(
            "rendered {}x{} instead of {WIDTH}x{HEIGHT}",
            rendered.width(),
            rendered.height()
        ));
    }
    Ok(download(&rendered))
}

fn check_pixels(pixels: &Pixels, pattern: FramePattern) -> Result<u8, String> {
    let data: &[u8] = &pixels.bytes;
    let mut worst = 0u8;
    for y in 0..HEIGHT {
        let row = y as usize * pixels.stride;
        for x in 0..WIDTH {
            let start = row + x as usize * 4;
            let actual = data
                .get(start..start + 4)
                .ok_or_else(|| format!("download too short at ({x}, {y})"))?;
            let expected = pattern.expected(x, y);
            let error = actual
                .iter()
                .zip(expected)
                .map(|(&got, want)| got.abs_diff(want))
                .max()
                .unwrap_or(0);
            if error > CHANNEL_TOLERANCE {
                return Err(format!(
                    "pixel ({x}, {y}) is {actual:?}, expected {expected:?}"
                ));
            }
            worst = worst.max(error);
        }
    }
    Ok(worst)
}

#[derive(Clone, Copy, Debug, Default)]
struct FdCensus {
    total: usize,
    sync_files: usize,
    dmabufs: usize,
}

impl FdCensus {
    fn take() -> FdCensus {
        let mut census = FdCensus::default();
        let Ok(entries) = std::fs::read_dir("/proc/self/fd") else {
            return census;
        };
        for entry in entries.flatten() {
            census.total += 1;
            let Ok(target) = std::fs::read_link(entry.path()) else {
                continue;
            };
            let target = target.to_string_lossy();
            if target == SYNC_FILE_LINK {
                census.sync_files += 1;
            } else if target.starts_with(DMABUF_LINK_PREFIX) {
                census.dmabufs += 1;
            }
        }
        census
    }
}

impl std::fmt::Display for FdCensus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} total/{} sync_file/{} dmabuf",
            self.total, self.sync_files, self.dmabufs
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_frames_use_distinct_patterns() {
        let patterns: Vec<FramePattern> = (0..FRAMES).map(FramePattern::for_frame).collect();
        for pair in patterns.windows(2) {
            assert_ne!(pair[0], pair[1]);
        }
    }

    #[test]
    fn first_frame_starts_from_the_reference_colour() {
        assert_eq!(
            FramePattern::for_frame(0).expected(0, 0),
            [40, 120, 200, 255]
        );
    }

    #[test]
    fn pattern_varies_along_both_axes() {
        let pattern = FramePattern::for_frame(3);
        assert_ne!(pattern.expected(0, 0), pattern.expected(1, 0));
        assert_ne!(pattern.expected(0, 0), pattern.expected(0, 1));
    }
}
