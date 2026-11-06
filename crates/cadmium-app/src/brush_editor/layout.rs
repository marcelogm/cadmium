//! The editor dialog's frame: under the header, three columns — the tip's drawing with
//! the example strokes under it, the test sheet, and the controls.

use adw::prelude::*;

use super::paper;

const DIALOG_SIZE: [i32; 2] = [1240, 840];
/// The drawing column's width, its margins included: 312 for the schematic and examples.
const DRAWING_WIDTH: i32 = 340;
const CONTROLS_WIDTH: i32 = 360;
const COLUMN_MARGIN: i32 = 14;
const COLUMN_SPACING: i32 = 10;
const CONTROLS_SPACING: i32 = 18;
const SHEET_MARGIN: i32 = 18;

/// The columns the editor fills.
pub(super) struct Columns {
    /// Left: the schematic goes on top (prepended), the example strokes' paper is under it.
    pub(super) drawing: gtk::Box,
    /// Right, scrolling: the Brush, Head and Hair groups.
    pub(super) controls: gtk::Box,
}

/// The dialog titled `title` under `header`. It never closes by itself (`can-close` false):
/// every way out goes through the editor (`close-attempt`, `force_close`).
pub(super) fn dialog(title: &str, header: &adw::HeaderBar) -> (adw::Dialog, Columns) {
    let columns = Columns {
        drawing: drawing_column(),
        controls: controls_column(),
    };
    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.append(&columns.drawing);
    body.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    body.append(&paper::test_sheet_placeholder(SHEET_MARGIN));
    body.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    body.append(&scrolling(&columns.controls));
    let content = adw::ToolbarView::new();
    content.add_top_bar(header);
    content.set_content(Some(&body));
    let dialog = adw::Dialog::builder()
        .title(title)
        .content_width(DIALOG_SIZE[0])
        .content_height(DIALOG_SIZE[1])
        .can_close(false)
        .child(&content)
        .build();
    (dialog, columns)
}

fn drawing_column() -> gtk::Box {
    let column = margined(gtk::Box::new(gtk::Orientation::Vertical, COLUMN_SPACING));
    // A size request leaves the margins out.
    column.set_width_request(DRAWING_WIDTH - 2 * COLUMN_MARGIN);
    column.append(&paper::examples_placeholder());
    column
}

fn controls_column() -> gtk::Box {
    margined(gtk::Box::new(gtk::Orientation::Vertical, CONTROLS_SPACING))
}

/// The controls' column keeps its width: the entry row inside would otherwise make it share
/// the room left with the test sheet.
fn scrolling(controls: &gtk::Box) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .hexpand(false)
        .width_request(CONTROLS_WIDTH)
        .child(controls)
        .build()
}

fn margined(column: gtk::Box) -> gtk::Box {
    column.set_margin_top(COLUMN_MARGIN);
    column.set_margin_bottom(COLUMN_MARGIN);
    column.set_margin_start(COLUMN_MARGIN);
    column.set_margin_end(COLUMN_MARGIN);
    column
}
