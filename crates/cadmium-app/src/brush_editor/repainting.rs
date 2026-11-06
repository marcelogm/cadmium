//! Painting the example strokes: a moment after the draft settles (at once for a preset),
//! the old picture dimmed first, then one part per low-priority idle tick, so the editor
//! never stalls before the dim shows.

use std::cell::Cell;
use std::rc::{Rc, Weak};
use std::time::Duration;

use cadmium_engine::brush::BrushSpec;
use cadmium_studio::{StudioError, Thumbnail};
use gtk::glib;
use gtk::prelude::*;

use super::examples::{EXAMPLES_FAILED, Order, Tick};
use super::{Editor, with_editor};
use crate::thumbnails::{ThumbnailKey, texture};

/// How long the draft must stay still before a slider's change is painted.
const DEBOUNCE: Duration = Duration::from_millis(250);

/// How a draft change reaches the examples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Change {
    /// A slider or the size, which move in runs: painted once the draft settles.
    Settling,
    /// A preset: painted at once.
    Immediate,
}

/// One painting of the examples, carried by its idle ticks.
struct Painting {
    order: Order,
    brush: BrushSpec,
    started: bool,
}

impl Editor {
    /// The draft changed: paints the examples again, at once or once it settles.
    pub(super) fn draft_changed(self: &Rc<Self>, change: Change) {
        self.cancel_debounce();
        match change {
            Change::Immediate => self.paint_examples(),
            Change::Settling => self.debounce_examples(),
        }
    }

    fn debounce_examples(self: &Rc<Self>) {
        let editor = Rc::downgrade(self);
        let source = glib::timeout_add_local_once(DEBOUNCE, move || debounced(&editor));
        self.examples.debounce.replace(Some(source));
    }

    /// Stops a running debounce. Its callback clears the slot as it fires, so a source in
    /// the slot is always still there to remove.
    pub(super) fn cancel_debounce(&self) {
        if let Some(source) = self.examples.debounce.take() {
            source.remove();
        }
    }

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

    /// The painting failed: the editor gives up on it.
    fn examples_failed(self: &Rc<Self>, order: Order, error: &StudioError) {
        self.examples.queue.borrow_mut().failed(order);
        self.set_examples_pending(false);
        self.give_up(EXAMPLES_FAILED, error);
    }

    fn set_examples_pending(&self, pending: bool) {
        if let Some(window) = self.window.upgrade() {
            window.set_examples_pending(pending);
        }
    }
}

/// The draft settled: the debounce is over, and the examples are painted.
fn debounced(editor: &Weak<Editor>) {
    if let Some(fired) = editor.upgrade() {
        fired.examples.debounce.take();
    }
    with_editor(editor, Editor::paint_examples);
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
