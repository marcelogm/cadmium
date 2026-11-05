use adw::prelude::*;
use cadmium_engine::pigments::MAX_SLOTS;
use cadmium_engine::stroke::Tool;

use super::presentation::{
    EYEDROPPER_HINT, MAX_SIZE_PX, MIN_SIZE_PX, PanelTool, SHAPES, SIZE_STEP, plate_gap_spec,
    tool_wording,
};
use crate::sliders::{SliderRow, param_row, slider_row};
use crate::swatch::{Swatch, SwatchShape};

const PIGMENT_SPACING: u32 = 4;
const PIGMENT_BOX_MARGIN: i32 = 10;
const PIGMENTS_PER_LINE: u32 = MAX_SLOTS as u32;
const MIN_PIGMENTS_PER_LINE: u32 = 4;
const LOADED_SWATCH_SIZE: (i32, i32) = (140, 20);
const LOADED_SPACING: i32 = 4;
const SIZE_PAGE: f64 = 0.1;
const UNIT_STEP: f64 = 0.01;
const UNIT_PAGE: f64 = 0.1;
const MEDIUM_SLIDER_MAX: f64 = 0.9;
const PLATE_GAP_TITLE: &str = "Slab Thickness";
const RELOAD_EACH_STROKE: &str = "Reload Every Stroke";
const CLEAN_EACH_STROKE: &str = "Clean Every Stroke";

pub(super) struct PigmentsGroup {
    pub(super) group: adw::PreferencesGroup,
    pub(super) flow: gtk::FlowBox,
}

pub(super) struct ToolGroup {
    pub(super) group: adw::PreferencesGroup,
    pub(super) loaded_row: adw::ActionRow,
    pub(super) loaded_swatch: Swatch,
    pub(super) reserve_bar: gtk::LevelBar,
    pub(super) size: SliderRow,
    pub(super) load: SliderRow,
    pub(super) plate_gap: SliderRow,
    pub(super) medium: SliderRow,
    pub(super) shape_row: adw::ComboRow,
    pub(super) reload_row: adw::SwitchRow,
    pub(super) clean_each_row: adw::SwitchRow,
}

pub(super) fn pigments_group() -> PigmentsGroup {
    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .max_children_per_line(PIGMENTS_PER_LINE)
        .min_children_per_line(MIN_PIGMENTS_PER_LINE)
        .row_spacing(PIGMENT_SPACING)
        .column_spacing(PIGMENT_SPACING)
        .margin_top(PIGMENT_BOX_MARGIN)
        .margin_bottom(PIGMENT_BOX_MARGIN)
        .margin_start(PIGMENT_BOX_MARGIN)
        .margin_end(PIGMENT_BOX_MARGIN)
        .build();
    let pigment_card = gtk::Box::builder().css_classes(["card"]).build();
    pigment_card.append(&flow);
    let group = adw::PreferencesGroup::builder()
        .title("Pigments")
        .description(tool_wording(PanelTool::Paint(Tool::Brush)).pigments)
        .build();
    group.add(&pigment_card);
    PigmentsGroup { group, flow }
}

pub(super) fn tool_group() -> ToolGroup {
    let (loaded_row, loaded_swatch, reserve_bar) = loaded_row();
    let size = size_row();
    let load = slider_row("Load", 0.0, 1.0, UNIT_STEP, UNIT_PAGE);
    let plate_gap = plate_gap_row();
    let medium = slider_row("Medium", 0.0, MEDIUM_SLIDER_MAX, UNIT_STEP, UNIT_PAGE);
    let shape_row = shape_row();
    let reload_row = adw::SwitchRow::builder().title(RELOAD_EACH_STROKE).build();
    let clean_each_row = adw::SwitchRow::builder().title(CLEAN_EACH_STROKE).build();
    let group = adw::PreferencesGroup::builder().title("Brush").build();
    group.add(&loaded_row);
    group.add(&size.row);
    group.add(&load.row);
    group.add(&plate_gap.row);
    group.add(&medium.row);
    group.add(&shape_row);
    group.add(&reload_row);
    group.add(&clean_each_row);
    ToolGroup {
        group,
        loaded_row,
        loaded_swatch,
        reserve_bar,
        size,
        load,
        plate_gap,
        medium,
        shape_row,
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
        tool_wording(PanelTool::Paint(Tool::Brush)).size_title,
        MIN_SIZE_PX.ln(),
        MAX_SIZE_PX.ln(),
        SIZE_STEP,
        SIZE_PAGE,
    )
}

fn plate_gap_row() -> SliderRow {
    let plate_gap = param_row(plate_gap_spec());
    plate_gap.row.set_title(PLATE_GAP_TITLE);
    plate_gap
        .scale
        .update_property(&[gtk::accessible::Property::Label(PLATE_GAP_TITLE)]);
    plate_gap
}

fn shape_row() -> adw::ComboRow {
    adw::ComboRow::builder()
        .title("Shape")
        .model(&gtk::StringList::new(&SHAPES.map(|(_, label)| label)))
        .build()
}
