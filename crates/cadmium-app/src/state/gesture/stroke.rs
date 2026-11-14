use cadmium_engine::pigments::mixture_srgb8;
use cadmium_engine::stroke::{StrokeSample, Tool};

use crate::state::AppState;

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
    let reloads = tool.profile().holder.is_some();
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
        let preparation = stroke_preparation(tool, self.reload_each_stroke, self.clean_each_stroke);
        self.begin_prepared_stroke(tool, sample, preparation);
    }

    fn begin_prepared_stroke(
        &mut self,
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
        self.provisional.colour = (tool == Tool::Brush)
            .then(|| self.held_colour(tool))
            .flatten();
        self.session.begin_stroke(sample);
        self.stroking = true;
        self.last_sample_time_ms = None;
        self.note_samples(std::slice::from_ref(&sample));
        self.wake();
    }

    pub fn add_samples(&mut self, samples: &[StrokeSample]) {
        if !self.stroking || samples.is_empty() {
            return;
        }
        self.session.add_samples(samples);
        self.note_samples(samples);
        self.wake();
    }

    fn held_colour(&self, tool: Tool) -> Option<[u8; 3]> {
        let held = if tool.profile().holds_on_blade() {
            &self.knife_mixture
        } else {
            &self.reserve_mixture
        };
        (!held.is_empty()).then(|| {
            mixture_srgb8(
                &self.session.db,
                &self.session.canvas.spec.pigment_slots,
                &held.fractions,
            )
        })
    }

    pub(super) fn finish_stroke(&mut self) {
        self.session.end_stroke();
        // The stroke took its own copy of the brush; the hand goes back to the painting tool.
        self.session.brush.tool = self.resting_tool;
        self.edits.record();
        self.measure_brush_once_painted();
    }
}

#[cfg(test)]
mod tests;
