use cadmium_engine::stroke::{StrokeSample, Tool};
use cadmium_engine::surface::Surface;
use cadmium_engine::tool::Holder;

use super::Contact;
use crate::state::AppState;
use crate::state::mixing_knife::strokes_on;
use crate::state::surface_tools::tip_for;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StrokePreparation {
    None,
    TopUp,
    CleanAndLoad,
}

fn stroke_preparation(
    tool: Tool,
    reload_each_stroke: bool,
    clean_each_stroke: bool,
) -> StrokePreparation {
    // The mixing knife's face keeps its mix from one pass to the next.
    let reloads = matches!(
        tool.profile().holder,
        Some(Holder::Reservoir | Holder::Blade)
    );
    if reloads && clean_each_stroke {
        StrokePreparation::CleanAndLoad
    } else if reloads && reload_each_stroke {
        StrokePreparation::TopUp
    } else {
        StrokePreparation::None
    }
}

impl AppState {
    pub fn begin_stroke(&mut self, tool: Tool, sample: StrokeSample) {
        self.begin_stroke_on(Surface::Painting, tool, sample);
    }

    /// Starts a stroke, unless `tool` doesn't work on `surface`; whether it started.
    pub(super) fn begin_stroke_on(
        &mut self,
        surface: Surface,
        tool: Tool,
        sample: StrokeSample,
    ) -> bool {
        if !strokes_on(tool, surface) {
            return false;
        }
        let preparation = stroke_preparation(tool, self.reload_each_stroke, self.clean_each_stroke);
        self.begin_prepared_stroke(surface, tool, sample, preparation);
        true
    }

    pub(super) fn begin_prepared_stroke(
        &mut self,
        surface: Surface,
        tool: Tool,
        sample: StrokeSample,
        preparation: StrokePreparation,
    ) {
        self.release();
        self.pacing.brush_measure_waits = false;
        self.session.brush.tool = tool;
        match preparation {
            StrokePreparation::None => {}
            StrokePreparation::TopUp => self.top_up(),
            StrokePreparation::CleanAndLoad => self.clean_and_load(),
        }
        self.take_up_mixing_knife_size(tool);
        self.provisional.colour = (surface == Surface::Painting && tool == Tool::Brush)
            .then(|| self.held_colour(tool))
            .flatten();
        let tip = tip_for(surface, tool);
        self.session.begin_stroke_with(surface, tip, sample);
        self.contact = Some(Contact::Stroke { surface });
        self.last_sample_time_ms = None;
        self.note_samples(std::slice::from_ref(&sample));
        self.wake();
    }

    pub fn add_samples(&mut self, samples: &[StrokeSample]) {
        if !matches!(self.contact, Some(Contact::Stroke { .. })) || samples.is_empty() {
            return;
        }
        self.session.add_samples(samples);
        self.note_samples(samples);
        self.wake();
    }

    pub(super) fn finish_stroke(&mut self) {
        self.session.end_stroke();
        self.hand_back_brush_size();
        // The stroke took its own copy of the brush; the hand goes back to the painting tool.
        self.session.brush.tool = self.resting_tool;
        self.edits.record();
        self.measure_brush_once_painted();
    }
}

#[cfg(test)]
mod tests;
