//! The Brushes group: a rack of brushes, each its stroke above its name, and a library menu.

use adw::prelude::*;
use gtk::{gdk, gio};

const RACK_TITLE: &str = "Brushes";
const RACK_MENU_TOOLTIP: &str = "Brush Library";
const RACK_COLUMNS: u32 = 3;
const RACK_SPACING: u32 = 4;
const RACK_MARGIN: i32 = 8;
const NAME_LINES: i32 = 2;
const NAME_MARGIN: i32 = 4;
const ITEM_MARGIN: i32 = 4;
/// A third of the thumbnail's height: low enough for compact rows at the sidebar width.
const PICTURE_HEIGHT: i32 = 32;

pub(super) struct Rack {
    pub(super) group: adw::PreferencesGroup,
    pub(super) flow: gtk::FlowBox,
}

pub(super) fn rack() -> Rack {
    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .homogeneous(true)
        .max_children_per_line(RACK_COLUMNS)
        .min_children_per_line(RACK_COLUMNS)
        .row_spacing(RACK_SPACING)
        .column_spacing(RACK_SPACING)
        .margin_top(RACK_MARGIN)
        .margin_bottom(RACK_MARGIN)
        .margin_start(RACK_MARGIN)
        .margin_end(RACK_MARGIN)
        .build();
    let card = gtk::Box::builder().css_classes(["card"]).build();
    card.append(&flow);
    let group = adw::PreferencesGroup::builder().title(RACK_TITLE).build();
    group.set_header_suffix(Some(&menu_button()));
    group.add(&card);
    Rack { group, flow }
}

fn menu_button() -> gtk::MenuButton {
    let menu = gio::Menu::new();
    menu.append(Some("Import…"), Some("win.import-brush"));
    menu.append(Some("Export…"), Some("win.export-brush"));
    menu.append(Some("Delete"), Some("win.delete-brush"));
    gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(RACK_MENU_TOOLTIP)
        .menu_model(&menu)
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build()
}

/// Refills the rack: one entry per name, with its thumbnail when one is ready.
pub(super) fn fill(flow: &gtk::FlowBox, names: &[String], thumbnails: &[Option<gdk::Texture>]) {
    flow.remove_all();
    for (index, name) in names.iter().enumerate() {
        let thumbnail = thumbnails.get(index).and_then(Option::as_ref);
        flow.append(&entry(name, thumbnail));
    }
}

fn entry(name: &str, thumbnail: Option<&gdk::Texture>) -> gtk::Box {
    let picture = gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Contain)
        .can_shrink(true)
        .height_request(PICTURE_HEIGHT)
        .margin_top(ITEM_MARGIN)
        .build();
    picture.set_paintable(thumbnail);
    let label = gtk::Label::builder()
        .label(name)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .lines(NAME_LINES)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .margin_top(NAME_MARGIN)
        .margin_bottom(NAME_MARGIN)
        .build();
    let entry = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .tooltip_text(name)
        .build();
    entry.append(&picture);
    entry.append(&label);
    entry
}

/// Shows `thumbnail` on the rack entry at `index`.
pub(super) fn show_thumbnail(flow: &gtk::FlowBox, index: usize, thumbnail: &gdk::Texture) {
    let picture = i32::try_from(index)
        .ok()
        .and_then(|index| flow.child_at_index(index))
        .and_then(|child| child.child())
        .and_then(|entry| entry.first_child())
        .and_downcast::<gtk::Picture>();
    if let Some(picture) = picture {
        picture.set_paintable(Some(thumbnail));
    }
}

pub(super) fn select(flow: &gtk::FlowBox, index: Option<usize>) {
    let child = index
        .and_then(|index| i32::try_from(index).ok())
        .and_then(|index| flow.child_at_index(index));
    match child {
        Some(child) => flow.select_child(&child),
        None => flow.unselect_all(),
    }
}

/// The rack position of the picked brush, if one is picked.
pub(super) fn picked(flow: &gtk::FlowBox) -> Option<usize> {
    let child = flow.selected_children().into_iter().next()?;
    usize::try_from(child.index()).ok()
}
