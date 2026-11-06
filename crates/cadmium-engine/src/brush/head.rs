//! The shape of a brush's hair bundle.

use super::within;

const ROUND_POINT_ONSET: f32 = 0.5;
const FAN_POINT_FADE_DEG: f32 = 10.0;
// used by the bristle layout in a later commit
#[allow(dead_code)]
const BELLY_SPLAY_SHARE: f32 = 0.35;
pub(crate) const MIN_LENGTH_SHARE: f32 = 0.2;

/// Head controls. Length is in ferrule radii, density in hairs per pixel of width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadSpec {
    pub thickness: f32,
    pub length: f32,
    pub dome: f32,
    pub slant: f32,
    pub point: f32,
    pub belly: f32,
    pub fan_deg: f32,
    pub density_per_px: f32,
}

/// How the roots are laid out across the ferrule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Section {
    Round,
    Bar,
    Arc,
}

const fn head(thickness: f32, length: f32, dome: f32, slant: f32) -> HeadSpec {
    HeadSpec {
        thickness,
        length,
        dome,
        slant,
        point: 1.0,
        belly: 0.0,
        fan_deg: 0.0,
        density_per_px: 1.6,
    }
}

impl HeadSpec {
    pub const THICKNESS_RANGE: [f32; 2] = [0.08, 1.0];
    pub const LENGTH_RANGE: [f32; 2] = [0.5, 6.0];
    pub const DOME_RANGE: [f32; 2] = [0.0, 1.0];
    pub const SLANT_RANGE: [f32; 2] = [-1.0, 1.0];
    pub const POINT_RANGE: [f32; 2] = [0.0, 1.0];
    pub const BELLY_RANGE: [f32; 2] = [0.0, 1.0];
    pub const FAN_RANGE_DEG: [f32; 2] = [0.0, 90.0];
    pub const DENSITY_RANGE_PER_PX: [f32; 2] = [0.5, 8.0];

    pub const ROUND: HeadSpec = head(1.0, 2.5, 0.0, 0.0);
    pub const FLAT: HeadSpec = head(0.25, 2.0, 0.0, 0.0);
    pub const FAN: HeadSpec = HeadSpec {
        fan_deg: 50.0,
        ..head(0.12, 2.0, 0.0, 0.0)
    };
    pub const BRIGHT: HeadSpec = HeadSpec {
        density_per_px: 1.8,
        ..head(0.3, 1.1, 0.0, 0.0)
    };
    pub const FILBERT: HeadSpec = HeadSpec {
        belly: 0.2,
        ..head(0.3, 2.0, 0.8, 0.0)
    };
    pub const RIGGER: HeadSpec = HeadSpec {
        density_per_px: 2.5,
        ..head(1.0, 6.0, 0.3, 0.0)
    };
    pub const ANGULAR: HeadSpec = head(0.25, 2.0, 0.0, 0.8);
    pub const DAGGER: HeadSpec = HeadSpec {
        belly: 0.2,
        ..head(0.3, 2.5, 0.5, 0.9)
    };
    pub const MOP: HeadSpec = HeadSpec {
        point: 0.6,
        belly: 0.8,
        density_per_px: 3.0,
        ..head(1.0, 1.8, 1.0, 0.0)
    };

    pub fn clamped(self) -> HeadSpec {
        let fallback = HeadSpec::ROUND;
        HeadSpec {
            thickness: within(self.thickness, Self::THICKNESS_RANGE, fallback.thickness),
            length: within(self.length, Self::LENGTH_RANGE, fallback.length),
            dome: within(self.dome, Self::DOME_RANGE, fallback.dome),
            slant: within(self.slant, Self::SLANT_RANGE, fallback.slant),
            point: within(self.point, Self::POINT_RANGE, fallback.point),
            belly: within(self.belly, Self::BELLY_RANGE, fallback.belly),
            fan_deg: within(self.fan_deg, Self::FAN_RANGE_DEG, fallback.fan_deg),
            density_per_px: within(
                self.density_per_px,
                Self::DENSITY_RANGE_PER_PX,
                fallback.density_per_px,
            ),
        }
    }

    pub fn is_round(&self) -> bool {
        self.thickness >= Self::THICKNESS_RANGE[1]
    }

    pub(crate) fn section(&self) -> Section {
        if self.is_round() {
            Section::Round
        } else if self.fan_deg > 0.0 {
            Section::Arc
        } else {
            Section::Bar
        }
    }

    /// How strongly a wet head draws to a point across its width and its thickness.
    pub(crate) fn point_axes(&self) -> [f32; 2] {
        let round_share = ((self.thickness - ROUND_POINT_ONSET)
            / (Self::THICKNESS_RANGE[1] - ROUND_POINT_ONSET))
            .clamp(0.0, 1.0);
        let open = 1.0 - (self.fan_deg / FAN_POINT_FADE_DEG).clamp(0.0, 1.0);
        [self.point * round_share * open, self.point * open]
    }

    /// Share of the full hair length for a root at `[across, thick]` in ferrule radii.
    pub(crate) fn length_share(&self, root: [f32; 2]) -> f32 {
        if self.dome == 0.0 && self.slant == 0.0 {
            return 1.0;
        }
        let reach = root[0].hypot(root[1]).min(1.0);
        let domed = (1.0 - self.dome) + self.dome * (1.0 - reach * reach).sqrt();
        let side = if self.slant < 0.0 { -1.0 } else { 1.0 };
        let across = (side * root[0]).clamp(-1.0, 1.0);
        let slanted = 1.0 - self.slant.abs() * 0.5 * (1.0 - across);
        (domed * slanted).max(MIN_LENGTH_SHARE)
    }

    // used by the bristle layout in a later commit
    #[allow(dead_code)]
    /// Outward splay of the rest pose, per ferrule radius from the axis.
    pub(crate) fn bulge(&self) -> f32 {
        let fan = if self.section() == Section::Round {
            self.fan_deg.to_radians().sin()
        } else {
            0.0
        };
        fan + self.belly * BELLY_SPLAY_SHARE
    }
}

#[cfg(test)]
mod tests;
