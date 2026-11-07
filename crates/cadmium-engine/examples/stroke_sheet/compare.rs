const VISIBLE_DIFFERENCE: u8 = 3;
const HEAT_GAIN: f32 = 6.0;
const BACKDROP_SHARE: f32 = 0.45;
const BACKDROP_LIFT: f32 = 0.5;
const CHANNEL_MAX: f32 = 255.0;
const RGBA: usize = 4;

pub(crate) struct Comparison {
    pub(crate) mean_difference: f64,
    pub(crate) max_difference: u8,
    pub(crate) changed_share: f64,
    pub(crate) heat_rgba: Vec<u8>,
}

fn pixel_difference(reference: &[u8], candidate: &[u8]) -> u8 {
    (0..3)
        .map(|channel| reference[channel].abs_diff(candidate[channel]))
        .max()
        .unwrap_or(0)
}

fn heat_pixel(candidate: &[u8], difference: u8) -> [u8; 4] {
    let grey = (0.299 * f32::from(candidate[0])
        + 0.587 * f32::from(candidate[1])
        + 0.114 * f32::from(candidate[2]))
        / CHANNEL_MAX;
    let backdrop = BACKDROP_LIFT + BACKDROP_SHARE * grey;
    let heat = (f32::from(difference) * HEAT_GAIN / CHANNEL_MAX).min(1.0);
    let red = backdrop + (1.0 - backdrop) * heat;
    let rest = backdrop * (1.0 - heat);
    let [red, green, blue] = [red, rest, rest].map(|value| (value * CHANNEL_MAX) as u8);
    [red, green, blue, u8::MAX]
}

/// Where and how much `candidate` differs from `reference`, both RGBA: the largest channel
/// difference of each pixel, summed up and painted red over a faded copy of the candidate.
pub(crate) fn compare(reference: &[u8], candidate: &[u8]) -> Comparison {
    let mut total = 0u64;
    let mut max_difference = 0u8;
    let mut changed = 0usize;
    let mut heat_rgba = Vec::with_capacity(candidate.len());
    for (before, after) in reference.chunks(RGBA).zip(candidate.chunks(RGBA)) {
        let difference = pixel_difference(before, after);
        total += u64::from(difference);
        max_difference = max_difference.max(difference);
        if difference >= VISIBLE_DIFFERENCE {
            changed += 1;
        }
        heat_rgba.extend(heat_pixel(after, difference));
    }
    let pixels = (reference.len() / RGBA).max(1);
    Comparison {
        mean_difference: total as f64 / pixels as f64,
        max_difference,
        changed_share: changed as f64 / pixels as f64,
        heat_rgba,
    }
}
