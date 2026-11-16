//! The Palette dialog: arranges the colour circle, mixes on the sheet, and pins colours to
//! My Colours.
//!
//! Under the header's Circle | Mixing switch, the centre shows the circle or the sheet; the
//! column on the right, the same in both modes, holds the chosen colour and the palette.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::glib;

use crate::colour_wheel::COLOUR_CIRCLE;
use crate::state::{AppState, PaletteMode, SharedState};
use crate::window::CadmiumWindow;

mod circle_page;
mod column;
mod header;
mod layout;
mod mixing_page;
mod presentation;
mod sheet_view;
mod signals;
mod snapshot;

use circle_page::CirclePage;
use column::{Column, ShownColumn};
use mixing_page::MixingPage;
use presentation::opening_mode;
#[cfg(all(target_os = "linux", any(debug_assertions, feature = "smoke")))]
pub(crate) use sheet_view::SheetView;
use snapshot::Snapshot;

/// The open dialog. Signal handlers hold it weakly; the dialog's `closed` handler holds the
/// one strong reference, and lets go of it there.
struct PaletteEditor {
    window: glib::WeakRef<CadmiumWindow>,
    state: SharedState,
    dialog: adw::Dialog,
    modes: adw::ToggleGroup,
    stack: gtk::Stack,
    circle: CirclePage,
    mixing: MixingPage,
    column: Column,
    shown: ShownColumn,
    /// Whether the dialog fills its widgets, so their signals are not the painter's.
    syncing: Cell<bool>,
    /// The window's undo and redo, heard while the dialog is open.
    history_handlers: RefCell<Vec<(gtk::gio::SimpleAction, glib::SignalHandlerId)>>,
}

/// Shows the dialog arranging the circle (mixing when the app has none); while an editor is open,
/// does nothing.
pub fn open(window: &CadmiumWindow) {
    open_in(window, PaletteMode::Circle);
}

/// Shows the dialog in `mode`; while an editor is open, does nothing.
pub fn open_in(window: &CadmiumWindow, mode: PaletteMode) {
    if window.has_editor() {
        return;
    }
    let state = window.state();
    {
        let mut state = state.borrow_mut();
        state.begin_palette_edit();
        state.set_palette_mode(opening_mode(mode, COLOUR_CIRCLE));
    }
    let editor = PaletteEditor::new(window, state);
    window.keep_editor(&editor.dialog, || ());
    window.refresh_panels();
    window.sync_history_actions();
    editor.dialog.present(Some(window));
}

impl PaletteEditor {
    fn new(window: &CadmiumWindow, state: SharedState) -> Rc<PaletteEditor> {
        let header = header::header();
        let circle = circle_page::circle_page();
        let mixing = mixing_page::mixing_page(state.clone());
        let column = column::column();
        let (dialog, stack) = layout::dialog(&header.bar, &circle.root, &mixing.root, &column);
        let editor = Rc::new(PaletteEditor {
            window: window.downgrade(),
            state,
            dialog,
            modes: header.modes,
            stack,
            circle,
            mixing,
            column,
            shown: ShownColumn::default(),
            syncing: Cell::new(false),
            history_handlers: RefCell::new(Vec::new()),
        });
        editor.connect(window);
        editor.refresh();
        editor.keep_until_closed();
        editor
    }

    /// Fills every widget from the state.
    fn refresh(self: &Rc<Self>) {
        let snapshot = Snapshot::of(&self.state.borrow());
        self.syncing.set(true);
        self.show_mode(&snapshot);
        self.show_chosen(&snapshot);
        self.show_colours(&snapshot);
        self.show_circle(&snapshot);
        self.show_sheet_tool(&snapshot);
        self.syncing.set(false);
    }

    /// Changes the state, then shows the change and what undo and redo can do now.
    fn change<T>(self: &Rc<Self>, change: impl FnOnce(&mut AppState) -> T) -> T {
        let changed = change(&mut self.state.borrow_mut());
        self.after_change();
        changed
    }

    fn after_change(self: &Rc<Self>) {
        self.refresh();
        if let Some(window) = self.window.upgrade() {
            window.sync_history_actions();
        }
    }

    /// Keeps the editor alive while its dialog is open; once closed, ends the edit and lets
    /// the window know.
    fn keep_until_closed(self: &Rc<Self>) {
        let keeper = RefCell::new(Some(Rc::clone(self)));
        self.dialog
            .connect_closed(move |_| keeper.take().iter().for_each(|editor| editor.close()));
    }

    fn close(&self) {
        for (action, handler) in self.history_handlers.take() {
            action.disconnect(handler);
        }
        self.column.remove_menu.unparent();
        let Some(window) = self.window.upgrade() else {
            return;
        };
        window.state().borrow_mut().end_palette_edit();
        window.after_editor_closed();
        window.sync_history_actions();
    }
}

/// Runs `action` on the editor while it is open and not filling its widgets.
fn with_editor(editor: &Weak<PaletteEditor>, action: impl FnOnce(&Rc<PaletteEditor>)) {
    if let Some(editor) = editor.upgrade()
        && !editor.syncing.get()
    {
        action(&editor);
    }
}

/// The mixing sheet's view in `dialog`'s widget tree: smoke runs wait for it to rest.
#[cfg(all(target_os = "linux", any(debug_assertions, feature = "smoke")))]
pub(crate) fn sheet_view_in(dialog: &adw::Dialog) -> Option<SheetView> {
    find_sheet_view(dialog.upcast_ref())
}

#[cfg(all(target_os = "linux", any(debug_assertions, feature = "smoke")))]
fn find_sheet_view(widget: &gtk::Widget) -> Option<SheetView> {
    if let Some(view) = widget.downcast_ref::<SheetView>() {
        return Some(view.clone());
    }
    let mut next = widget.first_child();
    while let Some(child) = next {
        if let Some(found) = find_sheet_view(&child) {
            return Some(found);
        }
        next = child.next_sibling();
    }
    None
}
