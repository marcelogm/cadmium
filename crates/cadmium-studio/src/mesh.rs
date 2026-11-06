//! Ferrule and handle meshes for the 3D brush view.
//!
//! Neither mesh has a bottom cap (hidden by the hairs); the ferrule has no top
//! cap either (hidden by the handle); the handle has a top cap.

use std::f32::consts::TAU;

use bytemuck::{Pod, Zeroable};

use crate::proportions::{FERRULE_LENGTH, FERRULE_MARGIN, HANDLE_CLEARANCE, HANDLE_PROFILE};

const SEGMENTS: u32 = 32;
/// Smallest radius or extent a mesh is built with, in px.
const MIN_SIZE: f32 = 1e-3;

/// A mesh vertex in ferrule-local space (x across the head, y along its
/// thickness, z up the brush axis), in px; `normal` has unit length.
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug, PartialEq)]
pub(crate) struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

/// An indexed triangle list in ferrule-local space (x across the head, y along
/// its thickness, z up the brush axis), in px, wound counter-clockwise seen
/// from outside.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Mesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

/// A finite size of at least `MIN_SIZE`; anything else becomes `MIN_SIZE`.
fn sanitised(size: f32) -> f32 {
    if size.is_finite() {
        size.max(MIN_SIZE)
    } else {
        MIN_SIZE
    }
}

/// `v` scaled to unit length, or `fallback` when it has none.
fn unit(v: [f32; 2], fallback: [f32; 2]) -> [f32; 2] {
    let len = v[0].hypot(v[1]);
    if len.is_finite() && len > 0.0 {
        [v[0] / len, v[1] / len]
    } else {
        fallback
    }
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

/// An elliptic tube around the bristle roots, open at both ends, in
/// ferrule-local space (x across, y along, z up the axis), all in px.
///
/// Its half-axes are the roots' half extents plus a wall margin; it spans
/// z = 0 to 1.5 ferrule radii. Non-finite or non-positive sizes are raised to
/// a tiny minimum.
pub(crate) fn ferrule_mesh(root_half_extent_px: [f32; 2], ferrule_radius_px: f32) -> Mesh {
    let r = sanitised(ferrule_radius_px);
    let a = sanitised(root_half_extent_px[0]) + FERRULE_MARGIN * r;
    let b = sanitised(root_half_extent_px[1]) + FERRULE_MARGIN * r;
    let mut mesh = Mesh::default();
    for z in [0.0, FERRULE_LENGTH * r] {
        for (sin, cos) in ring_angles() {
            let [nx, ny] = unit([cos / a, sin / b], [cos, sin]);
            mesh.vertices.push(MeshVertex {
                position: [a * cos, b * sin, z],
                normal: [nx, ny, 0.0],
            });
        }
    }
    stitch(&mut mesh.indices, 0, SEGMENTS + 1);
    mesh
}

/// A lathe stub above the ferrule, swelling then tapering, with a top cap, in
/// ferrule-local space (z up the brush axis), all in px.
///
/// It spans z = 1.5 to 5.5 ferrule radii. Non-finite or non-positive radii
/// are raised to a tiny minimum.
pub(crate) fn handle_mesh(ferrule_radius_px: f32) -> Mesh {
    let r = sanitised(ferrule_radius_px);
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
        let [nr, nz] = unit([after.0 - before.0, before.1 - after.1], [1.0, 0.0]);
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

#[cfg(test)]
mod tests;
