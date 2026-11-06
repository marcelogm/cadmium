//! The orbit camera that frames the brush over its demo stroke.
//!
//! The scene lives in canvas pixel space: x right, y down, z up (height in
//! px). Seen from above that frame is left-handed, so the view mirrors y
//! before a right-handed `look_at`; without it the image comes out mirrored.
//! The mirror flips triangle winding, so renderers must not rely on back-face
//! culling.

#[cfg(test)]
mod tests;

use glam::camera::rh::{proj::directx, view::look_at_mat4};
use glam::{Mat4, Vec3};

use crate::StrokeBox;
use crate::proportions::{FERRULE_LENGTH, HANDLE_CLEARANCE};

/// Yaw of the reset view, around the canvas normal.
const RESET_YAW: f32 = -35.0 * DEG;
/// Pitch of the reset view, above the canvas plane.
const RESET_PITCH: f32 = 35.0 * DEG;
/// Vertical field of view.
const FOV_Y: f32 = 45.0 * DEG;
/// Orbit speed, in radians per dragged pixel.
const ORBIT_RAD_PER_PX: f32 = 0.01;
/// Pitch limits: never under the canvas, never flipping over the top.
const PITCH_MIN: f32 = 10.0 * DEG;
const PITCH_MAX: f32 = 85.0 * DEG;
/// Distance factor of one zoom step; positive steps move closer.
const ZOOM_STEP: f32 = 0.9;
/// Zoom limits, as factors of the reset distance.
///
/// The closest zoom keeps the eye outside the framing sphere by at least the
/// near distance, so the near plane never cuts the box or the brush. The
/// distance is `zoom · R / sin(h)`, `h` the smaller half field of view (at
/// most 22.5°), and `distance · (1 − NEAR_FACTOR) ≥ R` needs
/// `zoom ≥ sin(22.5°) / 0.9 ≈ 0.425`. The one exception is a brush leaning
/// past the sphere at a box corner (see `framing`).
const ZOOM_MIN: f32 = 0.45;
const ZOOM_MAX: f32 = 4.0;
/// Near plane, as a factor of the distance; the far plane is at infinity, so
/// a low pitch looking across a large canvas never clips it.
const NEAR_FACTOR: f32 = 0.1;
/// Ferrule radius, as a share of the reach: a rough size for the framing.
const FERRULE_SHARE: f32 = 1.0 / 6.0;

const DEG: f32 = std::f32::consts::PI / 180.0;

/// Mirrors canvas space (y down) into a right-handed frame (y up) and back.
const MIRROR: Vec3 = Vec3::new(1.0, -1.0, 1.0);

/// An orbit around a target in canvas space, framed on a stroke and a brush.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitCamera {
    target: Vec3,
    radius: f32,
    aspect: f32,
    yaw: f32,
    pitch: f32,
    /// Distance as a factor of the reset distance for the current aspect.
    zoom: f32,
}

impl OrbitCamera {
    /// The reset view framing `stroke_box` and a brush of `reach_px` over it.
    ///
    /// The framing sphere is centred on the box centre at half the reach and
    /// holds both the box corners and the brush (reach, plus a ferrule of
    /// radius reach/6 and its clearance). The sphere is only measured upward
    /// from the centre: a brush leaning out over a box corner may poke past
    /// it, which the margin of the field of view hides.
    pub fn framing(stroke_box: StrokeBox, reach_px: f32, aspect: f32) -> OrbitCamera {
        let reach = finite_or(reach_px, 0.0).max(0.0);
        let centre_x = (stroke_box.x + stroke_box.width / 2.0) as f32;
        let centre_y = (stroke_box.y + stroke_box.height / 2.0) as f32;
        let target = Vec3::new(centre_x, centre_y, reach / 2.0);
        let half_w = (stroke_box.width / 2.0) as f32;
        let half_h = (stroke_box.height / 2.0) as f32;
        let corner = Vec3::new(half_w, half_h, reach / 2.0).length();
        let ferrule = reach * FERRULE_SHARE;
        let brush = reach + (FERRULE_LENGTH + HANDLE_CLEARANCE) * ferrule;
        OrbitCamera {
            target,
            radius: finite_or(corner.max(brush), 1.0).max(1.0),
            aspect: valid_aspect(aspect, 1.0),
            yaw: RESET_YAW,
            pitch: RESET_PITCH,
            zoom: 1.0,
        }
    }

    /// Restores the reset view, keeping the framing and the aspect.
    pub fn reset(&mut self) {
        self.yaw = RESET_YAW;
        self.pitch = RESET_PITCH;
        self.zoom = 1.0;
    }

    /// Updates the viewport aspect (width / height); ignores invalid values.
    pub fn set_aspect(&mut self, aspect: f32) {
        self.aspect = valid_aspect(aspect, self.aspect);
    }

    /// Orbits by a pointer drag of `dx_px`, `dy_px` pixels. The scene follows
    /// the cursor: dragging right turns its near side right, dragging down
    /// tilts the view toward top-down.
    pub fn orbit(&mut self, dx_px: f32, dy_px: f32) {
        if !(dx_px.is_finite() && dy_px.is_finite()) {
            return;
        }
        self.yaw = (self.yaw - dx_px * ORBIT_RAD_PER_PX) % std::f32::consts::TAU;
        self.pitch = (self.pitch + dy_px * ORBIT_RAD_PER_PX).clamp(PITCH_MIN, PITCH_MAX);
    }

    /// Zooms by `steps` (positive = closer), within the zoom limits.
    pub fn zoom(&mut self, steps: f32) {
        if steps.is_finite() {
            self.zoom = (self.zoom * ZOOM_STEP.powf(steps)).clamp(ZOOM_MIN, ZOOM_MAX);
        }
    }

    /// The orbit centre, in canvas space.
    pub fn target(&self) -> Vec3 {
        self.target
    }

    /// The eye distance from the target.
    pub fn distance(&self) -> f32 {
        self.reset_distance() * self.zoom
    }

    /// The eye position, in canvas space.
    pub fn eye(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let offset = Vec3::new(cos_pitch * sin_yaw, cos_pitch * cos_yaw, sin_pitch);
        self.target + offset * self.distance()
    }

    /// The camera's right vector, in canvas space.
    pub fn right(&self) -> Vec3 {
        self.axes()[0]
    }

    /// Canvas space to clip space, with wgpu's 0..1 depth range.
    pub fn view_proj(&self) -> Mat4 {
        // wgpu shares Direct3D's clip space: y up, depth 0..1.
        let proj = directx::perspective_infinite(FOV_Y, self.aspect, self.near());
        proj * self.view()
    }

    /// The ray under the NDC point (`ndc_x`, `ndc_y`), in canvas space: it
    /// starts at the eye and its direction is a unit vector.
    ///
    /// Built from the view basis rather than by inverting `view_proj`, which
    /// loses too much precision in f32 far from the eye.
    pub fn ray(&self, ndc_x: f32, ndc_y: f32) -> (Vec3, Vec3) {
        let [right, up, back] = self.axes();
        let half_height = (FOV_Y / 2.0).tan();
        let half_width = half_height * self.aspect;
        let dir = right * (ndc_x * half_width) + up * (ndc_y * half_height) - back;
        (self.eye(), dir.normalize())
    }

    /// The camera's right, up and back unit vectors, in canvas space: the
    /// rows of the view rotation. A look-at's rows are orthonormal and the
    /// mirror keeps their length, so they need no normalising.
    fn axes(&self) -> [Vec3; 3] {
        let view = self.view();
        [0, 1, 2].map(|row| view.row(row).truncate())
    }

    /// The near plane's distance from the eye.
    fn near(&self) -> f32 {
        self.distance() * NEAR_FACTOR
    }

    /// Canvas space to view space: a right-handed look-at in the y-mirrored
    /// frame, after the mirror.
    fn view(&self) -> Mat4 {
        let look = look_at_mat4(self.eye() * MIRROR, self.target * MIRROR, Vec3::Z);
        look * Mat4::from_scale(MIRROR)
    }

    /// The distance that fits the framing sphere in both fields of view.
    fn reset_distance(&self) -> f32 {
        let half_y = FOV_Y / 2.0;
        let half_x = (self.aspect * half_y.tan()).atan();
        self.radius / half_y.min(half_x).sin()
    }
}

/// `aspect` when it is a usable width / height, else `fallback`.
fn valid_aspect(aspect: f32, fallback: f32) -> f32 {
    if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        fallback
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}
