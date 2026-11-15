use cadmium_engine::gpu::GpuContext;
use cadmium_engine::pigments::mixture_srgb8;
use cadmium_engine::stroke::{StrokeSample, Tool};

use crate::state::{AppState, Easel, EaselId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StrokePreparation {
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

impl Easel {
    pub fn begin_stroke(&mut self, gpu: &GpuContext, tool: Tool, sample: StrokeSample) {
        let preparation = stroke_preparation(tool, self.reload_each_stroke, self.clean_each_stroke);
        self.begin_prepared_stroke(gpu, tool, sample, preparation);
    }

    fn begin_prepared_stroke(
        &mut self,
        gpu: &GpuContext,
        tool: Tool,
        sample: StrokeSample,
        preparation: StrokePreparation,
    ) {
        self.release(gpu);
        self.brush_measure_waits = false;
        self.session.brush.tool = tool;
        match preparation {
            StrokePreparation::None => {}
            StrokePreparation::TopUp => self.top_up(gpu),
            StrokePreparation::CleanAndLoad => self.clean_and_load(gpu),
        }
        self.session.begin_stroke(sample);
        self.stroking = true;
    }

    /// The colour of the paint the reserve was last read to hold; `None` while it holds none.
    pub(super) fn held_colour(&self) -> Option<[u8; 3]> {
        let held = &self.reserve_mixture;
        (!held.is_empty()).then(|| {
            mixture_srgb8(
                &self.session.db,
                &self.session.canvas.spec.pigment_slots,
                &held.fractions,
            )
        })
    }

    pub(super) fn finish_stroke(&mut self, gpu: &GpuContext) {
        self.session.end_stroke();
        // The stroke took its own copy of the brush; the hand goes back to the painting tool.
        self.session.brush.tool = self.resting_tool;
        self.measure_brush_once_painted(gpu);
    }
}

impl AppState {
    pub fn begin_stroke(&mut self, id: EaselId, tool: Tool, sample: StrokeSample) {
        self.release(id);
        let gpu = self.gpu.clone();
        self.easel_mut(id).begin_stroke(&gpu, tool, sample);
        if id == EaselId::Painting {
            self.provisional.colour = (tool == Tool::Brush)
                .then(|| self.painting.held_colour())
                .flatten();
        }
        self.last_sample_time_ms = None;
        self.note_samples(std::slice::from_ref(&sample));
        self.wake();
    }
}

#[cfg(test)]
mod tests;
