use cadmium_engine::gpu::{GpuContext, read_mapped, staging_buffer};

const TIMESTAMP_QUERIES: u32 = 2;
const TIMESTAMP_BYTES: u64 = TIMESTAMP_QUERIES as u64 * wgpu::QUERY_SIZE as u64;
const NANOSECONDS_PER_MILLISECOND: f64 = 1.0e6;
const PERCENTILE: f64 = 0.95;

pub(crate) struct Timestamps {
    queries: wgpu::QuerySet,
    resolved: wgpu::Buffer,
    staging: wgpu::Buffer,
    period_ns: f64,
}

pub(crate) struct FrameCost {
    pub(crate) gpu_ms: Option<f64>,
    pub(crate) wall_ms: f64,
}

pub(crate) fn finish_frame(
    gpu: &GpuContext,
    timestamps: Option<&Timestamps>,
    mut encoder: wgpu::CommandEncoder,
) -> Option<f64> {
    let Some(timestamps) = timestamps else {
        gpu.queue.submit([encoder.finish()]);
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        return None;
    };
    write_timestamp(&mut encoder, timestamps, None, Some(1));
    resolve(&mut encoder, timestamps);
    gpu.queue.submit([encoder.finish()]);
    Some(read_gpu_ms(gpu, timestamps))
}

pub(crate) fn write_timestamp(
    encoder: &mut wgpu::CommandEncoder,
    timestamps: &Timestamps,
    beginning_of_pass_write_index: Option<u32>,
    end_of_pass_write_index: Option<u32>,
) {
    encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("bristle_cost_timestamp"),
        timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
            query_set: &timestamps.queries,
            beginning_of_pass_write_index,
            end_of_pass_write_index,
        }),
    });
}

fn resolve(encoder: &mut wgpu::CommandEncoder, timestamps: &Timestamps) {
    encoder.resolve_query_set(
        &timestamps.queries,
        0..TIMESTAMP_QUERIES,
        &timestamps.resolved,
        0,
    );
    encoder.copy_buffer_to_buffer(
        &timestamps.resolved,
        0,
        &timestamps.staging,
        0,
        TIMESTAMP_BYTES,
    );
}

fn read_gpu_ms(gpu: &GpuContext, timestamps: &Timestamps) -> f64 {
    let ticks: [u64; 2] = read_mapped(gpu, &timestamps.staging, |bytes| {
        let ticks: &[u64] = bytemuck::cast_slice(bytes);
        [ticks[0], ticks[1]]
    })
    .expect("the GPU stays available while measuring");
    ticks[1].saturating_sub(ticks[0]) as f64 * timestamps.period_ns / NANOSECONDS_PER_MILLISECOND
}

pub(crate) fn timestamps(gpu: &GpuContext) -> Option<Timestamps> {
    if !gpu
        .device
        .features()
        .contains(wgpu::Features::TIMESTAMP_QUERY)
    {
        return None;
    }
    let queries = gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("bristle_cost_frame"),
        ty: wgpu::QueryType::Timestamp,
        count: TIMESTAMP_QUERIES,
    });
    let resolved = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bristle_cost_resolved"),
        size: TIMESTAMP_BYTES,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    Some(Timestamps {
        queries,
        resolved,
        staging: staging_buffer(gpu, "bristle_cost_staging", TIMESTAMP_BYTES),
        period_ns: f64::from(gpu.queue.get_timestamp_period()),
    })
}

fn mean_and_percentile(mut values: Vec<f64>) -> (f64, f64) {
    values.sort_by(f64::total_cmp);
    let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
    let rank = ((values.len() as f64 * PERCENTILE).ceil() as usize).saturating_sub(1);
    (mean, values.get(rank).copied().unwrap_or(0.0))
}

pub(crate) fn report(name: &str, costs: &[FrameCost]) {
    let (wall_mean, wall_p95) =
        mean_and_percentile(costs.iter().map(|cost| cost.wall_ms).collect());
    let gpu: Option<Vec<f64>> = costs.iter().map(|cost| cost.gpu_ms).collect();
    let gpu = gpu.map_or_else(
        || format!("{:>9} {:>9}", "-", "-"),
        |gpu| {
            let (mean, p95) = mean_and_percentile(gpu);
            format!("{mean:>9.2} {p95:>9.2}")
        },
    );
    println!(
        "{name:<36} {:>6} {gpu} {wall_mean:>9.2} {wall_p95:>9.2}",
        costs.len()
    );
}

const QUARTERS: usize = 4;
const QUARTER_NAMES: [&str; QUARTERS] = [
    "  first quarter",
    "  second quarter",
    "  third quarter",
    "  last quarter",
];

pub(crate) fn report_quarters(costs: &[FrameCost]) {
    let quarter = costs.len().div_ceil(QUARTERS).max(1);
    for (name, chunk) in QUARTER_NAMES.iter().zip(costs.chunks(quarter)) {
        report(name, chunk);
    }
}
