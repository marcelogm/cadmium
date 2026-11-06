//! The Brushes group: one row per brush (its names and its stroke) and a library menu.

use std::cell::RefCell;

use adw::prelude::*;
use gtk::{gdk, gio};

use super::presentation::RackEntry;

mod fit;
mod row;
mod stroke;

use stroke::StrokeView;

const RACK_TITLE: &str = "Brushes";
const RACK_MENU_TOOLTIP: &str = "Brush Library";
/// How many rows show before the rack scrolls.
const VISIBLE_ROWS: i32 = 5;

pub(super) struct Rack {
    pub(super) group: adw::PreferencesGroup,
    pub(super) list: gtk::ListBox,
    /// Holds the list at a fixed height: the sidebar's own scroller would give it only its
    /// minimum.
    scroller: gtk::ScrolledWindow,
    /// Each row's stroke, in rack order, so a thumbnail lands on its row.
    strokes: RefCell<Vec<StrokeView>>,
}

pub(super) fn rack() -> Rack {
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(false)
        .css_classes(["navigation-sidebar"])
        .build();
    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .hexpand(true)
        .child(&list)
        .build();
    let card = gtk::Box::builder()
        .css_classes(["card"])
        .overflow(gtk::Overflow::Hidden)
        .build();
    card.append(&scroller);
    let sized = list.clone();
    scroller.connect_map(move |scroller| fit_height(scroller, &sized));
    let group = adw::PreferencesGroup::builder().title(RACK_TITLE).build();
    group.set_header_suffix(Some(&menu_button()));
    group.add(&card);
    Rack {
        group,
        list,
        scroller,
        strokes: RefCell::new(Vec::new()),
    }
}

/// Holds `scroller` at the list's measured height up to `VISIBLE_ROWS` rows: the list's
/// natural height less the rows past them, so its own padding and any text size count.
fn fit_height(scroller: &gtk::ScrolledWindow, list: &gtk::ListBox) {
    if list.root().is_none() {
        return;
    }
    let natural = |widget: &gtk::Widget| {
        let (_, natural, _, _) = widget.measure(gtk::Orientation::Vertical, -1);
        natural
    };
    let hidden: i32 = std::iter::successors(list.row_at_index(VISIBLE_ROWS), |row| {
        row.next_sibling().and_downcast::<gtk::ListBoxRow>()
    })
    .map(|row| natural(row.upcast_ref()))
    .sum();
    scroller.set_min_content_height(natural(list.upcast_ref()) - hidden);
}

fn menu_button() -> gtk::MenuButton {
    let menu = gio::Menu::new();
    menu.append(Some("Edit…"), Some("win.edit-brush"));
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

impl Rack {
    /// Refills the rack: one row per entry, with its thumbnail when one is ready.
    pub(super) fn fill(&self, entries: &[RackEntry], thumbnails: &[Option<gdk::Texture>]) {
        self.list.remove_all();
        let strokes = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let thumbnail = thumbnails.get(index).and_then(Option::as_ref);
                let (row, stroke) = row::row(entry, thumbnail);
                self.list.append(&row);
                stroke
            })
            .collect();
        self.strokes.replace(strokes);
        fit_height(&self.scroller, &self.list);
    }

    /// Shows `thumbnail` on the row at `index`.
    pub(super) fn show_thumbnail(&self, index: usize, thumbnail: &gdk::Texture) {
        if let Some(stroke) = self.strokes.borrow().get(index) {
            stroke.show(Some(thumbnail));
        }
    }

    pub(super) fn select(&self, index: Option<usize>) {
        let row = index
            .and_then(|index| i32::try_from(index).ok())
            .and_then(|index| self.list.row_at_index(index));
        match row {
            Some(row) => self.list.select_row(Some(&row)),
            None => self.list.unselect_all(),
        }
    }

    /// The rack position of the picked brush, if one is picked.
    pub(super) fn picked(&self) -> Option<usize> {
        let row = self.list.selected_row()?;
        usize::try_from(row.index()).ok()
    }
}
