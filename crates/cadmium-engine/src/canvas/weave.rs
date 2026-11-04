use super::Weave;

const LINEN_AMPLITUDE_UM: f32 = 12.0;
const LINEN_PERIOD_PX: f32 = 6.0;
const COTTON_AMPLITUDE_UM: f32 = 6.0;
const COTTON_PERIOD_PX: f32 = 4.0;
const SMOOTH_PERIOD_PX: f32 = 1.0;
const WEAVE_BASE_SHARE: f32 = 0.85;
const WEAVE_JITTER_SHARE: f32 = 0.15;
const WEAVE_THREAD_SHARE: f32 = 0.5;
const WEAVE_MID_SHARE: f32 = 0.5;
const WEAVE_HEIGHT_STEP_UM: f32 = 1.0 / 64.0;
const PIXEL_CENTRE: f32 = 0.5;
const PCG_MULTIPLIER: u32 = 747796405;
const PCG_INCREMENT: u32 = 2891336453;
const PCG_OUTPUT_MULTIPLIER: u32 = 277803737;

fn weave_profile(weave: Weave) -> (f32, f32) {
    match weave {
        Weave::Linen => (LINEN_AMPLITUDE_UM, LINEN_PERIOD_PX),
        Weave::Cotton => (COTTON_AMPLITUDE_UM, COTTON_PERIOD_PX),
        Weave::Smooth => (0.0, SMOOTH_PERIOD_PX),
    }
}

fn pcg(seed: u32) -> u32 {
    let state = seed
        .wrapping_mul(PCG_MULTIPLIER)
        .wrapping_add(PCG_INCREMENT);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(PCG_OUTPUT_MULTIPLIER);
    (word >> 22) ^ word
}

fn cell_jitter(cell_x: i32, cell_y: i32) -> f32 {
    let mixed = pcg(cell_x.cast_unsigned() ^ pcg(cell_y.cast_unsigned()));
    WEAVE_BASE_SHARE + WEAVE_JITTER_SHARE * (mixed as f32 / u32::MAX as f32)
}

fn on_weave_height_grid(height: f32) -> f32 {
    (height / WEAVE_HEIGHT_STEP_UM).round() * WEAVE_HEIGHT_STEP_UM
}

pub fn weave_heights(weave: Weave, width: u32, height: u32) -> Vec<f32> {
    let (amplitude, period) = weave_profile(weave);
    let thread = |index: u32| {
        let position = index as f32 + PIXEL_CENTRE;
        (
            (std::f32::consts::TAU * position / period).sin(),
            (position / period).floor() as i32,
        )
    };
    let columns: Vec<(f32, i32)> = (0..width).map(thread).collect();
    let mut heights = Vec::with_capacity(width as usize * height as usize);
    for (row_thread, cell_y) in (0..height).map(thread) {
        heights.extend(columns.iter().map(|&(column_thread, cell_x)| {
            let threads = column_thread + row_thread;
            let jitter = cell_jitter(cell_x, cell_y);
            let raised =
                amplitude * WEAVE_MID_SHARE * (1.0 + WEAVE_THREAD_SHARE * threads * jitter);
            on_weave_height_grid(raised.clamp(0.0, amplitude))
        }));
    }
    heights
}
