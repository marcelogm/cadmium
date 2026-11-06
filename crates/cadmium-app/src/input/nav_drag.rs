use std::f64::consts::{PI, TAU};

use gtk::gdk;

use crate::settings::{PenButtonAction, Settings};

/// How far, in widget px, a zoom drag goes for the view to zoom by a factor of e.
const ZOOM_DRAG_PX_PER_E: f64 = 100.0;
/// The log-zoom a zoom drag adds per widget px.
pub(super) const ZOOM_PER_DRAG_PX: f64 = 1.0 / ZOOM_DRAG_PX_PER_E;
/// Nearer the centre than this, in widget px, a rotate drag's angle is too jumpy to follow.
const ROTATE_DEAD_RADIUS: f64 = 8.0;

/// What a navigation drag does to the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NavMode {
    Pan,
    Zoom,
    Rotate,
}

impl NavMode {
    /// The pointer shown while Space holds this mode ready.
    pub(crate) fn cursor_name(self) -> &'static str {
        match self {
            NavMode::Pan => "grab",
            NavMode::Zoom => "zoom-in",
            NavMode::Rotate => "move",
        }
    }
}

/// The pointer buttons a navigation drag can start from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NavButton {
    Primary,
    Middle,
    Secondary,
}

impl NavButton {
    pub(super) const ALL: [NavButton; 3] =
        [NavButton::Primary, NavButton::Middle, NavButton::Secondary];

    /// The GDK button number.
    pub(super) fn button(self) -> u32 {
        match self {
            NavButton::Primary => gdk::BUTTON_PRIMARY,
            NavButton::Middle => gdk::BUTTON_MIDDLE,
            NavButton::Secondary => gdk::BUTTON_SECONDARY,
        }
    }

    /// A stable index into per-button tables.
    pub(super) fn index(self) -> usize {
        self as usize
    }
}

/// What is known when a button goes down on the canvas.
#[derive(Clone, Copy, Debug)]
pub(super) struct NavPress {
    pub(super) button: NavButton,
    pub(super) from_pen: bool,
    /// The mode Space holds ready, while it is held.
    pub(super) space: Option<NavMode>,
    /// Whether the current tool pans with the primary contact (the Move tool).
    pub(super) tool_pans: bool,
    pub(super) stroking: bool,
}

/// The mode a press starts, or `None` when it doesn't navigate.
pub(super) fn nav_mode(press: NavPress, settings: Settings) -> Option<NavMode> {
    if press.stroking {
        return None;
    }
    match (press.button, press.from_pen) {
        (NavButton::Primary, _) => press.space.or(press.tool_pans.then_some(NavMode::Pan)),
        (NavButton::Middle, true) => bound_mode(settings.lower_pen_button),
        (NavButton::Secondary, true) => bound_mode(settings.upper_pen_button),
        (NavButton::Middle, false) => Some(NavMode::Pan),
        (NavButton::Secondary, false) => None,
    }
}

fn bound_mode(action: PenButtonAction) -> Option<NavMode> {
    match action {
        PenButtonAction::Pan => Some(NavMode::Pan),
        PenButtonAction::Zoom => Some(NavMode::Zoom),
        PenButtonAction::Rotate => Some(NavMode::Rotate),
        PenButtonAction::PickColour | PenButtonAction::Nothing => None,
    }
}

/// The mode Space holds ready under `modifiers`.
pub(crate) fn space_mode(modifiers: gdk::ModifierType) -> NavMode {
    if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
        NavMode::Zoom
    } else if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
        NavMode::Rotate
    } else {
        NavMode::Pan
    }
}

/// Whether a click of `button` picks a colour.
pub(super) fn picks_colour(button: u32, from_pen: bool, settings: Settings) -> bool {
    let action = match (button, from_pen) {
        (gdk::BUTTON_MIDDLE, true) => settings.lower_pen_button,
        (gdk::BUTTON_SECONDARY, true) => settings.upper_pen_button,
        (gdk::BUTTON_SECONDARY, false) => PenButtonAction::PickColour,
        _ => PenButtonAction::Nothing,
    };
    action == PenButtonAction::PickColour
}

/// One change a navigation drag makes to the view, in widget px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ViewStep {
    Pan { dx: f64, dy: f64 },
    Zoom { anchor: (f64, f64), factor: f32 },
    Rotate { centre: (f64, f64), delta: f32 },
}

/// A navigation drag in progress: where it started and the offset it last stepped to.
#[derive(Clone, Copy, Debug)]
pub(super) struct NavDrag {
    mode: NavMode,
    start: (f64, f64),
    last: (f64, f64),
}

impl NavDrag {
    pub(super) fn new(mode: NavMode, start: (f64, f64)) -> NavDrag {
        NavDrag {
            mode,
            start,
            last: (0.0, 0.0),
        }
    }

    /// The view change for the drag reaching `offset` from its start; `centre` is the
    /// widget's centre, which a rotation turns about.
    pub(super) fn step(&mut self, offset: (f64, f64), centre: (f64, f64)) -> Option<ViewStep> {
        let from = self.last;
        let step = match self.mode {
            NavMode::Pan => Some(ViewStep::Pan {
                dx: offset.0 - from.0,
                dy: offset.1 - from.1,
            }),
            NavMode::Zoom => Some(ViewStep::Zoom {
                anchor: self.start,
                factor: (-(offset.1 - from.1) * ZOOM_PER_DRAG_PX).exp() as f32,
            }),
            NavMode::Rotate => self.rotate_step(from, offset, centre),
        };
        if step.is_some() {
            self.last = offset;
        }
        step
    }

    fn rotate_step(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        centre: (f64, f64),
    ) -> Option<ViewStep> {
        let at = |o: (f64, f64)| (self.start.0 + o.0 - centre.0, self.start.1 + o.1 - centre.1);
        let (a, b) = (at(from), at(to));
        if a.0.hypot(a.1) < ROTATE_DEAD_RADIUS || b.0.hypot(b.1) < ROTATE_DEAD_RADIUS {
            return None;
        }
        let swept = (b.1.atan2(b.0) - a.1.atan2(a.0) + PI).rem_euclid(TAU) - PI;
        Some(ViewStep::Rotate {
            centre,
            delta: swept as f32,
        })
    }
}

#[cfg(test)]
mod tests;
