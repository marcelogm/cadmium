use std::cell::RefCell;

pub(crate) type Handler<W> = Box<dyn Fn(&W)>;

pub(crate) fn run_handlers<W>(handlers: &RefCell<Vec<Handler<W>>>, widget: &W) {
    for handler in handlers.borrow().iter() {
        handler(widget);
    }
}
