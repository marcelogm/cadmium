//! The Brushes group: a rack of brush names and a menu for the brush library.

use adw::prelude::*;
use gtk::gio;

const RACK_TITLE: &str = "Brushes";
const RACK_MENU_TOOLTIP: &str = "Brush Library";
const RACK_COLUMNS: u32 = 3;
const RACK_SPACING: u32 = 4;
const RACK_MARGIN: i32 = 8;
const NAME_LINES: i32 = 2;
const NAME_MARGIN: i32 = 6;

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

pub(super) fn fill(flow: &gtk::FlowBox, names: &[String]) {
    flow.remove_all();
    for name in names {
        let label = gtk::Label::builder()
            .label(name)
            .wrap(true)
            .justify(gtk::Justification::Center)
            .lines(NAME_LINES)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .tooltip_text(name)
            .margin_top(NAME_MARGIN)
            .margin_bottom(NAME_MARGIN)
            .build();
        flow.append(&label);
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
