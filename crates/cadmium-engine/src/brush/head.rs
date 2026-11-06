//! The shape of a brush's hair bundle.

use serde::{Deserialize, Serialize};

use super::within;

const ROUND_POINT_ONSET: f32 = 0.5;
const FAN_POINT_FADE_DEG: f32 = 10.0;
const BELLY_SPLAY_SHARE: f32 = 0.35;
pub(crate) const MIN_LENGTH_SHARE: f32 = 0.2;
/// Thickness from which a bar's roots start drawing into the round spiral (round at 1).
const ROUND_BLEND_ONSET: f32 = 0.8;
/// Fan angle by which a bar's roots have bent fully into the fan's arc (a bar at 0°).
const ARC_BLEND_DEG: f32 = 10.0;

/// Head controls. Length is in ferrule radii, density in hairs per pixel of width.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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

/// How much each section lays a head's roots: shares summing to 1. A factory head is one
/// section alone; a head near full thickness or with a small fan blends two or three, so
/// its layout slides from one section into the next instead of jumping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SectionMix {
    pub(crate) round: f32,
    pub(crate) bar: f32,
    pub(crate) arc: f32,
}

impl SectionMix {
    /// The sections with a share, in a fixed order.
    pub(crate) fn parts(self) -> impl Iterator<Item = (Section, f32)> {
        [
            (Section::Round, self.round),
            (Section::Bar, self.bar),
            (Section::Arc, self.arc),
        ]
        .into_iter()
        .filter(|&(_, share)| share > 0.0)
    }
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

impl Default for HeadSpec {
    fn default() -> Self {
        HeadSpec::ROUND
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
    pub const ANGULAR: HeadSpec = head(0.25, 2.0, 0.0, 0.4);
    pub const DAGGER: HeadSpec = HeadSpec {
        belly: 0.2,
        ..head(0.3, 2.5, 0.3, 0.5)
    };
    pub const MOP: HeadSpec = HeadSpec {
        point: 0.3,
        belly: 0.3,
        density_per_px: 2.5,
        ..head(1.0, 1.8, 0.4, 0.0)
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

    /// The sections the roots are laid in. Exactly one section, at a share of exactly 1,
    /// outside the blend bands (thickness below 0.8 or at 1; no fan, or 10° and wider).
    /// This smooths the root layout only: `is_round` still turns the root frame at
    /// thickness 1, which does not show because a near-round head's roots are a disc.
    pub(crate) fn section_mix(&self) -> SectionMix {
        let round = eased_share(self.thickness, ROUND_BLEND_ONSET, Self::THICKNESS_RANGE[1]);
        let arc = eased_share(self.fan_deg, Self::FAN_RANGE_DEG[0], ARC_BLEND_DEG);
        SectionMix {
            round,
            bar: (1.0 - round) * (1.0 - arc),
            arc: (1.0 - round) * arc,
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
        let reach = libm::hypotf(root[0], root[1]).min(1.0);
        let domed = (1.0 - self.dome) + self.dome * (1.0 - reach * reach).sqrt();
        let side = if self.slant < 0.0 { -1.0 } else { 1.0 };
        let across = (side * root[0]).clamp(-1.0, 1.0);
        let slanted = 1.0 - self.slant.abs() * 0.5 * (1.0 - across);
        (domed * slanted).max(MIN_LENGTH_SHARE)
    }

    /// Outward splay of the rest pose, per ferrule radius from the axis, for roots laid in
    /// `section`: a fan splays a round head; a bar or an arc only by its belly.
    pub(crate) fn bulge(&self, section: Section) -> f32 {
        let fan = if section == Section::Round {
            libm::sinf(self.fan_deg.to_radians())
        } else {
            0.0
        };
        fan + self.belly * BELLY_SPLAY_SHARE
    }
}

/// 0 up to `from`, 1 from `to` on, eased (smoothstep) in between; NaN counts as 0.
fn eased_share(value: f32, from: f32, to: f32) -> f32 {
    if value.is_nan() || value <= from {
        return 0.0;
    }
    if value >= to {
        return 1.0;
    }
    let t = (value - from) / (to - from);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests;
