const MIN_DAB_STEP_PX: f64 = 1.0;
const MIN_SPEED_INTERVAL_MS: f64 = 1.0;
const ARC_LENGTH_TOLERANCE_PX: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Brush,
    Knife,
    Blender,
    Rag,
    Eyedropper,
}

#[derive(Clone, Copy, Debug)]
pub struct StrokeSample {
    pub x: f64,
    pub y: f64,
    pub pressure: f32,
    pub tilt_x_deg: f32,
    pub tilt_y_deg: f32,
    pub time_ms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenAngles {
    pub altitude_rad: f32,
    pub azimuth_rad: f32,
}

pub fn pen_angles(tilt_x_deg: f32, tilt_y_deg: f32) -> PenAngles {
    let tan_x = tilt_x_deg.to_radians().tan();
    let tan_y = tilt_y_deg.to_radians().tan();
    PenAngles {
        altitude_rad: 1.0f32.atan2(tan_x.hypot(tan_y)),
        azimuth_rad: tan_y.atan2(tan_x),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PressureCurve {
    pub gamma: f32,
    pub min: f32,
    pub max: f32,
}

impl Default for PressureCurve {
    fn default() -> Self {
        PressureCurve {
            gamma: 1.0,
            min: 0.0,
            max: 1.0,
        }
    }
}

impl PressureCurve {
    pub fn apply(&self, p: f32) -> f32 {
        self.min + (self.max - self.min) * p.clamp(0.0, 1.0).powf(self.gamma)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Dab {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub pressure: f32,
    pub altitude: f32,
    pub azimuth: f32,
    pub direction: f32,
    pub speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushGeometry {
    pub max_diameter_px: f32,
    pub min_diameter_ratio: f32,
    pub spacing: f32,
    pub pressure_curve: PressureCurve,
}

impl BrushGeometry {
    fn diameter(&self, pressure: f32) -> f32 {
        self.max_diameter_px
            * (self.min_diameter_ratio
                + (1.0 - self.min_diameter_ratio) * self.pressure_curve.apply(pressure))
    }

    fn step(&self, pressure: f32) -> f64 {
        f64::from(self.spacing * self.diameter(pressure)).max(MIN_DAB_STEP_PX)
    }
}

pub struct StrokeBuilder {
    geometry: BrushGeometry,
    last_sample: Option<StrokeSample>,
    distance_since_dab: f64,
    emitted_any: bool,
}

impl StrokeBuilder {
    pub fn new(geometry: BrushGeometry) -> Self {
        StrokeBuilder {
            geometry,
            last_sample: None,
            distance_since_dab: 0.0,
            emitted_any: false,
        }
    }

    pub fn push(&mut self, sample: StrokeSample) -> Vec<Dab> {
        let dabs = match self.last_sample {
            None => vec![self.dab_at(&sample, 0.0, 0.0)],
            Some(previous) => self.walk_segment(&previous, &sample),
        };
        self.emitted_any |= !dabs.is_empty();
        self.last_sample = Some(sample);
        dabs
    }

    pub fn finish(&mut self) -> Vec<Dab> {
        let pending = match self.last_sample {
            Some(sample) if !self.emitted_any => vec![self.dab_at(&sample, 0.0, 0.0)],
            _ => Vec::new(),
        };
        self.last_sample = None;
        self.distance_since_dab = 0.0;
        self.emitted_any = false;
        pending
    }

    fn walk_segment(&mut self, from: &StrokeSample, to: &StrokeSample) -> Vec<Dab> {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let length = dx.hypot(dy);
        if length <= 0.0 {
            return Vec::new();
        }
        let elapsed_ms =
            f64::from(to.time_ms.saturating_sub(from.time_ms)).max(MIN_SPEED_INTERVAL_MS);
        let direction = dy.atan2(dx) as f32;
        let speed = (length / elapsed_ms) as f32;

        let mut dabs = Vec::new();
        let mut travelled = 0.0;
        loop {
            let pressure_here = lerp_f32(from.pressure, to.pressure, travelled / length);
            let needed = self.geometry.step(pressure_here) - self.distance_since_dab;
            if travelled + needed > length + ARC_LENGTH_TOLERANCE_PX {
                self.distance_since_dab += length - travelled;
                break;
            }
            travelled = (travelled + needed).min(length);
            self.distance_since_dab = 0.0;
            let at = interpolate(from, to, travelled / length);
            dabs.push(self.dab_at(&at, direction, speed));
        }
        dabs
    }

    fn dab_at(&self, sample: &StrokeSample, direction: f32, speed: f32) -> Dab {
        let angles = pen_angles(sample.tilt_x_deg, sample.tilt_y_deg);
        Dab {
            x: sample.x as f32,
            y: sample.y as f32,
            radius: self.geometry.diameter(sample.pressure) / 2.0,
            pressure: sample.pressure,
            altitude: angles.altitude_rad,
            azimuth: angles.azimuth_rad,
            direction,
            speed,
        }
    }
}

fn lerp_f32(a: f32, b: f32, t: f64) -> f32 {
    a + (b - a) * t as f32
}

fn interpolate(from: &StrokeSample, to: &StrokeSample, t: f64) -> StrokeSample {
    let elapsed = f64::from(to.time_ms.saturating_sub(from.time_ms));
    StrokeSample {
        x: from.x + (to.x - from.x) * t,
        y: from.y + (to.y - from.y) * t,
        pressure: lerp_f32(from.pressure, to.pressure, t),
        tilt_x_deg: lerp_f32(from.tilt_x_deg, to.tilt_x_deg, t),
        tilt_y_deg: lerp_f32(from.tilt_y_deg, to.tilt_y_deg, t),
        time_ms: from.time_ms + (elapsed * t).round() as u32,
    }
}
