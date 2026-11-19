//! Times each stage of each dab in its own compute pass, to attribute a frame's GPU time to
//! the stages. Splitting the passes costs a little, so this is a measuring aid for
//! `bristle_cost`, never the app's path.

use super::WetOilEngine;
use crate::gpu::{GpuContext, GpuLost, read_mapped, staging_buffer};
use crate::tool::DabStage;

const MAX_QUERIES: u32 = 4096;
const QUERIES_PER_PASS: u32 = 2;
const MAX_TIMED_PASSES: usize = (MAX_QUERIES / QUERIES_PER_PASS) as usize;
const QUERY_BYTES: u64 = wgpu::QUERY_SIZE as u64;
const NANOSECONDS_PER_MILLISECOND: f64 = 1.0e6;

/// One stage's GPU time over the dabs timed since the last read.
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
}

impl DabStageTimer {
    /// A timer, or `None` on an adapter without timestamp queries.
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
        })
    }

    /// The first query of the next timed pass, which runs `stage`; `None` once the query
    /// set is full, and that pass then runs untimed.
    pub(crate) fn claim(&mut self, stage: DabStage) -> Option<u32> {
        if self.passes.len() >= MAX_TIMED_PASSES {
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

    /// Copies every timestamp claimed so far to the staging buffer, after the work.
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

    /// The stage times resolved and submitted since the last read; empty if none were.
    pub(crate) fn take(&mut self, gpu: &GpuContext) -> Result<Vec<DabStageTime>, GpuLost> {
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

/// Each stage's dab count and summed time, in the order the stages first ran.
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
    /// Times every dab stage in its own compute pass from now on. A measuring aid for
    /// `bristle_cost`: false, and nothing changes, when the adapter has no timestamps.
    #[doc(hidden)]
    pub fn time_dab_stages(&mut self, gpu: &GpuContext) -> bool {
        self.stage_timer = DabStageTimer::new(gpu);
        self.stage_timer.is_some()
    }

    /// Resolves the stage times of the dabs encoded so far, at the end of a frame.
    #[doc(hidden)]
    pub fn resolve_dab_stage_times(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(timer) = self.stage_timer.as_mut() {
            timer.resolve(encoder);
        }
    }

    /// The stage times resolved since the last read, once their frame was submitted.
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
