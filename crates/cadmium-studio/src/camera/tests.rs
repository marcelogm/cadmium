use glam::{Vec3, Vec4Swizzles};

use super::{
    FERRULE_LENGTH, FERRULE_SHARE, FOV_Y, HANDLE_CLEARANCE, NEAR_FACTOR, OrbitCamera, PITCH_MIN,
    ZOOM_MIN,
};
use crate::StrokeBox;

const BOX: StrokeBox = StrokeBox {
    x: 40.0,
    y: 30.0,
    width: 200.0,
    height: 120.0,
};
const REACH: f32 = 48.0;

fn camera(aspect: f32) -> OrbitCamera {
    OrbitCamera::framing(BOX, REACH, aspect)
}

fn centre() -> (f32, f32) {
    let cx = (BOX.x + BOX.width / 2.0) as f32;
    let cy = (BOX.y + BOX.height / 2.0) as f32;
    (cx, cy)
}

/// NDC of a canvas-space point.
fn ndc(camera: &OrbitCamera, point: Vec3) -> Vec3 {
    let clip = camera.view_proj() * point.extend(1.0);
    clip.xyz() / clip.w
}

fn deg(value: f32) -> f32 {
    value.to_radians()
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-4 * a.abs().max(b.abs()).max(1.0)
}

#[test]
fn the_target_is_the_box_centre_at_half_the_reach() {
    let (cx, cy) = centre();
    assert_eq!(camera(1.5).target(), Vec3::new(cx, cy, REACH / 2.0));
}

#[test]
fn the_reset_view_looks_from_the_near_edge_at_the_reset_angles() {
    let camera = camera(1.5);
    let offset = (camera.eye() - camera.target()) / camera.distance();
    assert!(close(offset.z, deg(35.0).sin()));
    let horizontal = offset.truncate().normalize();
    assert!(close(horizontal.x, deg(-35.0).sin()));
    assert!(close(horizontal.y, deg(-35.0).cos()));
}

#[test]
fn the_distance_fits_the_framing_sphere_in_both_fields_of_view() {
    for aspect in [0.5, 1.0, 1.5, 3.0] {
        let camera = camera(aspect);
        let ferrule = REACH * FERRULE_SHARE;
        let brush = REACH + (FERRULE_LENGTH + HANDLE_CLEARANCE) * ferrule;
        let corner = Vec3::new(100.0, 60.0, REACH / 2.0).length();
        let radius = corner.max(brush);
        let half_y = deg(22.5);
        let half_x = (aspect * half_y.tan()).atan();
        let expected = radius / half_y.min(half_x).sin();
        assert!(close(camera.distance(), expected), "aspect {aspect}");
    }
}

#[test]
fn the_brush_widens_the_framing_over_a_small_box() {
    let tiny = StrokeBox {
        x: 0.0,
        y: 0.0,
        width: 2.0,
        height: 2.0,
    };
    let near = OrbitCamera::framing(tiny, 10.0, 1.0);
    let far = OrbitCamera::framing(tiny, 20.0, 1.0);
    assert!(close(far.distance(), 2.0 * near.distance()));
}

#[test]
fn the_reset_view_is_neither_mirrored_nor_flipped() {
    let camera = camera(1.5);
    let (cx, cy) = centre();
    let (r0, h0) = ((BOX.width / 2.0) as f32, (BOX.height / 2.0) as f32);
    let left = ndc(&camera, Vec3::new(cx - r0, cy, 0.0));
    let right = ndc(&camera, Vec3::new(cx + r0, cy, 0.0));
    assert!(left.x < right.x, "mirrored: {left} vs {right}");
    let near = ndc(&camera, Vec3::new(cx, cy + h0, 0.0));
    let far = ndc(&camera, Vec3::new(cx, cy - h0, 0.0));
    assert!(near.y < far.y, "flipped: {near} vs {far}");
    let base = ndc(&camera, Vec3::new(cx, cy, 0.0));
    let tip = ndc(&camera, Vec3::new(cx, cy, REACH));
    assert!(tip.y > base.y, "upside down: {tip} vs {base}");
}

#[test]
fn every_box_corner_is_on_screen_in_the_reset_view() {
    for aspect in [0.5, 1.5, 3.0] {
        let camera = camera(aspect);
        for (x, y) in [
            (BOX.x, BOX.y),
            (BOX.x + BOX.width, BOX.y),
            (BOX.x, BOX.y + BOX.height),
            (BOX.x + BOX.width, BOX.y + BOX.height),
        ] {
            let point = ndc(&camera, Vec3::new(x as f32, y as f32, 0.0));
            assert!(
                point.x.abs() <= 1.0 && point.y.abs() <= 1.0,
                "{aspect}: {point}"
            );
            assert!((0.0..=1.0).contains(&point.z), "{aspect}: depth {point}");
        }
    }
}

#[test]
fn orbit_turns_both_angles_by_a_hundredth_of_a_radian_per_pixel() {
    let mut camera = camera(1.5);
    let before = camera.eye() - camera.target();
    camera.orbit(10.0, 5.0);
    let after = camera.eye() - camera.target();
    let yaw = |v: Vec3| v.x.atan2(v.y);
    let pitch = |v: Vec3| (v.z / v.length()).asin();
    assert!(close(yaw(after) - yaw(before), -0.1));
    assert!(close(pitch(after) - pitch(before), 0.05));
}

#[test]
fn orbit_keeps_the_pitch_between_ten_and_eighty_five_degrees() {
    let mut camera = camera(1.5);
    let pitch = |c: &OrbitCamera| ((c.eye().z - c.target().z) / c.distance()).asin();
    camera.orbit(0.0, 10_000.0);
    assert!(close(pitch(&camera), deg(85.0)));
    camera.orbit(0.0, -10_000.0);
    assert!(close(pitch(&camera), deg(10.0)));
}

#[test]
fn zoom_scales_the_distance_within_its_limits() {
    let mut camera = camera(1.5);
    let reset = camera.distance();
    camera.zoom(2.0);
    assert!(close(camera.distance(), reset * 0.81));
    camera.zoom(-3.0);
    assert!(close(camera.distance(), reset / 0.9));
    camera.zoom(1000.0);
    assert!(close(camera.distance(), reset * ZOOM_MIN));
    camera.zoom(-1000.0);
    assert!(close(camera.distance(), reset * 4.0));
}

#[test]
fn reset_restores_the_reset_view() {
    let fresh = camera(1.5);
    let mut camera = fresh;
    camera.orbit(123.0, -45.0);
    camera.zoom(3.0);
    camera.reset();
    assert_eq!(camera, fresh);
}

#[test]
fn set_aspect_refits_and_ignores_invalid_values() {
    let mut camera = camera(1.5);
    camera.set_aspect(0.5);
    assert_eq!(camera, OrbitCamera::framing(BOX, REACH, 0.5));
    for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        camera.set_aspect(invalid);
        assert_eq!(camera, OrbitCamera::framing(BOX, REACH, 0.5));
    }
}

#[test]
fn the_centre_ray_passes_through_the_target() {
    let camera = camera(1.5);
    let (origin, dir) = camera.ray(0.0, 0.0);
    assert!(close(dir.length(), 1.0));
    let along = (camera.target() - origin).dot(dir);
    let miss = origin + dir * along - camera.target();
    assert!(miss.length() < 1e-3, "missed by {miss}");
}

#[test]
fn a_ray_right_of_centre_lands_right_of_the_target() {
    let camera = camera(1.5);
    let target = camera.target();
    let (origin, dir) = camera.ray(0.5, 0.0);
    let hit = origin + dir * ((target.z - origin.z) / dir.z);
    assert!((hit - target).dot(camera.right()) > 0.0);
}

#[test]
fn the_right_vector_in_the_reset_view_runs_along_the_canvas() {
    let right = camera(1.5).right();
    assert!(close(right.z, 0.0));
    assert!(right.x > 0.0);
    assert!(close(right.length(), 1.0));
}

#[test]
fn a_ray_projects_back_onto_its_ndc_point() {
    let mut camera = camera(1.5);
    camera.orbit(40.0, -20.0);
    for (x, y) in [(0.5, 0.0), (-0.7, 0.3), (0.2, -0.9)] {
        let (origin, dir) = camera.ray(x, y);
        let point = ndc(&camera, origin + dir * camera.distance());
        assert!(
            (point.x - x).abs() < 1e-4 && (point.y - y).abs() < 1e-4,
            "{x},{y}: {point}"
        );
    }
}

#[test]
fn dragging_right_turns_the_near_side_of_the_scene_right() {
    let mut camera = camera(1.5);
    let (cx, cy) = centre();
    let near_side = Vec3::new(cx, cy + 40.0, 0.0);
    let before = ndc(&camera, near_side);
    camera.orbit(20.0, 0.0);
    let after = ndc(&camera, near_side);
    assert!(after.x > before.x, "{before} -> {after}");
}

#[test]
fn dragging_down_tilts_the_view_toward_top_down() {
    let mut camera = camera(1.5);
    let height = |c: &OrbitCamera| (c.eye().z - c.target().z) / c.distance();
    let before = height(&camera);
    camera.orbit(0.0, 20.0);
    assert!(height(&camera) > before);
}

/// Clip depth of `point`, or `None` behind the eye.
fn clip_depth(camera: &OrbitCamera, point: Vec3) -> Option<f32> {
    let clip = camera.view_proj() * point.extend(1.0);
    (clip.w > 0.0).then(|| clip.z / clip.w)
}

#[test]
fn a_low_close_view_never_clips_the_far_side_of_a_large_canvas() {
    let small = StrokeBox {
        x: 1000.0,
        y: 500.0,
        width: 48.0,
        height: 32.0,
    };
    let mut camera = OrbitCamera::framing(small, 24.0, 1.5);
    camera.orbit(0.0, -10_000.0);
    camera.zoom(1000.0);
    assert!(close(camera.pitch, PITCH_MIN) && close(camera.zoom, ZOOM_MIN));
    let corners = [(0.0, 0.0), (2048.0, 0.0), (0.0, 1024.0), (2048.0, 1024.0)];
    for yaw_px in [0.0, 90.0, 180.0, 270.0] {
        camera.orbit(yaw_px, 0.0);
        let depths = corners.map(|(x, y)| clip_depth(&camera, Vec3::new(x, y, 0.0)));
        for depth in depths.into_iter().flatten() {
            assert!(
                (0.0..=1.0).contains(&depth),
                "yaw {}: depth {depth}",
                camera.yaw
            );
        }
    }
}

#[test]
fn the_closest_zoom_keeps_the_eye_outside_the_framing_sphere_by_the_near_distance() {
    for aspect in [0.5, 1.0, 1.5, 3.0] {
        for pitch_px in [-10_000.0, 10_000.0] {
            let mut camera = camera(aspect);
            camera.orbit(0.0, pitch_px);
            camera.zoom(1000.0);
            assert!(close(camera.zoom, ZOOM_MIN));
            let clearance = camera.distance() - camera.near();
            assert!(clearance >= camera.radius, "aspect {aspect}: {clearance}");
        }
    }
}

#[test]
fn the_closest_zoom_meets_its_bound_at_the_widest_limiting_half_fov() {
    // The half field of view that limits the distance is at most FOV_Y / 2;
    // the bound sin(h) / (1 - NEAR_FACTOR) is largest there.
    let bound = (FOV_Y / 2.0).sin() / (1.0 - NEAR_FACTOR);
    assert!(ZOOM_MIN >= bound, "{ZOOM_MIN} < {bound}");
}
