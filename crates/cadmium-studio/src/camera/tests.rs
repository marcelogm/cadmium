use glam::{Vec3, Vec4Swizzles};

use super::OrbitCamera;
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
        let ferrule = REACH / 6.0;
        let brush = REACH + 1.5 * ferrule + 4.0 * ferrule;
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
    assert!(close(yaw(after) - yaw(before), 0.1));
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
    assert!(close(camera.distance(), reset * 0.3));
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
