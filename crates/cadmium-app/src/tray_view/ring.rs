use std::time::Instant;

use gtk::gdk;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::TrayView;
use crate::cursor_outline::{CursorOutline, RingStroke, append_ring};
use crate::state::LoadRing;

const RING_START_GAP_PX: f32 = 30.0;
const RING_MIN_WIDTH_PX: f32 = 2.0;
const RING_MAX_WIDTH_PX: f32 = 8.0;
const RING_MIN_ALPHA: f32 = 0.35;
const RING_ALPHA_RANGE: f32 = 0.6;
const RING_FADE_SECONDS: f32 = 0.4;
const CHANNEL_MAX: f32 = 255.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct RingShape {
    pub radius: f32,
    pub width: f32,
    pub alpha: f32,
}

fn ring_shape(outline_radius: f32, ring: &LoadRing) -> RingShape {
    let level = ring.level.clamp(0.0, 1.0);
    let pressure = ring.pressure.clamp(0.0, 1.0);
    RingShape {
        radius: outline_radius + RING_START_GAP_PX * (1.0 - level),
        width: RING_MIN_WIDTH_PX + (RING_MAX_WIDTH_PX - RING_MIN_WIDTH_PX) * pressure,
        alpha: RING_MIN_ALPHA + RING_ALPHA_RANGE * level,
    }
}

fn fading_ring(shape: RingShape, seconds_since_release: f32) -> Option<RingShape> {
    let remaining = 1.0 - seconds_since_release.max(0.0) / RING_FADE_SECONDS;
    (remaining > 0.0).then_some(RingShape {
        alpha: shape.alpha * remaining,
        ..shape
    })
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ShownRing {
    ring: LoadRing,
    centre: (f64, f64),
    released_at: Option<Instant>,
}

fn paint_colour([red, green, blue]: [u8; 3], alpha: f32) -> gdk::RGBA {
    gdk::RGBA::new(
        f32::from(red) / CHANNEL_MAX,
        f32::from(green) / CHANNEL_MAX,
        f32::from(blue) / CHANNEL_MAX,
        alpha,
    )
}

/// The ring to show next: a live ring at the pen (or where it was shown, when the pen has
/// left), and a ring that has ended kept from the moment it ended until it has faded.
fn next_shown_ring(
    ring: Option<LoadRing>,
    shown: Option<ShownRing>,
    hover: Option<(f64, f64)>,
    now: Instant,
) -> Option<ShownRing> {
    match (ring, shown) {
        (Some(ring), _) => hover
            .or(shown.map(|shown| shown.centre))
            .map(|centre| ShownRing {
                ring,
                centre,
                released_at: None,
            }),
        (None, Some(shown)) => match shown.released_at {
            None => Some(ShownRing {
                released_at: Some(now),
                ..shown
            }),
            Some(released) if now.duration_since(released).as_secs_f32() < RING_FADE_SECONDS => {
                Some(shown)
            }
            Some(_) => None,
        },
        (None, None) => None,
    }
}

/// The shape to draw `shown` with at `now`, or none once it has faded.
fn visible_shape(shown: &ShownRing, outline_radius: f32, now: Instant) -> Option<RingShape> {
    let shape = ring_shape(outline_radius, &shown.ring);
    match shown.released_at {
        None => Some(shape),
        Some(released) => fading_ring(shape, now.duration_since(released).as_secs_f32()),
    }
}

pub(super) fn append_load_ring(
    snapshot: &gtk::Snapshot,
    shown: &ShownRing,
    outline: &CursorOutline,
) {
    if let Some(shape) = visible_shape(shown, outline.radius(), Instant::now()) {
        let (x, y) = shown.centre;
        append_ring(
            snapshot,
            &RingStroke {
                centre: (x as f32, y as f32),
                semi_axes: [shape.radius; 2],
                rotation_deg: 0.0,
                colour: paint_colour(shown.ring.colour, shape.alpha),
                width: shape.width,
            },
        );
    }
}

impl TrayView {
    pub(super) fn follow_load_ring(&self) -> bool {
        let imp = self.imp();
        let ring = imp
            .state
            .get()
            .and_then(|state| state.try_borrow().ok().map(|state| state.load_ring()));
        let Some(ring) = ring else {
            return imp.shown_ring.get().is_some();
        };
        let shown = imp.shown_ring.get();
        let next = next_shown_ring(ring, shown, imp.hover.get(), Instant::now());
        imp.shown_ring.set(next);
        if shown.is_some() || next.is_some() {
            self.queue_draw();
        }
        next.is_some()
    }
}

#[cfg(test)]
mod tests;
