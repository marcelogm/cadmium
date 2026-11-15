//! The column on the right, the same in both modes: the chosen colour with its buttons, then
//! the paints and My Colours, each a source to drag onto the circle.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use cadmium_engine::palette::ColourRef;
use gtk::{gdk, gio, graphene};

use super::presentation::{chosen_hint, pin_button_is_enabled, shows_circle_markers};
use super::snapshot::Snapshot;
use super::{PaletteEditor, with_editor};
use crate::colour_wheel::colour_ref_text;
use crate::palette_colours::current_modifiers;
use crate::swatch::{Swatch, SwatchShape};

const COLUMN_MARGIN: i32 = 14;
const COLUMN_SPACING: i32 = 10;
const CHOSEN_SPACING: i32 = 12;
const CHOSEN_SWATCH_SIZE: i32 = 48;
const COLOUR_SWATCH_SIZE: i32 = 26;
const COLOURS_PER_LINE: u32 = 8;
const MIN_COLOURS_PER_LINE: u32 = 4;
const COLOUR_SPACING: u32 = 4;
const NO_COLOUR: [u8; 3] = [225, 225, 225];
const CHOSEN_TITLE: &str = "Chosen Colour";
const PAINTS_TITLE: &str = "Paints";
const MY_COLOURS_TITLE: &str = "My Colours";
const PICK_LABEL: &str = "Pick Colour from Sheet";
const PIN_LABEL: &str = "Pin to My Colours";
const REMOVE_LABEL: &str = "Remove";
const MY_COLOUR_TOOLTIP: &str = "My colour";
/// The dialog's own actions: `palette.remove` takes the offered colour out of My Colours.
pub(super) const ACTION_GROUP: &str = "palette";
pub(super) const REMOVE_ACTION: &str = "remove";
const MARKER_CLASS: &str = "circle-marker";
const COLOUR_CURSOR: &str = "pointer";

pub(super) struct Column {
    pub(super) root: gtk::Box,
    pub(super) chosen_swatch: Swatch,
    pub(super) chosen_hint: gtk::Label,
    pub(super) pick: gtk::Button,
    pub(super) pin: gtk::Button,
    pub(super) paints: gtk::FlowBox,
    pub(super) my_colours_title: gtk::Label,
    pub(super) my_colours: gtk::FlowBox,
    /// "Remove", offered on a right click on one of My Colours.
    pub(super) remove_menu: gtk::PopoverMenu,
    /// The colour of My Colours the menu is offered for.
    pub(super) offered: Cell<Option<usize>>,
}

/// A colour on show: its swatch and the marker saying it is on the circle.
struct ColourCell {
    swatch: Swatch,
    marker: gtk::Box,
}

/// The paints' colours and tooltips, in slot order.
type ShownPaints = (Vec<[u8; 3]>, Vec<String>);

/// What the column last drew, so a refresh rebuilds only what changed.
#[derive(Default)]
pub(super) struct ShownColumn {
    paints: RefCell<Option<ShownPaints>>,
    paint_cells: RefCell<Vec<ColourCell>>,
    my_colours: RefCell<Option<Vec<[u8; 3]>>>,
    my_cells: RefCell<Vec<ColourCell>>,
}

/// The chosen colour's line: its swatch and the hint beside it.
fn chosen_line() -> (gtk::Box, Swatch, gtk::Label) {
    let chosen_swatch = Swatch::new(
        NO_COLOUR,
        SwatchShape::RoundedRect,
        CHOSEN_SWATCH_SIZE,
        CHOSEN_SWATCH_SIZE,
    );
    let chosen_hint = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .hexpand(true)
        .css_classes(["dim-label"])
        .build();
    let chosen = gtk::Box::builder().spacing(CHOSEN_SPACING).build();
    chosen.append(&chosen_swatch);
    chosen.append(&chosen_hint);
    (chosen, chosen_swatch, chosen_hint)
}

pub(super) fn column() -> Column {
    let (chosen, chosen_swatch, chosen_hint) = chosen_line();
    let pick = gtk::Button::with_label(PICK_LABEL);
    let pin = gtk::Button::builder()
        .label(PIN_LABEL)
        .css_classes(["suggested-action"])
        .build();
    let paints = colour_grid();
    let my_colours_title = title(MY_COLOURS_TITLE);
    let my_colours = colour_grid();
    let root = column_box();
    for child in [
        title(CHOSEN_TITLE).upcast_ref(),
        chosen.upcast_ref(),
        pick.upcast_ref::<gtk::Widget>(),
        pin.upcast_ref(),
        title(PAINTS_TITLE).upcast_ref(),
        paints.upcast_ref(),
        my_colours_title.upcast_ref(),
        my_colours.upcast_ref(),
    ] {
        root.append(child);
    }
    let remove_menu = remove_menu(&root);
    Column {
        root,
        chosen_swatch,
        chosen_hint,
        pick,
        pin,
        paints,
        my_colours_title,
        my_colours,
        remove_menu,
        offered: Cell::new(None),
    }
}

fn column_box() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(COLUMN_SPACING)
        .margin_top(COLUMN_MARGIN)
        .margin_bottom(COLUMN_MARGIN)
        .margin_start(COLUMN_MARGIN)
        .margin_end(COLUMN_MARGIN)
        .build()
}

fn title(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .css_classes(["caption-heading", "dim-label"])
        .build()
}

fn colour_grid() -> gtk::FlowBox {
    gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .max_children_per_line(COLOURS_PER_LINE)
        .min_children_per_line(MIN_COLOURS_PER_LINE)
        .row_spacing(COLOUR_SPACING)
        .column_spacing(COLOUR_SPACING)
        .build()
}

fn remove_menu(parent: &gtk::Box) -> gtk::PopoverMenu {
    let menu = gio::Menu::new();
    menu.append(
        Some(REMOVE_LABEL),
        Some(&format!("{ACTION_GROUP}.{REMOVE_ACTION}")),
    );
    let popover = gtk::PopoverMenu::from_model(Some(&menu));
    popover.set_has_arrow(true);
    popover.set_parent(parent);
    popover
}

impl PaletteEditor {
    pub(super) fn show_chosen(&self, snapshot: &Snapshot) {
        let column = &self.column;
        let colour = snapshot.chosen_colour.unwrap_or(NO_COLOUR);
        column.chosen_swatch.set_colour(colour);
        column.chosen_hint.set_label(chosen_hint(&snapshot.edit));
        column
            .pin
            .set_sensitive(pin_button_is_enabled(&snapshot.edit));
    }

    pub(super) fn show_colours(self: &Rc<Self>, snapshot: &Snapshot) {
        let paints = (snapshot.paints.clone(), snapshot.paint_tooltips.clone());
        if self.shown.paints.borrow().as_ref() != Some(&paints) {
            self.rebuild_paints(snapshot);
            self.shown.paints.replace(Some(paints));
        }
        if self.shown.my_colours.borrow().as_ref() != Some(&snapshot.my_colours) {
            self.rebuild_my_colours(&snapshot.my_colours);
        }
        let markers = shows_circle_markers(snapshot.edit.mode);
        let mark = |cell: &ColourCell, colour: ColourRef| {
            cell.swatch
                .set_selected(snapshot.highlighted == Some(colour));
            cell.marker
                .set_visible(markers && snapshot.on_circle.contains(&colour));
        };
        for (slot, cell) in self.shown.paint_cells.borrow().iter().enumerate() {
            mark(cell, ColourRef::Paint(slot));
        }
        for (index, cell) in self.shown.my_cells.borrow().iter().enumerate() {
            mark(cell, ColourRef::Mine(index));
        }
    }

    fn rebuild_paints(self: &Rc<Self>, snapshot: &Snapshot) {
        let grid = &self.column.paints;
        grid.remove_all();
        let mut cells = Vec::with_capacity(snapshot.paints.len());
        for (slot, colour) in snapshot.paints.iter().enumerate() {
            let tooltip = snapshot.paint_tooltips.get(slot).map(String::as_str);
            let (button, cell) = self.colour_button(ColourRef::Paint(slot), *colour, tooltip);
            grid.append(&button);
            cells.push(cell);
        }
        self.shown.paint_cells.replace(cells);
    }

    /// The grid and its title are hidden while nothing is pinned.
    fn rebuild_my_colours(self: &Rc<Self>, colours: &[[u8; 3]]) {
        let grid = &self.column.my_colours;
        grid.remove_all();
        let mut cells = Vec::with_capacity(colours.len());
        for (index, colour) in colours.iter().enumerate() {
            let colour_ref = ColourRef::Mine(index);
            let (button, cell) = self.colour_button(colour_ref, *colour, Some(MY_COLOUR_TOOLTIP));
            button.add_controller(self.removal_click(index));
            grid.append(&button);
            cells.push(cell);
        }
        grid.set_visible(!colours.is_empty());
        self.column
            .my_colours_title
            .set_visible(!colours.is_empty());
        self.shown.my_cells.replace(cells);
        self.shown.my_colours.replace(Some(colours.to_vec()));
    }

    fn colour_button(
        self: &Rc<Self>,
        colour: ColourRef,
        rgb: [u8; 3],
        tooltip: Option<&str>,
    ) -> (gtk::Button, ColourCell) {
        let swatch = Swatch::new(
            rgb,
            shape_of(colour),
            COLOUR_SWATCH_SIZE,
            COLOUR_SWATCH_SIZE,
        );
        let marker = gtk::Box::builder()
            .css_classes([MARKER_CLASS])
            .halign(gtk::Align::End)
            .valign(gtk::Align::End)
            .can_target(false)
            .visible(false)
            .build();
        let overlay = gtk::Overlay::builder().child(&swatch).build();
        overlay.add_overlay(&marker);
        let button = gtk::Button::builder()
            .child(&overlay)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .css_classes(["flat", "pigment-swatch"])
            .build();
        if matches!(colour, ColourRef::Paint(_)) {
            button.add_css_class("circular");
        }
        button.set_tooltip_text(tooltip);
        button.set_cursor_from_name(Some(COLOUR_CURSOR));
        if let Some(name) = tooltip {
            button.update_property(&[gtk::accessible::Property::Label(name)]);
        }
        let editor = Rc::downgrade(self);
        button.connect_clicked(move |button| {
            let modifiers = current_modifiers(button);
            with_editor(&editor, |editor| editor.click_colour(colour, modifiers));
        });
        button.add_controller(drag_source(colour, &swatch));
        (button, ColourCell { swatch, marker })
    }

    /// A right click on the colour `index` of My Colours offers to remove it.
    fn removal_click(self: &Rc<Self>, index: usize) -> gtk::GestureClick {
        let click = gtk::GestureClick::builder()
            .button(gdk::BUTTON_SECONDARY)
            .build();
        let editor = Rc::downgrade(self);
        click.connect_pressed(move |click, _, _, _| {
            let button = click.widget();
            with_editor(&editor, |editor| editor.offer_removal(index, button));
        });
        click
    }

    fn offer_removal(&self, index: usize, button: Option<gtk::Widget>) {
        let column = &self.column;
        let Some(bounds) = button.and_then(|button| button.compute_bounds(&column.root)) else {
            return;
        };
        column.offered.set(Some(index));
        column
            .remove_menu
            .set_pointing_to(Some(&rectangle(&bounds)));
        column.remove_menu.popup();
    }
}

/// Paints are round, My Colours square, as in the side panel.
fn shape_of(colour: ColourRef) -> SwatchShape {
    match colour {
        ColourRef::Paint(_) => SwatchShape::Circle,
        ColourRef::Mine(_) => SwatchShape::RoundedRect,
    }
}

/// Lets `colour` be dragged, as text, onto the circle; the drag shows its swatch.
fn drag_source(colour: ColourRef, swatch: &Swatch) -> gtk::DragSource {
    let source = gtk::DragSource::builder()
        .actions(gdk::DragAction::COPY)
        .build();
    let text = colour_ref_text(colour);
    source.connect_prepare(move |_, _, _| Some(gdk::ContentProvider::for_value(&text.to_value())));
    let icon = swatch.downgrade();
    source.connect_drag_begin(move |source, _| {
        if let Some(swatch) = icon.upgrade() {
            let paintable = gtk::WidgetPaintable::new(Some(&swatch));
            source.set_icon(Some(&paintable), 0, 0);
        }
    });
    source
}

fn rectangle(bounds: &graphene::Rect) -> gdk::Rectangle {
    gdk::Rectangle::new(
        bounds.x() as i32,
        bounds.y() as i32,
        bounds.width().ceil() as i32,
        bounds.height().ceil() as i32,
    )
}
