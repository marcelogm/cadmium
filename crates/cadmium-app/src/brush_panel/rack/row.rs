//! One rack row: the brush's names on two lines, then its stroke.

use adw::prelude::*;
use gtk::gdk;

use super::stroke::StrokeView;
use crate::brush_panel::presentation::RackEntry;

const ROW_SPACING: i32 = 8;
const ROW_MARGIN: i32 = 4;

/// A row for `entry`, and its stroke, which shows `thumbnail` until a new one arrives.
pub(super) fn row(
    entry: &RackEntry,
    thumbnail: Option<&gdk::Texture>,
) -> (gtk::ListBoxRow, StrokeView) {
    let stroke = StrokeView::new();
    stroke.show(thumbnail);
    let content = gtk::Box::builder()
        .spacing(ROW_SPACING)
        .margin_top(ROW_MARGIN)
        .margin_bottom(ROW_MARGIN)
        .build();
    content.append(&names(entry));
    content.append(stroke.widget());
    let row = gtk::ListBoxRow::builder()
        .child(&content)
        .tooltip_text(entry.name.as_str())
        .build();
    row.update_property(&[gtk::accessible::Property::Label(&entry.name)]);
    (row, stroke)
}

fn names(entry: &RackEntry) -> gtk::Box {
    let names = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .build();
    names.append(&line(&entry.title));
    let subtitle = line(&entry.subtitle);
    subtitle.add_css_class("dim-label");
    subtitle.add_css_class("caption");
    names.append(&subtitle);
    names
}

fn line(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build()
}
