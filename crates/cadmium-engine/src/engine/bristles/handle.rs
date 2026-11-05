use crate::stroke::Dab;

const MAX_STEP_MS: f64 = 50.0;
const MAX_SUBSTEPS: u32 = 16;
const SUBSTEP_TRAVEL_SHARE: f32 = 0.5;
const MILLISECONDS_PER_SECOND: f64 = 1000.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HandlePose {
    pub(super) ferrule_px: [f32; 3],
    pub(super) axis: [f32; 3],
}

pub(super) struct HandleGauge {
    pub(super) reach_px: f32,
    pub(super) ferrule_radius_px: f32,
    pub(super) press_depth: f32,
}

impl HandleGauge {
    pub(super) fn pose(&self, dab: &Dab) -> HandlePose {
        self.pose_at(dab, dab.pressure)
    }

    pub(super) fn lifted(&self, dab: &Dab) -> HandlePose {
        self.pose_at(dab, 0.0)
    }

    fn pose_at(&self, dab: &Dab, pressure: f32) -> HandlePose {
        HandlePose {
            ferrule_px: [dab.x, dab.y, self.height_px(pressure, dab.altitude)],
            axis: pen_axis(dab.altitude, dab.azimuth),
        }
    }

    fn height_px(&self, pressure: f32, altitude: f32) -> f32 {
        let hanging_px = self.reach_px * altitude.sin().clamp(0.0, 1.0);
        let pressed_px = hanging_px * (1.0 - pressure.clamp(0.0, 1.0) * self.press_depth);
        let ferrule_clearance_px = self.ferrule_radius_px * altitude.cos().max(0.0);
        pressed_px.max(ferrule_clearance_px)
    }
}

pub(super) fn pen_axis(altitude: f32, azimuth: f32) -> [f32; 3] {
    let (sin_altitude, cos_altitude) = altitude.sin_cos();
    let (sin_azimuth, cos_azimuth) = azimuth.sin_cos();
    [
        cos_altitude * cos_azimuth,
        cos_altitude * sin_azimuth,
        sin_altitude,
    ]
}

#[derive(Default)]
pub(super) struct HandleClock {
    previous_time_ms: Option<f64>,
    previous_ferrule_px: Option<[f32; 3]>,
}

impl HandleClock {
    pub(super) fn begin_stroke(&mut self) {
        self.previous_time_ms = None;
    }

    pub(super) fn forget_pose(&mut self) {
        self.previous_ferrule_px = None;
    }

    pub(super) fn elapsed_seconds(&mut self, time_ms: f32) -> f32 {
        if !time_ms.is_finite() {
            return 0.0;
        }
        let time_ms = f64::from(time_ms);
        let elapsed_ms = self.previous_time_ms.map_or(0.0, |previous_ms| {
            (time_ms - previous_ms).clamp(0.0, MAX_STEP_MS)
        });
        self.previous_time_ms = Some(time_ms);
        (elapsed_ms / MILLISECONDS_PER_SECOND) as f32
    }

    pub(super) fn later_seconds(&mut self, time_ms: f32) -> Option<f32> {
        let previous_ms = self.previous_time_ms?;
        let later = time_ms.is_finite() && f64::from(time_ms) > previous_ms;
        later.then(|| self.elapsed_seconds(time_ms))
    }

    pub(super) fn travel_px(&mut self, to: [f32; 3], resting: [f32; 3]) -> f32 {
        let from = self.previous_ferrule_px.unwrap_or(resting);
        self.previous_ferrule_px = Some(to);
        let squared: f32 = (0..3).map(|axis| (to[axis] - from[axis]).powi(2)).sum();
        squared.sqrt()
    }
}

pub(super) fn substeps(travel_px: f32, mean_radius_px: f32) -> u32 {
    let stride_px = SUBSTEP_TRAVEL_SHARE * mean_radius_px;
    if !travel_px.is_finite() || stride_px <= 0.0 {
        return MAX_SUBSTEPS;
    }
    ((travel_px / stride_px).ceil() as u32).clamp(1, MAX_SUBSTEPS)
}

#[cfg(test)]
mod tests;
