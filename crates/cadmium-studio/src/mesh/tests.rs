use super::{Mesh, ferrule_mesh, handle_mesh};

use crate::proportions::{FERRULE_LENGTH, HANDLE_CLEARANCE};

const R: f32 = 10.0;
const EPS: f32 = 1e-3;

fn assert_valid(mesh: &Mesh) {
    assert!(!mesh.vertices.is_empty());
    assert_eq!(mesh.indices.len() % 3, 0);
    assert!(
        mesh.indices
            .iter()
            .all(|&i| (i as usize) < mesh.vertices.len())
    );
    for v in &mesh.vertices {
        let n = v.normal;
        assert!(((n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() - 1.0).abs() < EPS);
    }
}

fn span(mesh: &Mesh, axis: usize) -> (f32, f32) {
    let values = mesh.vertices.iter().map(|v| v.position[axis]);
    (
        values.clone().fold(f32::MAX, f32::min),
        values.fold(f32::MIN, f32::max),
    )
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn assert_wound_outward(mesh: &Mesh) {
    for tri in mesh.indices.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|k| mesh.vertices[tri[k] as usize]);
        let face = cross(sub(b.position, a.position), sub(c.position, a.position));
        let average = [0, 1, 2].map(|k| a.normal[k] + b.normal[k] + c.normal[k]);
        assert!(dot(face, average) > 0.0, "inward triangle {tri:?}");
    }
}

#[test]
fn ferrule_is_a_valid_tube_of_the_right_length() {
    let mesh = ferrule_mesh([R, R], R);
    assert_valid(&mesh);
    let (low, high) = span(&mesh, 2);
    assert!(low.abs() < EPS);
    assert!((high - FERRULE_LENGTH * R).abs() < EPS);
}

#[test]
fn flat_ferrule_is_thinner_than_round() {
    let flat = span(&ferrule_mesh([R, 0.25 * R], R), 1);
    let round = span(&ferrule_mesh([R, R], R), 1);
    assert!(flat.1 - flat.0 < round.1 - round.0);
}

#[test]
fn handle_sits_above_the_ferrule() {
    let mesh = handle_mesh(R);
    assert_valid(&mesh);
    let (low, high) = span(&mesh, 2);
    assert!((low - FERRULE_LENGTH * R).abs() < EPS);
    assert!((high - (FERRULE_LENGTH + HANDLE_CLEARANCE) * R).abs() < EPS);
    let widest = mesh
        .vertices
        .iter()
        .map(|v| v.position[0].hypot(v.position[1]));
    assert!(widest.fold(0.0, f32::max) <= 1.2 * R + EPS);
}

#[test]
fn triangles_wind_outward() {
    assert_wound_outward(&ferrule_mesh([R, 0.25 * R], R));
    assert_wound_outward(&ferrule_mesh([R, R], R));
    assert_wound_outward(&handle_mesh(R));
}

#[test]
fn ferrule_vertices_lie_on_the_ellipse() {
    let (a, b) = (R + 0.08 * R, 0.25 * R + 0.08 * R);
    for v in &ferrule_mesh([R, 0.25 * R], R).vertices {
        let [x, y, _] = v.position;
        assert!(((x / a).powi(2) + (y / b).powi(2) - 1.0).abs() < EPS);
    }
}

#[test]
fn ferrule_normals_are_perpendicular_to_the_ellipse_and_outward() {
    let (a, b) = (R + 0.08 * R, 0.25 * R + 0.08 * R);
    for v in &ferrule_mesh([R, 0.25 * R], R).vertices {
        let [x, y, _] = v.position;
        let tangent = [-a * (y / b), b * (x / a), 0.0];
        assert!(dot(v.normal, tangent).abs() < EPS * a);
        assert!(dot(v.normal, [x, y, 0.0]) > 0.0);
    }
}

#[test]
fn handle_side_normals_point_away_from_the_axis() {
    let mesh = handle_mesh(R);
    let side = mesh.vertices.len() - 34;
    for v in &mesh.vertices[..side] {
        assert!(dot(v.normal, [v.position[0], v.position[1], 0.0]) > 0.0);
    }
}

fn assert_finite(mesh: &Mesh) {
    for v in &mesh.vertices {
        assert!(v.position.iter().all(|c| c.is_finite()));
    }
}

#[test]
fn degenerate_sizes_still_give_finite_unit_normals() {
    for bad in [0.0, -5.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let meshes = [
            ferrule_mesh([bad, bad], bad),
            ferrule_mesh([R, bad], R),
            handle_mesh(bad),
        ];
        for mesh in &meshes {
            assert_valid(mesh);
            assert_wound_outward(mesh);
            assert_finite(mesh);
        }
    }
}
