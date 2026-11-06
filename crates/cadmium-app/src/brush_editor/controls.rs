//! The head and hair controls (D6): each slider's label, range and step, and the value it moves.

use cadmium_engine::brush::{BrushSpec, HairSpec, HeadSpec};

use crate::sliders::format_stepped;

/// What a dropdown calls a head or hair that follows no preset.
pub(super) const CUSTOM: &str = "Custom";

/// Which half of the brush a control shapes; each half has its own preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Part {
    Head,
    Hair,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Control {
    Thickness,
    Length,
    Dome,
    Slant,
    Point,
    Belly,
    Fan,
    Density,
    Stiffness,
    StaticFriction,
    KineticFriction,
    Clumping,
    WetPoint,
    LoadCapacity,
    Damping,
    SideDrag,
    BendMemory,
    PointRelease,
    PointFilm,
    Wicking,
}

/// A slider's label, range and step (the steps are the approved mockup's).
struct Spec {
    control: Control,
    label: &'static str,
    range: [f32; 2],
    step: f32,
}

const fn spec(control: Control, label: &'static str, range: [f32; 2], step: f32) -> Spec {
    Spec {
        control,
        label,
        range,
        step,
    }
}

static HEAD: [Spec; 8] = [
    spec(
        Control::Thickness,
        "Thickness",
        HeadSpec::THICKNESS_RANGE,
        0.01,
    ),
    spec(Control::Length, "Length", HeadSpec::LENGTH_RANGE, 0.05),
    spec(Control::Dome, "Dome", HeadSpec::DOME_RANGE, 0.01),
    spec(Control::Slant, "Slant", HeadSpec::SLANT_RANGE, 0.01),
    spec(Control::Point, "Point", HeadSpec::POINT_RANGE, 0.01),
    spec(Control::Belly, "Belly", HeadSpec::BELLY_RANGE, 0.01),
    spec(Control::Fan, "Fan", HeadSpec::FAN_RANGE_DEG, 1.0),
    spec(
        Control::Density,
        "Density",
        HeadSpec::DENSITY_RANGE_PER_PX,
        0.1,
    ),
];

static HAIR: [Spec; 6] = [
    spec(
        Control::Stiffness,
        "Stiffness",
        HairSpec::STIFFNESS_RANGE,
        0.01,
    ),
    spec(
        Control::StaticFriction,
        "Static Friction",
        HairSpec::FRICTION_RANGE,
        0.01,
    ),
    spec(
        Control::KineticFriction,
        "Kinetic Friction",
        HairSpec::FRICTION_RANGE,
        0.01,
    ),
    spec(
        Control::Clumping,
        "Clumping",
        HairSpec::CLUMPING_RANGE,
        0.01,
    ),
    spec(
        Control::WetPoint,
        "Wet Point",
        HairSpec::WET_POINT_RANGE,
        0.01,
    ),
    spec(
        Control::LoadCapacity,
        "Load Capacity",
        HairSpec::LOAD_LIMIT_RANGE,
        0.05,
    ),
];

static HAIR_ADVANCED: [Spec; 6] = [
    spec(Control::Damping, "Damping", HairSpec::DAMPING_RANGE, 0.001),
    spec(
        Control::SideDrag,
        "Side Drag",
        HairSpec::DRAG_ANISOTROPY_RANGE,
        0.01,
    ),
    spec(
        Control::BendMemory,
        "Bend Memory",
        HairSpec::BEND_MEMORY_RANGE,
        0.1,
    ),
    spec(
        Control::PointRelease,
        "Point Release",
        HairSpec::POINT_RELEASE_RANGE,
        0.01,
    ),
    spec(
        Control::PointFilm,
        "Point Film",
        HairSpec::POINT_FILM_RANGE,
        0.01,
    ),
    spec(Control::Wicking, "Wicking", HairSpec::WICKING_RANGE, 0.001),
];

impl Control {
    /// The Head group's sliders, top to bottom.
    pub(super) fn head() -> impl Iterator<Item = Control> {
        HEAD.iter().map(|spec| spec.control)
    }

    /// The Hair group's main sliders, top to bottom.
    pub(super) fn hair() -> impl Iterator<Item = Control> {
        HAIR.iter().map(|spec| spec.control)
    }

    /// The Hair group's sliders under "Advanced", top to bottom.
    pub(super) fn hair_advanced() -> impl Iterator<Item = Control> {
        HAIR_ADVANCED.iter().map(|spec| spec.control)
    }

    pub(super) fn label(self) -> &'static str {
        self.spec().label
    }

    pub(super) fn range(self) -> [f32; 2] {
        self.spec().range
    }

    pub(super) fn step(self) -> f32 {
        self.spec().step
    }

    pub(super) fn part(self) -> Part {
        if HEAD.iter().any(|spec| spec.control == self) {
            Part::Head
        } else {
            Part::Hair
        }
    }

    pub(super) fn get(self, brush: &BrushSpec) -> f32 {
        let (mut head, mut hair) = (brush.head, brush.hair);
        *self.slot(&mut head, &mut hair)
    }

    /// Sets the control on `brush`, kept within its range.
    pub(super) fn set(self, brush: &mut BrushSpec, value: f32) {
        let [low, high] = self.range();
        *self.slot(&mut brush.head, &mut brush.hair) = value.clamp(low, high);
    }

    /// `value` as the slider's label shows it: the step's decimals, the fan in degrees.
    pub(super) fn format(self, value: f32) -> String {
        let text = format_stepped(value, f64::from(self.step()));
        match self {
            Control::Fan => format!("{text}°"),
            _ => text,
        }
    }

    fn spec(self) -> &'static Spec {
        HEAD.iter()
            .chain(&HAIR)
            .chain(&HAIR_ADVANCED)
            .find(|spec| spec.control == self)
            .expect("every control is listed")
    }

    fn slot<'a>(self, head: &'a mut HeadSpec, hair: &'a mut HairSpec) -> &'a mut f32 {
        match self {
            Control::Thickness => &mut head.thickness,
            Control::Length => &mut head.length,
            Control::Dome => &mut head.dome,
            Control::Slant => &mut head.slant,
            Control::Point => &mut head.point,
            Control::Belly => &mut head.belly,
            Control::Fan => &mut head.fan_deg,
            Control::Density => &mut head.density_per_px,
            Control::Stiffness => &mut hair.stiffness,
            Control::StaticFriction => &mut hair.static_friction,
            Control::KineticFriction => &mut hair.kinetic_friction,
            Control::Clumping => &mut hair.clumping,
            Control::WetPoint => &mut hair.wet_point,
            Control::LoadCapacity => &mut hair.load_limit,
            Control::Damping => &mut hair.damping,
            Control::SideDrag => &mut hair.drag_anisotropy,
            Control::BendMemory => &mut hair.bend_memory,
            Control::PointRelease => &mut hair.point_release,
            Control::PointFilm => &mut hair.point_film,
            Control::Wicking => &mut hair.wicking,
        }
    }
}

/// Where `preset` sits in a dropdown listing `all`, then "Custom" (where `None` sits).
pub(super) fn position_of<T: Copy + PartialEq>(all: &[T], preset: Option<T>) -> u32 {
    let position = preset.and_then(|preset| all.iter().position(|&each| each == preset));
    position.unwrap_or(all.len()) as u32
}

/// The preset at `position` in a dropdown listing `all`, then "Custom" (`None`).
pub(super) fn preset_at<T: Copy>(all: &[T], position: u32) -> Option<T> {
    all.get(position as usize).copied()
}

#[cfg(test)]
mod tests;
