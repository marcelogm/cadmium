use crate::engine::WetOilEngine;
use crate::engine::shader::bristle_view_source;

/// Read-only access to the live bristles, for rendering on the same device.
/// Valid after the brush's first dab (the layout is fitted then).
///
/// The bristle count reaches shaders as an instance count or a uniform the
/// caller supplies from [`BristleView::count`]; the WGSL module has no
/// `bristle_count()` of its own.
pub struct BristleView<'a> {
    states: &'a wgpu::Buffer,
    rests: &'a wgpu::Buffer,
    handle: &'a wgpu::Buffer,
    in_hand: bool,
    count: u32,
    ferrule_radius_px: f32,
    root_half_extent_px: [f32; 2],
}

impl<'a> BristleView<'a> {
    /// Storage buffer of `vec4<f32>`: per bristle, positions, previous
    /// positions, anchors and the rest bend.
    pub fn states(&self) -> &'a wgpu::Buffer {
        self.states
    }

    /// Storage buffer of `BristleRest`, one per bristle.
    pub fn rests(&self) -> &'a wgpu::Buffer {
        self.rests
    }

    /// Storage buffer holding one `HandleState`.
    pub fn handle(&self) -> &'a wgpu::Buffer {
        self.handle
    }

    /// Whether a bristle tool painted the last dab. False before the first
    /// dab and after a dab with a tool that has no bristles (the knife, say),
    /// when the buffers still hold the last brush's hairs.
    pub fn in_hand(&self) -> bool {
        self.in_hand
    }

    /// Bristles in the fitted layout, at most 512: the instance count to draw.
    /// 0 before the first dab. Refitted on a dab, so after a brush change it
    /// stays the old layout's until the next dab.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Ferrule radius of the fitted layout, in px. Like [`BristleView::count`],
    /// it follows a resize only once the next dab refits the layout.
    pub fn ferrule_radius_px(&self) -> f32 {
        self.ferrule_radius_px
    }

    /// Largest root offset from the ferrule centre, across (x) and along (y)
    /// the unspun head.
    pub fn root_half_extent_px(&self) -> [f32; 2] {
        self.root_half_extent_px
    }

    /// WGSL module: engine constants, `BristleRest`, `HandleState`, `Pose`, `Frame`,
    /// `frame_of`/`tilted`/`spun`, `bristle_vertex(b, v) -> vec4` (xyz canvas px, z height,
    /// w floor) and `handle_pose()`. Every shader that includes it must declare
    /// `states: array<vec4<f32>>` and `handle: HandleState` globals, even if unused;
    /// in vertex stages both must be `var<storage, read>` (the handle buffer has no
    /// uniform usage).
    pub fn wgsl(&self) -> String {
        bristle_view_source()
    }
}

impl WetOilEngine {
    pub fn bristle_view(&self) -> BristleView<'_> {
        let buffers = &self.buffers.bristles;
        BristleView {
            states: &buffers.states,
            rests: &buffers.rests,
            handle: &buffers.handle,
            in_hand: self.bristles.in_hand(),
            count: self.bristles.count(),
            ferrule_radius_px: self.bristles.ferrule_radius_px(),
            root_half_extent_px: self.bristles.root_half_extent_px(),
        }
    }
}
