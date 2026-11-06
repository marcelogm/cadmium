//! The test sheet in the editor's centre column: the painter tries the draft brush on it by
//! hand, with the painting's paint. The editor owns the sheet's canvas, so its video memory
//! goes with the editor; the widget only links to it.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use cadmium_engine::brush::BrushSpec;
use cadmium_studio::{StudioError, TEST_SHEET_SIZE, TestSheet};
use gtk::glib;

use super::{Editor, with_editor};
use crate::state::{AppState, SharedState};
use crate::window::CadmiumWindow;

mod view;

use view::TestSheetView;

/// What the toast says when the test sheet can't be shown.
pub(super) const SHEET_FAILED: &str = "Can't show the test sheet";
const CLEAR_MARGIN: i32 = 12;
/// Clear's label colour: the sheet is light paper in both themes, so the theme's own label
/// colour (white in the dark theme) would not read on it.
const CLEAR_INK: &str = "#2e3436";
const HINT: &str = "Paint here to try the brush";
const HINT_COLOUR: &str = "#a3a39c";

/// The editor's test sheet: its canvas, the widget showing it, and the overlay round it
/// (for the hint and Clear).
pub(super) struct Sheet {
    /// The sheet's canvas; the widget holds it weakly.
    shared: Rc<RefCell<TestSheet>>,
    view: TestSheetView,
    clear: gtk::Button,
    /// Shown on a blank sheet.
    hint: gtk::Label,
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
        let clear = clear_button();
        let hint = hint_label();
        let root = gtk::Overlay::builder().child(&view).build();
        root.add_overlay(&hint);
        root.add_overlay(&clear);
        view.connect_stroke_began(glib::clone!(
            #[weak]
            hint,
            move |_| hint.set_visible(false)
        ));
        Sheet {
            shared,
            view,
            clear,
            hint,
            root,
        }
    }

    /// A blank sheet invites a stroke again.
    fn show_hint(&self) {
        self.hint.set_visible(true);
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

    /// Clear wipes the sheet; the sheet's lost GPU and copy fallback go where the canvas's go.
    pub(super) fn connect_sheet(self: &Rc<Self>) {
        let editor = Rc::downgrade(self);
        self.sheet
            .clear
            .connect_clicked(move |_| with_editor(&editor, Editor::clear_sheet));
        let window = self.window.clone();
        self.sheet
            .view
            .connect_gpu_lost(move |_| with_window(&window, CadmiumWindow::announce_gpu_lost));
        let window = self.window.clone();
        self.sheet.view.connect_presentation_fallback(move |_| {
            with_window(&window, CadmiumWindow::announce_copy_presentation);
        });
    }

    /// A blank canvas; one that can't be made gives up on the editor.
    fn clear_sheet(self: &Rc<Self>) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let gpu = window.state().borrow().gpu.clone();
        self.sheet.view.finish_stroke();
        let cleared = self.sheet.shared.borrow_mut().clear(&gpu);
        match cleared {
            Ok(()) => {
                self.sheet.show_hint();
                self.sheet.view.queue_present();
            }
            Err(error) => self.give_up(SHEET_FAILED, &error),
        }
    }
}

/// Runs `act` on the window, while it is there.
fn with_window(window: &glib::WeakRef<CadmiumWindow>, act: fn(&CadmiumWindow)) {
    if let Some(window) = window.upgrade() {
        act(&window);
    }
}

/// Clear, in the sheet's top-right corner, in dark ink.
fn clear_button() -> gtk::Button {
    let label = gtk::Label::builder()
        .use_markup(true)
        .label(format!("<span foreground=\"{CLEAR_INK}\">Clear</span>"))
        .build();
    gtk::Button::builder()
        .child(&label)
        .tooltip_text("Clear the test sheet")
        .halign(gtk::Align::End)
        .valign(gtk::Align::Start)
        .margin_top(CLEAR_MARGIN)
        .margin_end(CLEAR_MARGIN)
        .build()
}

/// A faint line in the middle of a blank sheet; pointer events go through it, and screen
/// readers skip it (the sheet's own description says as much).
fn hint_label() -> gtk::Label {
    gtk::Label::builder()
        .use_markup(true)
        .label(format!("<span foreground=\"{HINT_COLOUR}\">{HINT}</span>"))
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .can_target(false)
        .accessible_role(gtk::AccessibleRole::Presentation)
        .build()
}

/// The test sheet's widget in `dialog`'s widget tree: smoke runs paint on it.
#[cfg(all(target_os = "linux", any(debug_assertions, feature = "smoke")))]
pub(crate) fn test_sheet_in(dialog: &adw::Dialog) -> Option<TestSheetView> {
    find_sheet(dialog.upcast_ref())
}

#[cfg(all(target_os = "linux", any(debug_assertions, feature = "smoke")))]
fn find_sheet(widget: &gtk::Widget) -> Option<TestSheetView> {
    if let Some(sheet) = widget.downcast_ref::<TestSheetView>() {
        return Some(sheet.clone());
    }
    let mut next = widget.first_child();
    while let Some(child) = next {
        if let Some(found) = find_sheet(&child) {
            return Some(found);
        }
        next = child.next_sibling();
    }
    None
}

#[cfg(test)]
mod tests;
