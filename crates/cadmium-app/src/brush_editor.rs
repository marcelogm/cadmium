//! The brush editor: edits the worn brush as a draft, then saves it to the library.
//!
//! Three columns: the tip's drawing with the example strokes under it, the test sheet,
//! and the controls. The examples and the test sheet are blank paper for now.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use cadmium_studio::{HairPreset, HeadPreset};
use gtk::glib;

use crate::state::AppState;
use crate::surface_scale::device_scale;
use crate::window::CadmiumWindow;

mod brush_group;
mod closing;
mod controls;
mod draft;
mod header;
mod layout;
mod listening;
mod paper;
mod parts;
mod saving;
mod schematic;
mod wiring;

use brush_group::BrushGroup;
use draft::{Draft, Origin};
use listening::Listening;
use parts::PartGroups;
use schematic::Subject;

/// The open editor. Signal handlers hold it weakly; the dialog's `closed` handler holds the
/// one strong reference, and lets go of it there.
struct Editor {
    window: glib::WeakRef<CadmiumWindow>,
    dialog: adw::Dialog,
    draft: RefCell<Draft>,
    /// Whether the widgets' signals reach the editor: not while it fills them, nor once closing.
    listening: Listening,
    /// The alert asking about the editor ("Discard Changes?", "Delete …?"), while it is open.
    alert: RefCell<Option<adw::AlertDialog>>,
    brush: BrushGroup,
    parts: PartGroups,
    schematic: gtk::DrawingArea,
}

/// Shows the editor on the worn brush; while one is open, does nothing.
pub fn open(window: &CadmiumWindow) {
    if window.has_editor() {
        return;
    }
    let draft = draft_for(&window.state().borrow());
    let editor = Editor::new(window, draft);
    window.keep_editor(&editor.dialog);
    editor.dialog.present(Some(window));
}

/// What the editor edits: the worn library brush as the library holds it (at its suggested
/// size, whatever size the painting uses now); a worn brush in no library is edited like a
/// factory one.
fn draft_for(state: &AppState) -> Draft {
    let worn = state
        .worn_library_brush()
        .and_then(|index| Some((index, state.brushes.get(index)?.clone())));
    match worn {
        Some((index, brush)) if state.brushes.is_user(index) => {
            Draft::open(brush, Origin::User(index))
        }
        Some((_, brush)) => Draft::open(brush, Origin::Factory),
        None => Draft::open(state.worn_brush_spec(), Origin::Factory),
    }
}

impl Editor {
    fn new(window: &CadmiumWindow, draft: Draft) -> Rc<Editor> {
        let header = header::header();
        let (dialog, columns) = layout::dialog(&draft.chosen_name(), &header.bar);
        let brush = brush_group::brush_group();
        columns.controls.append(&brush.group);
        let parts = parts::part_groups();
        columns.controls.append(&parts.head);
        columns.controls.append(&parts.hair);
        let schematic = schematic::schematic_area();
        columns.drawing.prepend(&schematic);
        let editor = Rc::new(Editor {
            window: window.downgrade(),
            dialog,
            draft: RefCell::new(draft),
            listening: Listening::default(),
            alert: RefCell::new(None),
            brush,
            parts,
            schematic,
        });
        editor.connect(&header);
        editor.connect_schematic();
        editor.sync();
        editor.keep_until_closed();
        editor
    }

    /// Fills every widget from the draft.
    fn sync(&self) {
        self.listening.set_syncing(true);
        let draft = self.draft.borrow();
        self.brush.show(draft.current());
        self.parts.show(&draft);
        drop(draft);
        self.schematic.queue_draw();
        self.listening.set_syncing(false);
    }

    /// Draws the draft's schematic, whenever the widget asks.
    fn connect_schematic(self: &Rc<Self>) {
        let editor = Rc::downgrade(self);
        self.schematic
            .set_draw_func(move |area, context, width, height| {
                draw_schematic(&editor, area, context, [width, height]);
            });
    }

    /// Keeps the editor alive while its dialog is open; once closed, closes an alert still
    /// asking about it and lets the window know.
    fn keep_until_closed(self: &Rc<Self>) {
        let keeper = RefCell::new(Some(Rc::clone(self)));
        let window = self.window.clone();
        self.dialog
            .connect_closed(move |_| after_closed(&keeper, &window));
    }
}

/// The closed editor's last act: closes an alert still asking about it, lets go of it, and
/// tells the window.
fn after_closed(keeper: &RefCell<Option<Rc<Editor>>>, window: &glib::WeakRef<CadmiumWindow>) {
    if let Some(editor) = keeper.take() {
        editor.dismiss_alert();
    }
    if let Some(window) = window.upgrade() {
        window.after_editor_closed();
    }
}

fn draw_schematic(
    editor: &Weak<Editor>,
    area: &gtk::DrawingArea,
    context: &gtk::cairo::Context,
    size: [i32; 2],
) {
    let Some(editor) = editor.upgrade() else {
        return;
    };
    let draft = editor.draft.borrow();
    let subject = Subject {
        brush: draft.current(),
        head: draft.head_preset().map(HeadPreset::name),
        hair: draft.hair_preset().map(HairPreset::name),
    };
    schematic::draw(context, &subject, size, device_scale(area));
}

/// Runs `action` on the editor while it is open, not closing and not filling its widgets.
fn with_editor(editor: &Weak<Editor>, action: impl FnOnce(&Rc<Editor>)) {
    if let Some(editor) = editor.upgrade()
        && editor.listening.heeds()
    {
        action(&editor);
    }
}

#[cfg(test)]
mod tests;
