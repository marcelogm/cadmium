const MIN_DAB_STEP_PX: f64 = 1.0;
const MIN_SPEED_INTERVAL_MS: f64 = 1.0;
const ARC_LENGTH_TOLERANCE_PX: f64 = 1e-6;
const MAX_TILT_DEG: f32 = 89.0;
const MAX_DABS_PER_SEGMENT: usize = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Brush,
    Knife,
    Blender,
    Rag,
    Eyedropper,
}

impl Tool {
    #[cfg(test)]
    pub(crate) const ALL: [Tool; 5] = [
        Tool::Brush,
        Tool::Knife,
        Tool::Blender,
        Tool::Rag,
        Tool::Eyedropper,
    ];
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
    let tan_x = tilt_x_deg
        .clamp(-MAX_TILT_DEG, MAX_TILT_DEG)
        .to_radians()
        .tan();
    let tan_y = tilt_y_deg
        .clamp(-MAX_TILT_DEG, MAX_TILT_DEG)
        .to_radians()
        .tan();
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
    pub fn diameter(&self, pressure: f32) -> f32 {
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
}

#[derive(Clone, Copy)]
struct PenPoint {
    x: f64,
    y: f64,
    pressure: f32,
    tilt_x_deg: f32,
    tilt_y_deg: f32,
}

impl PenPoint {
    fn of(sample: &StrokeSample) -> Self {
        PenPoint {
            x: sample.x,
            y: sample.y,
            pressure: sample.pressure,
            tilt_x_deg: sample.tilt_x_deg,
            tilt_y_deg: sample.tilt_y_deg,
        }
    }

    fn towards(&self, to: &PenPoint, t: f64) -> PenPoint {
        PenPoint {
            x: self.x + (to.x - self.x) * t,
            y: self.y + (to.y - self.y) * t,
            pressure: lerp_f32(self.pressure, to.pressure, t),
            tilt_x_deg: lerp_f32(self.tilt_x_deg, to.tilt_x_deg, t),
            tilt_y_deg: lerp_f32(self.tilt_y_deg, to.tilt_y_deg, t),
        }
    }
}

struct Segment {
    start: PenPoint,
    end: PenPoint,
    length: f64,
    shortest_step: f64,
    direction: f32,
    speed: f32,
}

impl StrokeBuilder {
    pub fn new(geometry: BrushGeometry) -> Self {
        StrokeBuilder {
            geometry,
            last_sample: None,
            distance_since_dab: 0.0,
        }
    }

    pub fn push(&mut self, sample: StrokeSample) -> Vec<Dab> {
        if !is_finite(&sample) {
            return Vec::new();
        }
        let dabs = match self.last_sample {
            None => {
                let point = PenPoint::of(&sample);
                let azimuth = pen_angles(point.tilt_x_deg, point.tilt_y_deg).azimuth_rad;
                vec![self.dab_at(&point, azimuth, 0.0)]
            }
            Some(previous) => match self.walk_segment(&previous, &sample) {
                Some(dabs) => dabs,
                None => return Vec::new(),
            },
        };
        self.last_sample = Some(sample);
        dabs
    }

    pub fn finish(&mut self) -> Vec<Dab> {
        self.last_sample = None;
        self.distance_since_dab = 0.0;
        Vec::new()
    }

    fn walk_segment(&mut self, from: &StrokeSample, to: &StrokeSample) -> Option<Vec<Dab>> {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let length = dx.hypot(dy);
        if !length.is_finite() {
            return None;
        }
        if length <= 0.0 {
            return Some(Vec::new());
        }
        let elapsed_ms =
            f64::from(to.time_ms.saturating_sub(from.time_ms)).max(MIN_SPEED_INTERVAL_MS);
        let segment = Segment {
            start: PenPoint::of(from),
            end: PenPoint::of(to),
            length,
            shortest_step: length / (MAX_DABS_PER_SEGMENT - 1) as f64,
            direction: dy.atan2(dx) as f32,
            speed: (length / elapsed_ms) as f32,
        };
        let mut travelled = 0.0;
        Some(std::iter::from_fn(|| self.next_dab(&segment, &mut travelled)).collect())
    }

    fn next_dab(&mut self, segment: &Segment, travelled: &mut f64) -> Option<Dab> {
        let length = segment.length;
        let pressure_here = lerp_f32(
            segment.start.pressure,
            segment.end.pressure,
            *travelled / length,
        );
        let step = self.geometry.step(pressure_here).max(segment.shortest_step);
        let needed = (step - self.distance_since_dab).max(0.0);
        if *travelled + needed > length + ARC_LENGTH_TOLERANCE_PX {
            self.distance_since_dab += length - *travelled;
            return None;
        }
        *travelled = (*travelled + needed).min(length);
        self.distance_since_dab = 0.0;
        let at = segment.start.towards(&segment.end, *travelled / length);
        Some(self.dab_at(&at, segment.direction, segment.speed))
    }

    fn dab_at(&self, point: &PenPoint, direction: f32, speed: f32) -> Dab {
        let angles = pen_angles(point.tilt_x_deg, point.tilt_y_deg);
        Dab {
            x: point.x as f32,
            y: point.y as f32,
            radius: self.geometry.diameter(point.pressure) / 2.0,
            pressure: self.geometry.pressure_curve.apply(point.pressure),
            altitude: angles.altitude_rad,
            azimuth: angles.azimuth_rad,
            direction,
            speed,
        }
    }
}

fn is_finite(sample: &StrokeSample) -> bool {
    sample.x.is_finite()
        && sample.y.is_finite()
        && sample.pressure.is_finite()
        && sample.tilt_x_deg.is_finite()
        && sample.tilt_y_deg.is_finite()
}

fn lerp_f32(a: f32, b: f32, t: f64) -> f32 {
    a + (b - a) * t as f32
}
