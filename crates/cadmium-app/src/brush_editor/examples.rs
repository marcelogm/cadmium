//! The example strokes under the schematic: an S-curve and a pressure ramp, painted by
//! the engine with the draft brush and the painting's paint.

use std::cell::RefCell;

use gtk::glib;

mod job;
mod queue;
mod view;

pub(super) use job::{ExampleJob, Tick};
use queue::ExampleQueue;
pub(super) use queue::Order;
pub(super) use view::ExamplesView;

/// What the toast says when the example strokes can't be painted.
pub(super) const EXAMPLES_FAILED: &str = "Can't show the example strokes";

/// The editor's examples: the picture, the job that paints it and which painting counts.
pub(super) struct Examples {
    pub(super) view: ExamplesView,
    pub(super) job: RefCell<ExampleJob>,
    pub(super) queue: RefCell<ExampleQueue>,
    /// The debounce running for a settling change; its callback clears it as it fires.
    pub(super) debounce: RefCell<Option<glib::SourceId>>,
}

impl Examples {
    pub(super) fn new(job: ExampleJob) -> Examples {
        Examples {
            view: ExamplesView::new(),
            job: RefCell::new(job),
            queue: RefCell::new(ExampleQueue::default()),
            debounce: RefCell::new(None),
        }
    }
}

#[cfg(test)]
mod tests;
