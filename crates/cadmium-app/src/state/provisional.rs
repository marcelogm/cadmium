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
