use adw::prelude::*;
use cadmium_engine::palette::MAX_DARKEN;
use cadmium_engine::pigments::MAX_SLOTS;
use cadmium_engine::stroke::Tool;

use super::presentation::{
    EYEDROPPER_HINT, MAX_SIZE_PX, MIN_SIZE_PX, SIZE_STEP, hardness_grade, palette_view_name,
    palette_views, plate_gap_spec,
};
use crate::colour_wheel::{COLOUR_CIRCLE, ColourWheel};
use crate::sliders::{SliderRow, param_row, slider_row};
use crate::swatch::{Swatch, SwatchShape};
use crate::tools::panel::ToolPanel;

const PALETTE_TITLE: &str = "Palette";
const PAINTS_TITLE: &str = "Paints";
const MY_COLOURS_TITLE: &str = "My Colours";
const DARKEN_TITLE: &str = "Darken";
const EDIT_PALETTE_LABEL: &str = "Edit Palette…";
const DARKEN_TOOLTIP: &str = "Adds black to every colour of the circle";
const PALETTE_SPACING: i32 = 12;
const PALETTE_CARD_MARGIN: i32 = 10;
const PALETTE_CARD_SPACING: i32 = 8;
const WHEEL_SIZE_PX: i32 = 230;
const DARKEN_SPACING: i32 = 12;
const PIGMENT_SPACING: u32 = 4;
const PIGMENTS_PER_LINE: u32 = MAX_SLOTS as u32;
const MIN_PIGMENTS_PER_LINE: u32 = 4;
const LOADED_SWATCH_SIZE: (i32, i32) = (140, 20);
const LOADED_SPACING: i32 = 4;
const SIZE_PAGE: f64 = 0.1;
const UNIT_STEP: f64 = 0.01;
const UNIT_PAGE: f64 = 0.1;
const MEDIUM_SLIDER_MAX: f64 = 0.9;
const PLATE_GAP_TITLE: &str = "Slab Thickness";
const HARDNESS_TITLE: &str = "Hardness";
const HARDNESS_SUBTITLE: &str = "Soft B leads are dark; hard H leads stay light and grainy";
/// The softest, the middle (HB) and the hardest lead, marked with the grades the value shows.
const HARDNESS_MARKS: [f32; 3] = [0.0, 0.5, 1.0];
const RELOAD_EACH_STROKE: &str = "Reload Every Stroke";
const CLEAN_EACH_STROKE: &str = "Clean Every Stroke";

pub(super) struct PaletteGroup {
    pub(super) group: adw::PreferencesGroup,
    pub(super) view_toggles: adw::ToggleGroup,
    pub(super) stack: gtk::Stack,
    pub(super) paints: gtk::FlowBox,
    pub(super) my_colours_title: gtk::Label,
    pub(super) my_colours: gtk::FlowBox,
    pub(super) wheel: ColourWheel,
    pub(super) darken: gtk::Scale,
}

pub(super) struct ToolGroup {
    pub(super) group: adw::PreferencesGroup,
    pub(super) loaded_row: adw::ActionRow,
    pub(super) loaded_swatch: Swatch,
    pub(super) reserve_bar: gtk::LevelBar,
    pub(super) size: SliderRow,
    pub(super) hardness: SliderRow,
    pub(super) load: SliderRow,
    pub(super) plate_gap: SliderRow,
    pub(super) medium: SliderRow,
    pub(super) reload_row: adw::SwitchRow,
    pub(super) clean_each_row: adw::SwitchRow,
}

pub(super) fn palette_group() -> PaletteGroup {
    let view_toggles = adw::ToggleGroup::builder().build();
    let stack = gtk::Stack::builder().vhomogeneous(false).build();
    let paints = colour_grid();
    let my_colours_title = grid_title(MY_COLOURS_TITLE);
    let my_colours = colour_grid();
    let colours = palette_card(&[
        grid_title(PAINTS_TITLE).upcast_ref(),
        paints.upcast_ref(),
        my_colours_title.upcast_ref(),
        my_colours.upcast_ref(),
    ]);
    let wheel = ColourWheel::new(WHEEL_SIZE_PX);
    let (darken_line, darken) = darken_line();
    let circle = palette_card(&[wheel.widget().upcast_ref(), darken_line.upcast_ref()]);
    for ((view, label), page) in palette_views(COLOUR_CIRCLE).iter().zip([colours, circle]) {
        let name = palette_view_name(*view);
        view_toggles.add(adw::Toggle::builder().name(name).label(*label).build());
        stack.add_named(&page, Some(name));
    }
    // A single face needs no switch.
    view_toggles.set_visible(COLOUR_CIRCLE);
    PaletteGroup {
        group: palette_preferences_group(&view_toggles, &stack),
        view_toggles,
        stack,
        paints,
        my_colours_title,
        my_colours,
        wheel,
        darken,
    }
}

fn palette_preferences_group(
    view_toggles: &adw::ToggleGroup,
    stack: &gtk::Stack,
) -> adw::PreferencesGroup {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(PALETTE_SPACING)
        .build();
    content.append(view_toggles);
    content.append(stack);
    content.append(&edit_palette_button());
    let group = adw::PreferencesGroup::builder()
        .title(PALETTE_TITLE)
        .description(Tool::Brush.wording().pigments)
        .build();
    group.add(&content);
    group
}

fn edit_palette_button() -> gtk::Button {
    gtk::Button::builder()
        .label(EDIT_PALETTE_LABEL)
        .action_name("win.edit-palette")
        .halign(gtk::Align::Center)
        .css_classes(["pill"])
        .build()
}

fn palette_card(children: &[&gtk::Widget]) -> gtk::Box {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(PALETTE_CARD_SPACING)
        .margin_top(PALETTE_CARD_MARGIN)
        .margin_bottom(PALETTE_CARD_MARGIN)
        .margin_start(PALETTE_CARD_MARGIN)
        .margin_end(PALETTE_CARD_MARGIN)
        .hexpand(true)
        .build();
    for child in children {
        content.append(*child);
    }
    let card = gtk::Box::builder().css_classes(["card"]).build();
    card.append(&content);
    card
}

fn colour_grid() -> gtk::FlowBox {
    gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .max_children_per_line(PIGMENTS_PER_LINE)
        .min_children_per_line(MIN_PIGMENTS_PER_LINE)
        .row_spacing(PIGMENT_SPACING)
        .column_spacing(PIGMENT_SPACING)
        .build()
}

fn grid_title(title: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(title)
        .xalign(0.0)
        .css_classes(["caption-heading", "dim-label"])
        .build()
}

fn darken_line() -> (gtk::Box, gtk::Scale) {
    let adjustment =
        gtk::Adjustment::new(0.0, 0.0, f64::from(MAX_DARKEN), UNIT_STEP, UNIT_PAGE, 0.0);
    let darken = gtk::Scale::builder()
        .adjustment(&adjustment)
        .draw_value(false)
        .hexpand(true)
        .tooltip_text(DARKEN_TOOLTIP)
        .build();
    darken.update_property(&[gtk::accessible::Property::Label(DARKEN_TITLE)]);
    let line = gtk::Box::builder().spacing(DARKEN_SPACING).build();
    line.append(&gtk::Label::new(Some(DARKEN_TITLE)));
    line.append(&darken);
    (line, darken)
}

pub(super) fn tool_group() -> ToolGroup {
    let (loaded_row, loaded_swatch, reserve_bar) = loaded_row();
    let size = size_row();
    let hardness = hardness_row();
    let load = slider_row("Load", 0.0, 1.0, UNIT_STEP, UNIT_PAGE);
    let plate_gap = plate_gap_row();
    let medium = slider_row("Medium", 0.0, MEDIUM_SLIDER_MAX, UNIT_STEP, UNIT_PAGE);
    let reload_row = adw::SwitchRow::builder().title(RELOAD_EACH_STROKE).build();
    let clean_each_row = adw::SwitchRow::builder().title(CLEAN_EACH_STROKE).build();
    let group = adw::PreferencesGroup::builder().title("Brush").build();
    group.add(&loaded_row);
    group.add(&size.row);
    group.add(&hardness.row);
    group.add(&load.row);
    group.add(&plate_gap.row);
    group.add(&medium.row);
    group.add(&reload_row);
    group.add(&clean_each_row);
    ToolGroup {
        group,
        loaded_row,
        loaded_swatch,
        reserve_bar,
        size,
        hardness,
        load,
        plate_gap,
        medium,
        reload_row,
        clean_each_row,
    }
}

pub(super) fn clean_button() -> gtk::Button {
    gtk::Button::builder()
        .action_name("win.clean-brush")
        .halign(gtk::Align::Center)
        .css_classes(["pill"])
        .build()
}

pub(super) fn hint_label() -> gtk::Label {
    gtk::Label::builder()
        .label(EYEDROPPER_HINT)
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build()
}

fn loaded_row() -> (adw::ActionRow, Swatch, gtk::LevelBar) {
    let loaded_swatch = Swatch::new(
        [0, 0, 0],
        SwatchShape::RoundedRect,
        LOADED_SWATCH_SIZE.0,
        LOADED_SWATCH_SIZE.1,
    );
    let reserve_bar = gtk::LevelBar::builder()
        .min_value(0.0)
        .max_value(1.0)
        .width_request(LOADED_SWATCH_SIZE.0)
        .build();
    let loaded = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(LOADED_SPACING)
        .valign(gtk::Align::Center)
        .build();
    loaded.append(&loaded_swatch);
    loaded.append(&reserve_bar);
    let loaded_row = adw::ActionRow::builder().subtitle_lines(2).build();
    loaded_row.add_suffix(&loaded);
    (loaded_row, loaded_swatch, reserve_bar)
}

fn size_row() -> SliderRow {
    slider_row(
        Tool::Brush.wording().size_title,
        MIN_SIZE_PX.ln(),
        MAX_SIZE_PX.ln(),
        SIZE_STEP,
        SIZE_PAGE,
    )
}

fn hardness_row() -> SliderRow {
    let hardness = slider_row(HARDNESS_TITLE, 0.0, 1.0, UNIT_STEP, UNIT_PAGE);
    hardness.row.set_subtitle(HARDNESS_SUBTITLE);
    hardness.scale.set_tooltip_text(Some(HARDNESS_SUBTITLE));
    for mark in HARDNESS_MARKS {
        hardness.scale.add_mark(
            f64::from(mark),
            gtk::PositionType::Bottom,
            Some(hardness_grade(mark)),
        );
    }
    hardness
}

fn plate_gap_row() -> SliderRow {
    let plate_gap = param_row(plate_gap_spec());
    plate_gap.row.set_title(PLATE_GAP_TITLE);
    plate_gap
        .scale
        .update_property(&[gtk::accessible::Property::Label(PLATE_GAP_TITLE)]);
    plate_gap
}
