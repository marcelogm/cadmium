//! Ferrule and handle meshes, plus the hair tint, for the 3D brush view.
//!
//! Ferrule-local space: x across the head, y along its thickness, z up the
//! brush axis. A vertex shader poses them with the engine's tilt and spin.
//! Neither mesh has a bottom cap (hidden by the hairs); the ferrule has no top
//! cap either (hidden by the handle); the handle has a top cap.

use std::f32::consts::TAU;

use bytemuck::{Pod, Zeroable};
use cadmium_engine::brush::HairSpec;

/// The ferrule's length above the reach, in ferrule radii.
pub(crate) const FERRULE_LENGTH: f32 = 1.5;
/// Clearance above the ferrule for the handle, in ferrule radii.
pub(crate) const HANDLE_CLEARANCE: f32 = 4.0;

const SEGMENTS: u32 = 32;
/// Ferrule wall margin around the roots, in ferrule radii.
const FERRULE_MARGIN: f32 = 0.08;
/// Handle profile as (height share of the clearance, radius in ferrule radii).
const HANDLE_PROFILE: [(f32, f32); 6] = [
    (0.0, 0.9),
    (0.15, 1.05),
    (0.4, 1.2),
    (0.7, 1.1),
    (0.9, 0.85),
    (1.0, 0.7),
];

const TINTS: [(HairSpec, [f32; 3]); 5] = [
    (HairSpec::SYNTHETIC, [0.55, 0.38, 0.12]),
    (HairSpec::HOG, [0.80, 0.72, 0.55]),
    (HairSpec::SABLE, [0.35, 0.12, 0.05]),
    (HairSpec::MONGOOSE, [0.30, 0.25, 0.20]),
    (HairSpec::SQUIRREL, [0.10, 0.07, 0.05]),
];

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug, PartialEq)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

fn ring_angles() -> impl Iterator<Item = (f32, f32)> {
    (0..=SEGMENTS).map(|i| (i as f32 / SEGMENTS as f32 * TAU).sin_cos())
}

/// Links ring `a` to the ring `b` above it (each `SEGMENTS + 1` long), with
/// triangles counter-clockwise seen from outside.
fn stitch(indices: &mut Vec<u32>, a: u32, b: u32) {
    for i in 0..SEGMENTS {
        let (a0, a1, b0, b1) = (a + i, a + i + 1, b + i, b + i + 1);
        indices.extend([a0, a1, b0, a1, b1, b0]);
    }
}

/// An elliptic tube around the bristle roots, open at both ends.
pub fn ferrule_mesh(root_half_extent_px: [f32; 2], ferrule_radius_px: f32) -> Mesh {
    let r = ferrule_radius_px;
    let a = root_half_extent_px[0] + FERRULE_MARGIN * r;
    let b = root_half_extent_px[1] + FERRULE_MARGIN * r;
    let mut mesh = Mesh::default();
    for z in [0.0, FERRULE_LENGTH * r] {
        for (sin, cos) in ring_angles() {
            let normal = [cos / a, sin / b, 0.0];
            let len = normal[0].hypot(normal[1]);
            mesh.vertices.push(MeshVertex {
                position: [a * cos, b * sin, z],
                normal: [normal[0] / len, normal[1] / len, 0.0],
            });
        }
    }
    stitch(&mut mesh.indices, 0, SEGMENTS + 1);
    mesh
}

/// A lathe stub above the ferrule, swelling then tapering, with a top cap.
pub fn handle_mesh(ferrule_radius_px: f32) -> Mesh {
    let r = ferrule_radius_px;
    let z0 = FERRULE_LENGTH * r;
    let rise = HANDLE_CLEARANCE * r;
    let points: Vec<(f32, f32)> = HANDLE_PROFILE
        .iter()
        .map(|&(t, radius)| (z0 + t * rise, radius * r))
        .collect();
    let mut mesh = Mesh::default();
    for (k, &(z, radius)) in points.iter().enumerate() {
        let before = points[k.saturating_sub(1)];
        let after = points[(k + 1).min(points.len() - 1)];
        let (dz, dr) = (after.0 - before.0, after.1 - before.1);
        let len = dz.hypot(dr);
        let (nr, nz) = (dz / len, -dr / len);
        for (sin, cos) in ring_angles() {
            mesh.vertices.push(MeshVertex {
                position: [radius * cos, radius * sin, z],
                normal: [nr * cos, nr * sin, nz],
            });
        }
    }
    for k in 0..points.len() as u32 - 1 {
        stitch(
            &mut mesh.indices,
            k * (SEGMENTS + 1),
            (k + 1) * (SEGMENTS + 1),
        );
    }
    add_top_cap(&mut mesh, points[points.len() - 1]);
    mesh
}

fn add_top_cap(mesh: &mut Mesh, (z, radius): (f32, f32)) {
    let ring = mesh.vertices.len() as u32;
    let normal = [0.0, 0.0, 1.0];
    for (sin, cos) in ring_angles() {
        mesh.vertices.push(MeshVertex {
            position: [radius * cos, radius * sin, z],
            normal,
        });
    }
    let centre = mesh.vertices.len() as u32;
    mesh.vertices.push(MeshVertex {
        position: [0.0, 0.0, z],
        normal,
    });
    for i in 0..SEGMENTS {
        mesh.indices.extend([centre, ring + i, ring + i + 1]);
    }
}

/// Linear-light bristle colour for the factory hair nearest in stiffness; a
/// tie goes to the earlier factory hair (synthetic, hog, sable, mongoose,
/// squirrel).
pub fn hair_tint(hair: &HairSpec) -> [f32; 3] {
    let distance = |h: &HairSpec| (h.stiffness - hair.stiffness).abs();
    TINTS
        .iter()
        .min_by(|a, b| distance(&a.0).total_cmp(&distance(&b.0)))
        .map_or(TINTS[0].1, |t| t.1)
}

#[cfg(test)]
mod tests;
