//! What makes a stroke look hand-made: seeded wobble in where it lands, how far it pulls,
//! how it bows, how hard it presses and how the handle leans.

use cadmium_engine::stroke::StrokeSample;

pub type Point = (f64, f64);
pub type Recipe = Vec<(usize, f32)>;

const SAMPLE_STEP_PX: f64 = 3.0;
const TREMOR_PX: f64 = 0.6;
const TREMOR_MEMORY: f64 = 0.85;
const LIGHTEST_TOUCH: f32 = 0.2;

/// SplitMix64, seeded so the painting comes out the same on every run.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn range(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.unit()
    }

    pub fn spread(&mut self, amount: f64) -> f64 {
        self.range(-amount, amount)
    }

    pub fn chance(&mut self, probability: f64) -> bool {
        self.unit() < probability
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = ((self.unit() * (i + 1) as f64) as usize).min(i);
            items.swap(i, j);
        }
    }
}

/// One pull of the hand: where its middle lands, which way it goes, how far, how it bows.
#[derive(Clone, Copy)]
pub struct Gesture {
    pub middle: Point,
    pub angle: f64,
    pub length: f64,
    pub bend: f64,
}

impl Gesture {
    /// Points along a gently bowed path, with a tremor that drifts instead of jittering.
    pub fn path(&self, rng: &mut Rng) -> Vec<Point> {
        let count = (self.length / SAMPLE_STEP_PX).ceil().max(2.0) as usize;
        let (dx, dy) = (self.angle.cos(), self.angle.sin());
        let mut tremor = 0.0;
        (0..=count)
            .map(|i| {
                let t = i as f64 / count as f64;
                tremor = (tremor + rng.spread(TREMOR_PX)) * TREMOR_MEMORY;
                let along = (t - 0.5) * self.length;
                let across = self.bend * 4.0 * t * (1.0 - t) * self.length + tremor;
                (
                    self.middle.0 + dx * along - dy * across,
                    self.middle.1 + dy * along + dx * across,
                )
            })
            .collect()
    }
}

/// The longest run of `path` that stays where `inside` allows.
pub fn clip(path: &[Point], inside: impl Fn(Point) -> bool) -> &[Point] {
    let mut best = 0..0;
    let mut start = 0;
    for (i, &p) in path.iter().enumerate() {
        if !inside(p) {
            start = i + 1;
        } else if i + 1 - start > best.len() {
            best = start..i + 1;
        }
    }
    &path[best]
}

/// How the hand holds the brush through one stroke: lands, pulls, lifts off, the handle
/// leaning the way the stroke goes so a flat brush drags broadside.
pub struct Touch {
    peak: f32,
    land: f64,
    lift: f64,
    lean_deg: f64,
    twist: f64,
    ms_per_sample: f64,
}

impl Touch {
    pub fn random(rng: &mut Rng) -> Touch {
        Touch {
            peak: rng.range(0.7, 1.0) as f32,
            land: rng.range(0.06, 0.2),
            lift: rng.range(0.15, 0.4),
            lean_deg: rng.range(22.0, 36.0),
            twist: rng.spread(0.12),
            ms_per_sample: rng.range(6.0, 10.0),
        }
    }

    /// The handle held straight up, so a round brush prints round whichever way it turns.
    pub fn upright(self) -> Touch {
        Touch {
            lean_deg: 0.0,
            ..self
        }
    }

    pub fn samples(&self, path: &[Point]) -> Vec<StrokeSample> {
        let (Some(first), Some(last)) = (path.first(), path.last()) else {
            return Vec::new();
        };
        let heading = (last.1 - first.1).atan2(last.0 - first.0) + self.twist;
        let tilt = (
            (self.lean_deg * heading.cos()) as f32,
            (self.lean_deg * heading.sin()) as f32,
        );
        let end = path.len().saturating_sub(1).max(1) as f64;
        path.iter()
            .enumerate()
            .map(|(i, &(x, y))| StrokeSample {
                x,
                y,
                pressure: self.pressure(i as f64 / end),
                tilt_x_deg: tilt.0,
                tilt_y_deg: tilt.1,
                time_ms: (i as f64 * self.ms_per_sample) as u32,
            })
            .collect()
    }

    fn pressure(&self, t: f64) -> f32 {
        let envelope = smoothstep(t / self.land) * smoothstep((1.0 - t) / self.lift);
        LIGHTEST_TOUCH + (self.peak - LIGHTEST_TOUCH) * envelope as f32
    }
}

fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// A mix that shifts from `from` to `to` (light to shade, say) as `t` goes from 0 to 1.
#[derive(Clone, Copy)]
pub struct Tint {
    pub from: &'static [(usize, f32)],
    pub to: &'static [(usize, f32)],
}

impl Tint {
    pub fn at(&self, t: f64) -> Recipe {
        let t = t.clamp(0.0, 1.0) as f32;
        let mut mix: Recipe = self.from.iter().map(|&(s, p)| (s, p * (1.0 - t))).collect();
        mix.extend(self.to.iter().map(|&(s, p)| (s, p * t)));
        mix
    }
}

/// The same mix, but never measured out exactly the same twice.
pub fn vary(rng: &mut Rng, recipe: &[(usize, f32)], amount: f64) -> Recipe {
    recipe
        .iter()
        .map(|&(slot, part)| (slot, part * (1.0 + rng.spread(amount)) as f32))
        .collect()
}
