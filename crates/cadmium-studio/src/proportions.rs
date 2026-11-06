//! Brush proportions shared by the camera's framing and the meshes, in
//! ferrule radii.

/// The ferrule's length above the reach.
pub(crate) const FERRULE_LENGTH: f32 = 1.5;
/// Clearance above the ferrule for the handle.
pub(crate) const HANDLE_CLEARANCE: f32 = 4.0;
/// Ferrule wall margin around the bristle roots.
pub(crate) const FERRULE_MARGIN: f32 = 0.08;
/// Handle profile as (height share of the clearance, radius).
pub(crate) const HANDLE_PROFILE: [(f32, f32); 6] = [
    (0.0, 0.9),
    (0.15, 1.05),
    (0.4, 1.2),
    (0.7, 1.1),
    (0.9, 0.85),
    (1.0, 0.7),
];
