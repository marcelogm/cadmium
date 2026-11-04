use std::collections::VecDeque;

use cadmium_engine::gpu::GpuContext;
use gtk::gdk;
use gtk::prelude::*;

use crate::dmabuf::{DmabufPool, ModifierPolicy};
use check::{FdCensus, check_pixels, download, render_through_gsk};
use pattern::{FramePattern, PatternPass};

mod check;
mod pattern;
#[cfg(test)]
mod tests;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 128;
const FRAMES: u32 = 200;
const HELD_TEXTURES: usize = 2;
const EXHAUST_POOL_EVERY: u32 = 50;
const MAX_CONSECUTIVE_STALLS: u32 = 120;
const FD_SETTLE_TICKS: u32 = 120;
const MAX_STEADY_FD_GROWTH: usize = 32;

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

    #[expect(clippy::excessive_nesting, clippy::too_many_lines)]
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

    #[expect(clippy::excessive_nesting)]
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
