//! The example strokes under the schematic (D3): an S-curve and a pressure ramp, painted by
//! the engine with the draft brush and the painting's paint (C4).

use std::cell::RefCell;

use cadmium_studio::StudioError;

mod job;
mod queue;
mod view;

pub(super) use job::{ExampleJob, Tick};
pub(super) use queue::{ExampleQueue, Order};
pub(super) use view::ExamplesView;

const FAILURE_TOAST: &str = "Can't show the example strokes";

/// The editor's examples: the picture, the job that paints it and which painting counts.
pub(super) struct Examples {
    pub(super) view: ExamplesView,
    pub(super) job: RefCell<ExampleJob>,
    pub(super) queue: RefCell<ExampleQueue>,
}

impl Examples {
    pub(super) fn new(job: ExampleJob) -> Examples {
        Examples {
            view: ExamplesView::new(),
            job: RefCell::new(job),
            queue: RefCell::new(ExampleQueue::default()),
        }
    }
}

/// The toast for examples that failed to paint; `None` for a lost GPU, which the window's
/// GPU-lost dialog reports.
pub(super) fn failure_toast(error: &StudioError, gpu_lost: bool) -> Option<String> {
    match error {
        StudioError::Gpu(_) => None,
        _ if gpu_lost => None,
        error => Some(format!("{FAILURE_TOAST}: {error}")),
    }
}

#[cfg(test)]
mod tests;
