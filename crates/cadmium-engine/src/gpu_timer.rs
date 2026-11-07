use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::gpu::GpuContext;

const SLOTS: usize = 4;
const QUERIES_PER_SLOT: u32 = 2;
const SLOT_BYTES: u64 = QUERIES_PER_SLOT as u64 * wgpu::QUERY_SIZE as u64;
const NANOSECONDS_PER_MILLISECOND: f64 = 1.0e6;
const FREE: u8 = 0;
const ENCODED: u8 = 1;
const MAPPING: u8 = 2;
const MAPPED: u8 = 3;

struct Slot {
    staging: wgpu::Buffer,
    state: Arc<AtomicU8>,
    work: usize,
}

/// What one timed frame cost: the `work` it was ended with and its GPU time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimedFrame {
    pub work: usize,
    pub gpu_ms: f64,
}

/// Times stretches of a frame's GPU work with timestamp queries, read back without ever
/// waiting: a frame's time comes out of `finished` a frame or two after it was submitted.
pub struct GpuFrameTimer {
    queries: wgpu::QuerySet,
    resolved: wgpu::Buffer,
    slots: Vec<Slot>,
    period_ns: f64,
}

impl GpuFrameTimer {
    /// A timer, or `None` on an adapter without timestamp queries.
    pub fn new(gpu: &GpuContext) -> Option<GpuFrameTimer> {
        if !gpu
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            return None;
        }
        let queries = gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("gpu_frame_timer"),
            ty: wgpu::QueryType::Timestamp,
            count: QUERIES_PER_SLOT * SLOTS as u32,
        });
        let resolved = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_frame_timer_resolved"),
            size: SLOT_BYTES * SLOTS as u64,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let slots = (0..SLOTS).map(|_| new_slot(gpu)).collect();
        Some(GpuFrameTimer {
            queries,
            resolved,
            slots,
            period_ns: f64::from(gpu.queue.get_timestamp_period()),
        })
    }

    /// Marks the start of the stretch to time; `None` while every slot is still in flight.
    pub fn begin(&self, encoder: &mut wgpu::CommandEncoder) -> Option<usize> {
        let index = self
            .slots
            .iter()
            .position(|slot| slot.state.load(Ordering::Acquire) == FREE)?;
        self.write(encoder, Some(first_query(index)), None);
        Some(index)
    }

    /// Marks the end of the stretch `begin` started in `slot`, which did `work`.
    pub fn end(&mut self, encoder: &mut wgpu::CommandEncoder, slot: usize, work: usize) {
        let first = first_query(slot);
        self.write(encoder, None, Some(first + 1));
        let offset = slot as u64 * SLOT_BYTES;
        encoder.resolve_query_set(
            &self.queries,
            first..first + QUERIES_PER_SLOT,
            &self.resolved,
            offset,
        );
        let slot = &mut self.slots[slot];
        encoder.copy_buffer_to_buffer(&self.resolved, offset, &slot.staging, 0, SLOT_BYTES);
        slot.work = work;
        slot.state.store(ENCODED, Ordering::Release);
    }

    /// Starts reading back every stretch ended in work that has now been submitted.
    pub fn after_submit(&self) {
        self.slots
            .iter()
            .filter(|slot| slot.state.load(Ordering::Acquire) == ENCODED)
            .for_each(start_reading);
    }

    /// The frames whose times have come back since the last call, oldest slot first.
    pub fn finished(&self, gpu: &GpuContext) -> Vec<TimedFrame> {
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        self.slots
            .iter()
            .filter(|slot| slot.state.load(Ordering::Acquire) == MAPPED)
            .map(|slot| self.take(slot))
            .collect()
    }

    fn take(&self, slot: &Slot) -> TimedFrame {
        let ticks = match slot.staging.slice(..).get_mapped_range() {
            Ok(view) => {
                let ticks: &[u64] = bytemuck::cast_slice(&view);
                [ticks[0], ticks[1]]
            }
            Err(_) => [0, 0],
        };
        slot.staging.unmap();
        slot.state.store(FREE, Ordering::Release);
        TimedFrame {
            work: slot.work,
            gpu_ms: ticks[1].saturating_sub(ticks[0]) as f64 * self.period_ns
                / NANOSECONDS_PER_MILLISECOND,
        }
    }

    fn write(&self, encoder: &mut wgpu::CommandEncoder, begin: Option<u32>, end: Option<u32>) {
        encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("gpu_frame_timer_mark"),
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                query_set: &self.queries,
                beginning_of_pass_write_index: begin,
                end_of_pass_write_index: end,
            }),
        });
    }
}

fn start_reading(slot: &Slot) {
    slot.state.store(MAPPING, Ordering::Release);
    let state = Arc::clone(&slot.state);
    slot.staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let next = if result.is_ok() { MAPPED } else { FREE };
            state.store(next, Ordering::Release);
        });
}

fn first_query(slot: usize) -> u32 {
    slot as u32 * QUERIES_PER_SLOT
}

fn new_slot(gpu: &GpuContext) -> Slot {
    Slot {
        staging: crate::gpu::staging_buffer(gpu, "gpu_frame_timer_staging", SLOT_BYTES),
        state: Arc::new(AtomicU8::new(FREE)),
        work: 0,
    }
}
