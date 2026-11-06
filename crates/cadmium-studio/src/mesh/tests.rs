use cadmium_engine::brush::HairSpec;

use super::{FERRULE_LENGTH, HANDLE_CLEARANCE, Mesh, ferrule_mesh, hair_tint, handle_mesh};

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

#[test]
fn ferrule_is_a_valid_tube_of_the_right_length() {
    let mesh = ferrule_mesh([R, R], R);
    assert_valid(&mesh);
    let (low, high) = span(&mesh, 2);
    assert!(low.abs() < EPS);
    assert!((high - FERRULE_LENGTH * R).abs() < EPS);
}

#[test]
fn ferrule_stays_inside_its_ellipse() {
    let (a, b) = (R + 0.08 * R, 0.25 * R + 0.08 * R);
    let mesh = ferrule_mesh([R, 0.25 * R], R);
    for v in &mesh.vertices {
        let [x, y, _] = v.position;
        assert!((x / a).powi(2) + (y / b).powi(2) < 1.0 + EPS);
    }
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
fn factory_hairs_get_their_tint() {
    assert_eq!(hair_tint(&HairSpec::SYNTHETIC), [0.55, 0.38, 0.12]);
    assert_eq!(hair_tint(&HairSpec::HOG), [0.80, 0.72, 0.55]);
    assert_eq!(hair_tint(&HairSpec::SABLE), [0.35, 0.12, 0.05]);
    assert_eq!(hair_tint(&HairSpec::MONGOOSE), [0.30, 0.25, 0.20]);
    assert_eq!(hair_tint(&HairSpec::SQUIRREL), [0.10, 0.07, 0.05]);
}

#[test]
fn custom_hair_takes_the_nearest_stiffness() {
    let stiff = HairSpec {
        stiffness: 0.99,
        ..HairSpec::SYNTHETIC
    };
    assert_eq!(hair_tint(&stiff), hair_tint(&HairSpec::HOG));
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

#[test]
fn a_stiffness_tie_goes_to_the_earlier_factory_hair() {
    let (early, late) = (HairSpec::SABLE, HairSpec::SQUIRREL);
    let middle = (early.stiffness + late.stiffness) / 2.0;
    let hair = HairSpec {
        stiffness: middle,
        ..early
    };
    assert_eq!(
        (early.stiffness - middle).abs(),
        (late.stiffness - middle).abs()
    );
    assert_eq!(hair_tint(&hair), hair_tint(&early));
}
