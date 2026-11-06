//! The test sheet in the editor's centre column: the painter tries the draft brush on it by
//! hand, with the painting's paint. The editor owns the sheet's canvas, so its video memory
//! goes with the editor; the widget only links to it.

use std::cell::RefCell;
use std::rc::Rc;

use cadmium_engine::brush::BrushSpec;
use cadmium_studio::{StudioError, TEST_SHEET_SIZE, TestSheet};
use gtk::glib;

use super::Editor;
use crate::state::{AppState, SharedState};
use crate::window::CadmiumWindow;

mod view;

use view::TestSheetView;

/// What the toast says when the test sheet can't be shown.
pub(super) const SHEET_FAILED: &str = "Can't show the test sheet";

/// The editor's test sheet: its canvas, the widget showing it, and the overlay round it
/// (for the hint and Clear).
pub(super) struct Sheet {
    /// The sheet's canvas; the widget holds it weakly.
    pub(super) shared: Rc<RefCell<TestSheet>>,
    pub(super) view: TestSheetView,
    root: gtk::Overlay,
}

/// A blank sheet whose strokes take `brush`, in the painting's paint.
pub(super) fn new_sheet(state: &AppState, brush: &BrushSpec) -> Result<TestSheet, StudioError> {
    TestSheet::new(&state.gpu, &state.studio_paint(), brush, TEST_SHEET_SIZE)
}

impl Sheet {
    pub(super) fn new(state: SharedState, sheet: TestSheet) -> Sheet {
        let shared = Rc::new(RefCell::new(sheet));
        let view = TestSheetView::new(state, &shared);
        let root = gtk::Overlay::builder().child(&view).build();
        Sheet { shared, view, root }
    }

    pub(super) fn widget(&self) -> &gtk::Overlay {
        &self.root
    }
}

impl Editor {
    /// The draft as it is now paints the sheet's next stroke.
    pub(super) fn sheet_brush_changed(&self) {
        let draft = self.draft.borrow();
        self.sheet.shared.borrow_mut().set_brush(draft.current());
    }

    /// The sheet's lost GPU and copy fallback go where the canvas's go.
    pub(super) fn connect_sheet(&self) {
        let window = self.window.clone();
        self.sheet
            .view
            .connect_gpu_lost(move |_| with_window(&window, CadmiumWindow::announce_gpu_lost));
        let window = self.window.clone();
        self.sheet.view.connect_presentation_fallback(move |_| {
            with_window(&window, CadmiumWindow::announce_copy_presentation);
        });
    }
}

/// Runs `act` on the window, while it is there.
fn with_window(window: &glib::WeakRef<CadmiumWindow>, act: fn(&CadmiumWindow)) {
    if let Some(window) = window.upgrade() {
        act(&window);
    }
}

#[cfg(test)]
mod tests;
