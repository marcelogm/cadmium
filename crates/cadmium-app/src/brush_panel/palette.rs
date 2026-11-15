use std::cell::RefCell;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use cadmium_engine::palette::ColourRef;
use cadmium_engine::pigments::MAX_SLOTS;
use gtk::glib;

use super::BrushPanel;
use super::presentation::{circle_shows, palette_view_name};
use super::signals::with_panel;
use super::snapshot::{PigmentChoice, Snapshot};
use crate::palette_colours::{colour_click, current_modifiers};
use crate::swatch::{Swatch, SwatchShape};

const PIGMENT_SWATCH_SIZE: i32 = 26;
const MY_COLOUR_TOOLTIP: &str = "My colour";
const ADD_LIST_SWATCH_SIZE: i32 = 18;
const ADD_LIST_SPACING: i32 = 10;
const ADD_LIST_ROW_MARGIN: i32 = 6;

/// What the palette group last drew, so a refresh rebuilds only what changed.
#[derive(Default)]
pub(super) struct ShownPalette {
    paint_swatches: RefCell<Vec<Swatch>>,
    slots: RefCell<Option<Vec<String>>>,
    my_colour_swatches: RefCell<Vec<Swatch>>,
    my_colours: RefCell<Option<Vec<[u8; 3]>>>,
}

impl BrushPanel {
    pub(super) fn show_palette(self: &Rc<Self>, snapshot: &Snapshot) {
        let shown = &self.shown_palette;
        if shown.slots.borrow().as_ref() != Some(&snapshot.slots) {
            self.rebuild_paints(snapshot);
        }
        if shown.my_colours.borrow().as_ref() != Some(&snapshot.palette.my_colours) {
            self.rebuild_my_colours(&snapshot.palette.my_colours);
        }
        self.highlight_colours(snapshot.palette.highlighted);
        self.show_palette_view(snapshot);
        self.show_wheel(snapshot);
    }

    fn rebuild_paints(self: &Rc<Self>, snapshot: &Snapshot) {
        let paints = &self.palette.paints;
        paints.remove_all();
        let mut swatches = Vec::with_capacity(snapshot.slots.len());
        for (slot, colour) in snapshot.slot_colours.iter().enumerate() {
            let tooltip = snapshot.tooltips.get(slot).map(String::as_str);
            let swatch = Swatch::new(
                *colour,
                SwatchShape::Circle,
                PIGMENT_SWATCH_SIZE,
                PIGMENT_SWATCH_SIZE,
            );
            paints.append(&self.colour_button(ColourRef::Paint(slot), &swatch, tooltip));
            swatches.push(swatch);
        }
        if snapshot.slots.len() < MAX_SLOTS && !snapshot.addable.is_empty() {
            paints.append(&self.add_pigment_button(&snapshot.addable));
        }
        self.shown_palette.paint_swatches.replace(swatches);
        let slots = Some(snapshot.slots.clone());
        self.shown_palette.slots.replace(slots);
    }

    /// The grid and its title are hidden while nothing is pinned.
    fn rebuild_my_colours(self: &Rc<Self>, colours: &[[u8; 3]]) {
        let grid = &self.palette.my_colours;
        grid.remove_all();
        let mut swatches = Vec::with_capacity(colours.len());
        for (index, colour) in colours.iter().enumerate() {
            let swatch = Swatch::new(
                *colour,
                SwatchShape::RoundedRect,
                PIGMENT_SWATCH_SIZE,
                PIGMENT_SWATCH_SIZE,
            );
            let colour = ColourRef::Mine(index);
            grid.append(&self.colour_button(colour, &swatch, Some(MY_COLOUR_TOOLTIP)));
            swatches.push(swatch);
        }
        grid.set_visible(!colours.is_empty());
        self.palette
            .my_colours_title
            .set_visible(!colours.is_empty());
        self.shown_palette.my_colour_swatches.replace(swatches);
        self.shown_palette
            .my_colours
            .replace(Some(colours.to_vec()));
    }

    fn highlight_colours(&self, highlighted: Option<ColourRef>) {
        let shown = &self.shown_palette;
        for (slot, swatch) in shown.paint_swatches.borrow().iter().enumerate() {
            swatch.set_selected(highlighted == Some(ColourRef::Paint(slot)));
        }
        for (index, swatch) in shown.my_colour_swatches.borrow().iter().enumerate() {
            swatch.set_selected(highlighted == Some(ColourRef::Mine(index)));
        }
    }

    fn show_palette_view(&self, snapshot: &Snapshot) {
        let palette = &self.palette;
        let name = palette_view_name(snapshot.palette.view);
        if palette.view_toggles.active_name().as_deref() != Some(name) {
            palette.view_toggles.set_active_name(Some(name));
        }
        palette.stack.set_visible_child_name(name);
        palette.darken.set_sensitive(snapshot.palette.can_darken);
        let darken = f64::from(snapshot.palette.circle_darken);
        palette.darken.set_value(darken);
    }

    /// Mixes the wheel again only while it shows and what it is mixed from changed.
    fn show_wheel(&self, snapshot: &Snapshot) {
        let shows = circle_shows(snapshot.tool, snapshot.palette.view);
        let state = self.state.borrow();
        let db = &state.painting.session.db;
        (self.palette.wheel).show_if_stale(shows, &snapshot.palette.wheel_key, db);
    }

    fn colour_button(
        self: &Rc<Self>,
        colour: ColourRef,
        swatch: &Swatch,
        tooltip: Option<&str>,
    ) -> gtk::Button {
        let button = gtk::Button::builder()
            .child(swatch)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .css_classes(["flat", "pigment-swatch"])
            .build();
        if matches!(colour, ColourRef::Paint(_)) {
            button.add_css_class("circular");
        }
        button.set_tooltip_text(tooltip);
        if let Some(name) = tooltip {
            button.update_property(&[gtk::accessible::Property::Label(name)]);
        }
        button.set_cursor_from_name(Some("pointer"));
        let panel = Rc::downgrade(self);
        button.connect_clicked(move |button| {
            let click = colour_click(current_modifiers(button));
            with_panel(&panel, |panel| panel.click_colour(colour, click));
        });
        button
    }

    fn add_pigment_button(self: &Rc<Self>, addable: &[PigmentChoice]) -> gtk::MenuButton {
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["navigation-sidebar"])
            .build();
        for choice in addable {
            list.append(&addable_row(choice));
        }
        let popover = gtk::Popover::builder().child(&list).build();
        let open_popover = popover.downgrade();
        let button = gtk::MenuButton::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Add pigment")
            .popover(&popover)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .css_classes(["circular", "flat"])
            .build();
        let panel = Rc::downgrade(self);
        list.connect_row_activated(move |_, row| add_chosen_pile(&panel, &open_popover, row));
        button
    }
}

fn addable_row(choice: &PigmentChoice) -> gtk::ListBoxRow {
    let line = gtk::Box::builder()
        .spacing(ADD_LIST_SPACING)
        .margin_top(ADD_LIST_ROW_MARGIN)
        .margin_bottom(ADD_LIST_ROW_MARGIN)
        .build();
    line.append(&Swatch::new(
        choice.colour,
        SwatchShape::Circle,
        ADD_LIST_SWATCH_SIZE,
        ADD_LIST_SWATCH_SIZE,
    ));
    line.append(&gtk::Label::new(Some(choice.name)));
    line.append(
        &gtk::Label::builder()
            .label(choice.colour_index)
            .hexpand(true)
            .xalign(1.0)
            .css_classes(["dim-label", "caption"])
            .build(),
    );
    gtk::ListBoxRow::builder()
        .child(&line)
        .name(choice.id)
        .build()
}

fn add_chosen_pile(
    panel: &Weak<BrushPanel>,
    popover: &glib::WeakRef<gtk::Popover>,
    row: &gtk::ListBoxRow,
) {
    if let Some(popover) = popover.upgrade() {
        popover.popdown();
    }
    let id = row.widget_name();
    if let Some(panel) = panel.upgrade() {
        panel.add_pile(&id);
    }
}
