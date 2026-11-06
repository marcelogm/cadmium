//! Painting the example strokes: the old picture dimmed first, then one part per
//! low-priority idle tick, so the editor never stalls before the dim shows.

use std::cell::Cell;
use std::rc::{Rc, Weak};

use cadmium_engine::brush::BrushSpec;
use cadmium_studio::{StudioError, Thumbnail};
use gtk::glib;
use gtk::prelude::*;

use super::Editor;
use super::closing::Closing;
use super::examples::{Order, Tick, failure_toast};
use crate::LOG_DOMAIN;
use crate::thumbnails::{ThumbnailKey, texture};

/// One painting of the examples, carried by its idle ticks.
struct Painting {
    order: Order,
    brush: BrushSpec,
    started: bool,
}

impl Editor {
    /// Paints the examples of the draft as it is now, unless they show it, or are being
    /// painted with it, already.
    pub(super) fn paint_examples(self: &Rc<Self>) {
        let brush = self.draft.borrow().current().clone();
        let fired = self
            .examples
            .queue
            .borrow_mut()
            .fire(ThumbnailKey::of(&brush));
        let Some(order) = fired else {
            return;
        };
        self.examples.view.dim();
        self.set_examples_pending(true);
        let painting = Cell::new(Some(Painting {
            order,
            brush,
            started: false,
        }));
        let editor = Rc::downgrade(self);
        // A tick callback runs as a frame starts: the idle queued from it runs after the
        // frame that paints the dim.
        self.examples
            .view
            .widget()
            .add_tick_callback(move |_, _| queue_once(&editor, &painting));
    }

    /// Starts the painting on its first tick, then paints one part per tick.
    fn advance_examples(&self, painting: &mut Painting) -> Tick {
        let mut job = self.examples.job.borrow_mut();
        if painting.started {
            return job.tick();
        }
        painting.started = true;
        match job.start(&painting.brush) {
            Ok(()) => Tick::Working,
            Err(error) => Tick::Failed(error),
        }
    }

    fn after_example_tick(self: &Rc<Self>, order: Order, tick: Tick) -> glib::ControlFlow {
        match tick {
            Tick::Working => return glib::ControlFlow::Continue,
            Tick::Done(sheet) => self.show_examples(order, sheet),
            Tick::Failed(error) => self.examples_failed(order, &error),
            Tick::Idle => {
                self.examples.queue.borrow_mut().failed(order);
                self.set_examples_pending(false);
            }
        }
        glib::ControlFlow::Break
    }

    fn show_examples(&self, order: Order, sheet: Thumbnail) {
        if self.examples.queue.borrow_mut().finished(order) {
            self.examples.view.show(&texture(sheet));
            self.set_examples_pending(false);
        }
    }

    /// Closes the editor and says why; a lost GPU goes to the window's GPU-lost dialog,
    /// which closes the editor first.
    fn examples_failed(self: &Rc<Self>, order: Order, error: &StudioError) {
        self.examples.queue.borrow_mut().failed(order);
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let gpu_lost = window.state().borrow().gpu_is_lost();
        match failure_toast(error, gpu_lost) {
            Some(toast) => {
                glib::g_warning!(LOG_DOMAIN, "{toast}");
                self.finish(Closing::Done);
                window.show_toast(&toast);
            }
            None => window.announce_gpu_lost(),
        }
    }

    fn set_examples_pending(&self, pending: bool) {
        if let Some(window) = self.window.upgrade() {
            window.set_examples_pending(pending);
        }
    }
}

/// Queues the painting's idle ticks, the first time the tick callback runs.
fn queue_once(editor: &Weak<Editor>, painting: &Cell<Option<Painting>>) -> glib::ControlFlow {
    if let Some(painting) = painting.take() {
        queue_ticks(editor.clone(), painting);
    }
    glib::ControlFlow::Break
}

fn queue_ticks(editor: Weak<Editor>, mut painting: Painting) {
    glib::idle_add_local_full(glib::Priority::LOW, move || {
        example_tick(&editor, &mut painting)
    });
}

/// One part of `painting`; it stops once the editor closes or a newer painting supersedes it.
fn example_tick(editor: &Weak<Editor>, painting: &mut Painting) -> glib::ControlFlow {
    let Some(editor) = editor.upgrade() else {
        return glib::ControlFlow::Break;
    };
    let current = editor.examples.queue.borrow().is_current(painting.order);
    if !editor.listening.heeds() || !current {
        return glib::ControlFlow::Break;
    }
    let tick = editor.advance_examples(painting);
    editor.after_example_tick(painting.order, tick)
}
