use std::cell::{Cell, RefCell};

use crate::state::SharedState;

pub(crate) type Handler<W> = Box<dyn Fn(&W)>;

pub(crate) fn run_handlers<W>(handlers: &RefCell<Vec<Handler<W>>>, widget: &W) {
    for handler in handlers.borrow().iter() {
        handler(widget);
    }
}

/// Runs `handlers` the first time `state` is seen with its GPU lost.
pub(crate) fn announce_gpu_loss_once<W>(
    state: Option<&SharedState>,
    announced: &Cell<bool>,
    handlers: &RefCell<Vec<Handler<W>>>,
    widget: &W,
) {
    let lost = state
        .and_then(|state| state.try_borrow().ok().map(|state| state.gpu_is_lost()))
        .unwrap_or(false);
    if lost && !announced.replace(true) {
        run_handlers(handlers, widget);
    }
}
