use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, glib};

use super::BrushPanel;
use super::presentation::{offers_more_pigments, pigment_click};
use super::snapshot::{PigmentChoice, Snapshot};
use crate::swatch::{Swatch, SwatchShape};

const PIGMENT_SWATCH_SIZE: i32 = 26;
const ADD_LIST_SWATCH_SIZE: i32 = 18;
const ADD_LIST_SPACING: i32 = 10;
const ADD_LIST_ROW_MARGIN: i32 = 6;

impl BrushPanel {
    pub(super) fn rebuild_pigments(self: &Rc<Self>, snapshot: &Snapshot) {
        self.pigments.flow.remove_all();
        let mut swatches = Vec::with_capacity(snapshot.slots.len());
        for (slot, colour) in snapshot.slot_colours.iter().enumerate() {
            let tooltip = snapshot.tooltips.get(slot).map(String::as_str);
            let (button, swatch) = self.pile_button(slot, *colour, tooltip);
            self.pigments.flow.append(&button);
            swatches.push(swatch);
        }
        if offers_more_pigments(snapshot.slots.len(), snapshot.addable.len()) {
            self.pigments
                .flow
                .append(&self.add_pigment_button(&snapshot.addable));
        }
        self.pigment_swatches.replace(swatches);
        self.shown_slots.replace(Some(snapshot.slots.clone()));
    }

    fn pile_button(
        self: &Rc<Self>,
        slot: usize,
        colour: [u8; 3],
        tooltip: Option<&str>,
    ) -> (gtk::Button, Swatch) {
        let swatch = Swatch::new(
            colour,
            SwatchShape::Circle,
            PIGMENT_SWATCH_SIZE,
            PIGMENT_SWATCH_SIZE,
        );
        let button = gtk::Button::builder()
            .child(&swatch)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .css_classes(["flat", "circular", "pigment-swatch"])
            .build();
        button.set_tooltip_text(tooltip);
        if let Some(name) = tooltip {
            button.update_property(&[gtk::accessible::Property::Label(name)]);
        }
        button.set_cursor_from_name(Some("pointer"));
        let panel = Rc::downgrade(self);
        button
            .connect_clicked(move |button| click_pile_with_current_modifiers(&panel, slot, button));
        (button, swatch)
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

fn click_pile_with_current_modifiers(panel: &Weak<BrushPanel>, slot: usize, button: &gtk::Button) {
    let click = pigment_click(current_modifiers(button));
    if let Some(panel) = panel.upgrade() {
        panel.click_pile(slot, click);
    }
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

fn current_modifiers(widget: &impl IsA<gtk::Widget>) -> gdk::ModifierType {
    widget
        .display()
        .default_seat()
        .and_then(|seat| seat.keyboard())
        .map(|keyboard| keyboard.modifier_state())
        .unwrap_or_else(gdk::ModifierType::empty)
}
