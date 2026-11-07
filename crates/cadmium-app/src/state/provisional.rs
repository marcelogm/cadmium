use cadmium_engine::present::ViewTransform;
use cadmium_engine::stroke::Dab;

use crate::view::canvas_to_viewport;

pub const PROVISIONAL_OPACITY: f32 = 0.6;
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
    marks: Vec<Mark>,
    view: ViewTransform,
}

#[derive(Default)]
pub(crate) struct Provisional {
    pub(crate) colour: Option<[u8; 3]>,
    pub(crate) presented: Option<ProvisionalTrail>,
}

impl ProvisionalTrail {
    pub fn of(colour: [u8; 3], pending: &[Dab], view: ViewTransform) -> ProvisionalTrail {
        ProvisionalTrail {
            colour,
            marks: thinned(pending),
            view,
        }
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

fn far_enough(kept: &Mark, next: &Mark) -> bool {
    let gap = (next.centre.0 - kept.centre.0).hypot(next.centre.1 - kept.centre.1);
    gap >= THINNING_SHARE_OF_RADIUS * kept.radius.min(next.radius)
}

/// The dabs as marks, dropping those that sit within half a radius of the mark kept before
/// them; the last dab, the stroke's tip, always stays.
fn thinned(dabs: &[Dab]) -> Vec<Mark> {
    let mut marks: Vec<Mark> = Vec::new();
    for dab in dabs {
        let mark = mark_of(dab);
        match marks.last() {
            Some(kept) if !far_enough(kept, &mark) => {}
            _ => marks.push(mark),
        }
    }
    if let (Some(tip), Some(kept)) = (dabs.last().map(mark_of), marks.last())
        && *kept != tip
    {
        marks.push(tip);
    }
    marks
}

#[cfg(test)]
mod tests;
