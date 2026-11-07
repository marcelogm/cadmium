use cadmium_engine::present::ViewTransform;
use cadmium_engine::stroke::Dab;

use crate::view::canvas_to_viewport;

pub const PROVISIONAL_OPACITY: f32 = 0.6;
/// How long a lag lasts before the trail starts to show.
pub const TRAIL_DELAY_MS: f64 = 60.0;
/// How long the trail then takes to reach full strength.
pub const TRAIL_FADE_MS: f64 = 150.0;
/// How many times faster the built-up lag drains than it builds.
const FALL_SPEEDUP: f64 = 4.0;
/// The most of a gap between frames that counts toward what the newer frame shows; the
/// rest of the gap is spent in what the older frame showed.
const LONGEST_FADE_STEP_MS: f64 = 1000.0 / 60.0;
const THINNING_SHARE_OF_RADIUS: f32 = 0.5;

/// A disc of provisional paint, in canvas px for a trail, in widget px once mapped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub centre: (f32, f32),
    pub radius: f32,
}

/// The dabs a frame left queued, as the paint they promise: the brush's colour over the
/// stretch of stroke the presented texture does not show yet, kept with the view that
/// texture was drawn with.
#[derive(Clone, Debug, PartialEq)]
pub struct ProvisionalTrail {
    pub colour: [u8; 3],
    /// How strongly the trail shows, from nothing up to PROVISIONAL_OPACITY.
    pub opacity: f32,
    marks: Vec<Mark>,
    view: ViewTransform,
}

#[derive(Default)]
pub(crate) struct Provisional {
    pub(crate) colour: Option<[u8; 3]>,
    pub(crate) presented: Option<ProvisionalTrail>,
    pub(crate) fade: TrailFade,
}

/// How strongly the trail shows. It builds up the lag while frames have a trail, hidden
/// through a short lag and fading in once the lag lasts. The lag drains faster than it
/// builds, so a lag that comes and goes stays hidden.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TrailFade {
    built_up_ms: f64,
    last_ms: Option<f64>,
    showing: bool,
}

impl TrailFade {
    /// Moves the lag on to `now_ms`, building it while `showing` and draining it while not,
    /// and gives the trail's opacity: nothing through the first `TRAIL_DELAY_MS` of lag,
    /// then rising to full strength over `TRAIL_FADE_MS`. A long gap since the last frame
    /// was spent in that frame's state, so only one frame's worth of it counts toward the
    /// new state.
    pub(crate) fn advance(&mut self, now_ms: f64, showing: bool) -> f32 {
        let elapsed_ms = self.last_ms.map_or(0.0, |last| (now_ms - last).max(0.0));
        self.last_ms = Some(now_ms);
        let toward_new_ms = elapsed_ms.min(LONGEST_FADE_STEP_MS);
        self.built_up_ms = lag_after(self.built_up_ms, self.showing, elapsed_ms - toward_new_ms);
        self.built_up_ms = lag_after(self.built_up_ms, showing, toward_new_ms);
        self.showing = showing;
        ((self.built_up_ms - TRAIL_DELAY_MS) / TRAIL_FADE_MS).clamp(0.0, 1.0) as f32
            * PROVISIONAL_OPACITY
    }
}

fn lag_after(built_up_ms: f64, showing: bool, elapsed_ms: f64) -> f64 {
    if showing {
        (built_up_ms + elapsed_ms).min(TRAIL_DELAY_MS + TRAIL_FADE_MS)
    } else {
        (built_up_ms - elapsed_ms * FALL_SPEEDUP).max(0.0)
    }
}

impl ProvisionalTrail {
    pub fn of(colour: [u8; 3], pending: &[Dab], view: ViewTransform) -> ProvisionalTrail {
        ProvisionalTrail {
            colour,
            opacity: PROVISIONAL_OPACITY,
            marks: thinned(pending),
            view,
        }
    }

    /// The same trail shown at `opacity`.
    pub fn faded(self, opacity: f32) -> ProvisionalTrail {
        ProvisionalTrail { opacity, ..self }
    }

    pub fn canvas_marks(&self) -> &[Mark] {
        &self.marks
    }

    /// The marks in widget px, for a widget whose device pixels are `device_scale` of its own.
    pub fn widget_marks(&self, device_scale: f32) -> Vec<Mark> {
        let zoom = self.view.zoom / device_scale;
        self.marks
            .iter()
            .map(|mark| {
                let (x, y) = canvas_to_viewport(
                    self.view,
                    f64::from(mark.centre.0),
                    f64::from(mark.centre.1),
                );
                Mark {
                    centre: (x as f32 / device_scale, y as f32 / device_scale),
                    radius: mark.radius * zoom,
                }
            })
            .collect()
    }
}

fn mark_of(dab: &Dab) -> Mark {
    Mark {
        centre: (dab.x, dab.y),
        radius: dab.radius,
    }
}

/// The cell of a canvas grid about half the dab's radius wide that holds the dab, with the
/// grid's size, which doubles in steps so dabs of nearly the same size share a grid.
fn cell_of(dab: &Dab) -> (i32, i32, i32) {
    let scale = (THINNING_SHARE_OF_RADIUS * dab.radius)
        .max(1.0)
        .log2()
        .floor() as i32;
    let size = 2.0_f32.powi(scale);
    let cell = |coordinate: f32| (coordinate / size).floor() as i32;
    (cell(dab.x), cell(dab.y), scale)
}

/// The dabs as marks, keeping a dab only where it steps into another grid cell than the dab
/// before it. That depends on the dab and its neighbour alone, never on where the queue
/// starts, so painting the head of the queue leaves the marks behind it in place. The head
/// and the stroke's tip always stay.
fn thinned(dabs: &[Dab]) -> Vec<Mark> {
    let last = dabs.len().saturating_sub(1);
    dabs.iter()
        .enumerate()
        .filter(|&(index, dab)| {
            index == 0 || index == last || cell_of(&dabs[index - 1]) != cell_of(dab)
        })
        .map(|(_, dab)| mark_of(dab))
        .collect()
}

#[cfg(test)]
mod tests;
