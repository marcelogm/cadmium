//! How a brush's hairs bend, slide, clump and hold paint.

use super::within;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HairSpec {
    pub stiffness: f32,
    pub damping: f32,
    pub static_friction: f32,
    pub kinetic_friction: f32,
    pub drag_anisotropy: f32,
    /// Bend memory, per second.
    pub bend_memory: f32,
    pub clumping: f32,
    pub wet_point: f32,
    pub point_release: f32,
    pub point_film: f32,
    pub load_limit: f32,
    pub wicking: f32,
}

impl HairSpec {
    pub const STIFFNESS_RANGE: [f32; 2] = [0.0, 1.0];
    pub const DAMPING_RANGE: [f32; 2] = [0.0, 0.1];
    pub const FRICTION_RANGE: [f32; 2] = [0.0, 2.0];
    pub const DRAG_ANISOTROPY_RANGE: [f32; 2] = [0.0, 1.0];
    pub const BEND_MEMORY_RANGE: [f32; 2] = [0.0, 20.0];
    pub const CLUMPING_RANGE: [f32; 2] = [0.0, 1.0];
    pub const WET_POINT_RANGE: [f32; 2] = [0.0, 1.0];
    pub const POINT_RELEASE_RANGE: [f32; 2] = [0.01, 1.0];
    pub const POINT_FILM_RANGE: [f32; 2] = [0.01, 1.0];
    pub const LOAD_LIMIT_RANGE: [f32; 2] = [1.0, 5.0];
    pub const WICKING_RANGE: [f32; 2] = [0.0, 0.25];

    pub const SYNTHETIC: HairSpec = HairSpec {
        stiffness: 0.5,
        damping: 0.05,
        static_friction: 1.5,
        kinetic_friction: 1.2,
        drag_anisotropy: 0.3,
        bend_memory: 2.0,
        clumping: 0.5,
        wet_point: 0.9,
        point_release: 0.15,
        point_film: 0.25,
        load_limit: 3.0,
        wicking: 0.01,
    };
    pub const HOG: HairSpec = HairSpec {
        stiffness: 0.85,
        damping: 0.04,
        static_friction: 1.8,
        kinetic_friction: 1.5,
        drag_anisotropy: 0.45,
        bend_memory: 1.0,
        clumping: 0.2,
        wet_point: 0.1,
        point_release: 0.3,
        point_film: 0.25,
        load_limit: 3.5,
        wicking: 0.005,
    };
    pub const SABLE: HairSpec = HairSpec {
        stiffness: 0.4,
        damping: 0.06,
        static_friction: 1.2,
        kinetic_friction: 0.9,
        drag_anisotropy: 0.25,
        bend_memory: 3.0,
        clumping: 0.8,
        wet_point: 1.0,
        point_release: 0.1,
        point_film: 0.2,
        load_limit: 4.0,
        wicking: 0.02,
    };
    pub const MONGOOSE: HairSpec = HairSpec {
        stiffness: 0.7,
        damping: 0.05,
        static_friction: 1.6,
        kinetic_friction: 1.3,
        drag_anisotropy: 0.35,
        bend_memory: 1.5,
        clumping: 0.4,
        wet_point: 0.5,
        point_release: 0.2,
        point_film: 0.25,
        load_limit: 3.0,
        wicking: 0.01,
    };
    pub const SQUIRREL: HairSpec = HairSpec {
        stiffness: 0.15,
        damping: 0.08,
        static_friction: 1.0,
        kinetic_friction: 0.7,
        drag_anisotropy: 0.2,
        bend_memory: 4.0,
        clumping: 0.9,
        wet_point: 0.9,
        point_release: 0.08,
        point_film: 0.2,
        load_limit: 5.0,
        wicking: 0.03,
    };

    pub fn clamped(self) -> HairSpec {
        let f = HairSpec::SYNTHETIC;
        HairSpec {
            stiffness: within(self.stiffness, Self::STIFFNESS_RANGE, f.stiffness),
            damping: within(self.damping, Self::DAMPING_RANGE, f.damping),
            static_friction: within(
                self.static_friction,
                Self::FRICTION_RANGE,
                f.static_friction,
            ),
            kinetic_friction: within(
                self.kinetic_friction,
                Self::FRICTION_RANGE,
                f.kinetic_friction,
            ),
            drag_anisotropy: within(
                self.drag_anisotropy,
                Self::DRAG_ANISOTROPY_RANGE,
                f.drag_anisotropy,
            ),
            bend_memory: within(self.bend_memory, Self::BEND_MEMORY_RANGE, f.bend_memory),
            clumping: within(self.clumping, Self::CLUMPING_RANGE, f.clumping),
            wet_point: within(self.wet_point, Self::WET_POINT_RANGE, f.wet_point),
            point_release: within(
                self.point_release,
                Self::POINT_RELEASE_RANGE,
                f.point_release,
            ),
            point_film: within(self.point_film, Self::POINT_FILM_RANGE, f.point_film),
            load_limit: within(self.load_limit, Self::LOAD_LIMIT_RANGE, f.load_limit),
            wicking: within(self.wicking, Self::WICKING_RANGE, f.wicking),
        }
    }
}

impl Default for HairSpec {
    fn default() -> Self {
        HairSpec::SYNTHETIC
    }
}

#[cfg(test)]
mod tests;
