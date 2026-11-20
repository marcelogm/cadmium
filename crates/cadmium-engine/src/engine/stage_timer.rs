//! Splitting the passes changes the frame times, so the gate must not use them.

use super::WetOilEngine;
use crate::gpu::{GpuContext, GpuLost, read_mapped, staging_buffer};
use crate::tool::DabStage;

const MAX_QUERIES: u32 = wgpu::QUERY_SET_MAX_QUERIES;
const QUERIES_PER_PASS: u32 = 2;
const MAX_TIMED_PASSES: usize = (MAX_QUERIES / QUERIES_PER_PASS) as usize;
const QUERY_BYTES: u64 = wgpu::QUERY_SIZE as u64;
const NANOSECONDS_PER_MILLISECOND: f64 = 1.0e6;

#[derive(Clone, Debug, PartialEq)]
pub struct DabStageTime {
    pub stage: String,
    pub dabs: u32,
    pub gpu_ms: f64,
}

pub(crate) struct DabStageTimer {
    queries: wgpu::QuerySet,
    resolved: wgpu::Buffer,
    staging: wgpu::Buffer,
    period_ns: f64,
    passes: Vec<DabStage>,
    resolved_passes: usize,
    untimed_passes: usize,
}

impl DabStageTimer {
    pub(crate) fn new(gpu: &GpuContext) -> Option<DabStageTimer> {
        if !gpu
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            return None;
        }
        let bytes = u64::from(MAX_QUERIES) * QUERY_BYTES;
        Some(DabStageTimer {
            queries: gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("dab_stage_timer"),
                ty: wgpu::QueryType::Timestamp,
                count: MAX_QUERIES,
            }),
            resolved: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("dab_stage_timer_resolved"),
                size: bytes,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            staging: staging_buffer(gpu, "dab_stage_timer_staging", bytes),
            period_ns: f64::from(gpu.queue.get_timestamp_period()),
            passes: Vec::new(),
            resolved_passes: 0,
            untimed_passes: 0,
        })
    }

    pub(crate) fn claim(&mut self, stage: DabStage) -> Option<u32> {
        if self.passes.len() >= MAX_TIMED_PASSES {
            self.untimed_passes += 1;
            return None;
        }
        let first = self.passes.len() as u32 * QUERIES_PER_PASS;
        self.passes.push(stage);
        Some(first)
    }

    pub(crate) fn writes(&self, first: u32) -> wgpu::ComputePassTimestampWrites<'_> {
        wgpu::ComputePassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(first),
            end_of_pass_write_index: Some(first + 1),
        }
    }

    pub(crate) fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let count = self.passes.len() as u32 * QUERIES_PER_PASS;
        if count == 0 {
            return;
        }
        encoder.resolve_query_set(&self.queries, 0..count, &self.resolved, 0);
        encoder.copy_buffer_to_buffer(
            &self.resolved,
            0,
            &self.staging,
            0,
            u64::from(count) * QUERY_BYTES,
        );
        self.resolved_passes = self.passes.len();
    }

    pub(crate) fn take(&mut self, gpu: &GpuContext) -> Result<Vec<DabStageTime>, GpuLost> {
        if self.untimed_passes > 0 {
            eprintln!(
                "dab stage timer: {} passes ran untimed past the {MAX_TIMED_PASSES}-pass cap",
                self.untimed_passes
            );
            self.untimed_passes = 0;
        }
        let passes = std::mem::take(&mut self.passes);
        let resolved = std::mem::replace(&mut self.resolved_passes, 0);
        if resolved == 0 {
            return Ok(Vec::new());
        }
        let used = resolved * QUERIES_PER_PASS as usize * QUERY_BYTES as usize;
        let ticks: Vec<u64> = read_mapped(gpu, &self.staging, |bytes| {
            bytemuck::cast_slice::<u8, u64>(&bytes[..used]).to_vec()
        })?;
        Ok(tally(&passes[..resolved], &ticks, self.period_ns))
    }
}

fn tally(passes: &[DabStage], ticks: &[u64], period_ns: f64) -> Vec<DabStageTime> {
    let mut times: Vec<DabStageTime> = Vec::new();
    for (index, stage) in passes.iter().enumerate() {
        let first = index * QUERIES_PER_PASS as usize;
        let gpu_ms = ticks[first + 1].saturating_sub(ticks[first]) as f64 * period_ns
            / NANOSECONDS_PER_MILLISECOND;
        let name = format!("{stage:?}");
        match times.iter_mut().find(|time| time.stage == name) {
            Some(time) => {
                time.dabs += 1;
                time.gpu_ms += gpu_ms;
            }
            None => times.push(DabStageTime {
                stage: name,
                dabs: 1,
                gpu_ms,
            }),
        }
    }
    times
}

impl WetOilEngine {
    /// The bristle settle path (`settle_bristles`) is not timed.
    #[doc(hidden)]
    pub fn time_dab_stages(&mut self, gpu: &GpuContext) -> bool {
        self.stage_timer = DabStageTimer::new(gpu);
        self.stage_timer.is_some()
    }

    #[doc(hidden)]
    pub fn resolve_dab_stage_times(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(timer) = self.stage_timer.as_mut() {
            timer.resolve(encoder);
        }
    }

    #[doc(hidden)]
    pub fn take_dab_stage_times(&mut self, gpu: &GpuContext) -> Result<Vec<DabStageTime>, GpuLost> {
        match self.stage_timer.as_mut() {
            Some(timer) => timer.take(gpu),
            None => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests;
