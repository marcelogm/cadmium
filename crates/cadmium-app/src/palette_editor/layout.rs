//! The dialog's frame: under the header, the page of the mode in the centre and the column
//! on the right.

use adw::prelude::*;

use super::column::Column;
use super::header::TITLE;
use super::presentation::mode_name;
use crate::brush_editor::DIALOG_SIZE;
use crate::colour_wheel::COLOUR_CIRCLE;
use crate::state::PaletteMode;

const COLUMN_WIDTH: i32 = 340;

/// The dialog under `header`, with the circle and mixing pages in a stack in the centre and
/// `column` on the right; returns the stack too.
pub(super) fn dialog(
    header: &adw::HeaderBar,
    circle: &gtk::Box,
    mixing: &gtk::Box,
    column: &Column,
) -> (adw::Dialog, gtk::Stack) {
    let stack = gtk::Stack::builder().hexpand(true).vexpand(true).build();
    if COLOUR_CIRCLE {
        stack.add_named(circle, Some(mode_name(PaletteMode::Circle)));
    }
    stack.add_named(mixing, Some(mode_name(PaletteMode::Mixing)));
    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.append(&stack);
    body.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    body.append(&scrolling(&column.root));
    let content = adw::ToolbarView::new();
    content.add_top_bar(header);
    content.set_content(Some(&body));
    let dialog = adw::Dialog::builder()
        .title(TITLE)
        .content_width(DIALOG_SIZE[0])
        .content_height(DIALOG_SIZE[1])
        .child(&content)
        .build();
    (dialog, stack)
}

/// The column keeps its width; the page in the centre takes the room left.
fn scrolling(column: &gtk::Box) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .hexpand(false)
        .width_request(COLUMN_WIDTH)
        .child(column)
        .build()
}
