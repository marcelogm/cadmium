use std::collections::VecDeque;

use cadmium_engine::gpu::GpuContext;
use gtk::gdk;

use crate::dmabuf::DmabufPool;
use check::FdCensus;
use pattern::PatternPass;

mod check;
mod pattern;
mod run;
#[cfg(test)]
mod tests;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 128;

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
