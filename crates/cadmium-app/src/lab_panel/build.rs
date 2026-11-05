use adw::prelude::*;
use cadmium_engine::params::SimParams;

use super::ParamControl;
use super::presentation::{PARAM_GROUPS, VIEW_TOGGLES};
use super::snapshot::SlotInfo;
use crate::sliders::param_row;
use crate::swatch::{Swatch, SwatchShape};

const VIEW_SPACING: i32 = 12;
const PIGMENT_SWATCH_SIZE: (i32, i32) = (28, 20);
const PIGMENT_SWATCH_SPACING: i32 = 4;
const PROVISIONAL_NOTICE: &str = "Spectral data is provisional (hand-tuned, not measured).";

pub(super) struct ViewControls {
    pub(super) group: adw::PreferencesGroup,
    pub(super) toggles: adw::ToggleGroup,
    pub(super) pigment_row: adw::ActionRow,
    pub(super) pigment_dropdown: gtk::DropDown,
}

pub(super) fn view_controls() -> ViewControls {
    let view_group = adw::ToggleGroup::builder().build();
    for (name, label) in VIEW_TOGGLES {
        view_group.add(adw::Toggle::builder().name(name).label(label).build());
    }
    let pigment_dropdown = gtk::DropDown::builder().valign(gtk::Align::Center).build();
    let pigment_row = adw::ActionRow::builder().title("Pigment").build();
    pigment_row.add_suffix(&pigment_dropdown);
    let pigment_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    pigment_list.append(&pigment_row);
    let view_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(VIEW_SPACING)
        .build();
    view_box.append(&view_group);
    view_box.append(&pigment_list);
    let view = adw::PreferencesGroup::builder().title("View").build();
    view.add(&view_box);
    ViewControls {
        group: view,
        toggles: view_group,
        pigment_row,
        pigment_dropdown,
    }
}

pub(super) fn param_groups(root: &gtk::Box) -> Vec<ParamControl> {
    let mut params = Vec::new();
    for (group, title) in PARAM_GROUPS {
        let preferences = adw::PreferencesGroup::builder().title(title).build();
        for spec in SimParams::specs().iter().filter(|spec| spec.group == group) {
            let slider = param_row(spec);
            preferences.add(&slider.row);
            params.push(ParamControl {
                spec,
                scale: slider.scale,
                label: slider.value_label,
            });
        }
        root.append(&preferences);
    }
    params
}

pub(super) fn reset_button() -> gtk::Button {
    gtk::Button::builder()
        .label("Restore Defaults")
        .halign(gtk::Align::Center)
        .css_classes(["pill"])
        .build()
}

pub(super) fn pen_group() -> (adw::PreferencesGroup, [gtk::Label; 4]) {
    let pen = adw::PreferencesGroup::builder().title("Pen").build();
    let pen_labels = ["Pressure", "Tilt", "Samples per Frame", "Rate"].map(|title| {
        let label = gtk::Label::builder()
            .css_classes(["numeric", "dim-label"])
            .build();
        let row = adw::ActionRow::builder().title(title).build();
        row.add_suffix(&label);
        pen.add(&row);
        label
    });
    (pen, pen_labels)
}

pub(super) fn pigments_group() -> adw::PreferencesGroup {
    adw::PreferencesGroup::builder()
        .title("Pigments")
        .description("Masstone, 1:1 tint with white, glaze over white and black")
        .build()
}

pub(super) fn provisional_notice() -> gtk::Label {
    gtk::Label::builder()
        .label(PROVISIONAL_NOTICE)
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label", "caption"])
        .build()
}

pub(super) fn slot_row(info: &SlotInfo) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(info.name)
        .subtitle(&info.subtitle)
        .subtitle_lines(2)
        .build();
    let swatches = gtk::Box::builder()
        .spacing(PIGMENT_SWATCH_SPACING)
        .valign(gtk::Align::Center)
        .build();
    let glaze = lab_swatch(info.glaze_over_white, "Glaze over white and black");
    glaze.set_right_half_colour(Some(info.glaze_over_black));
    swatches.append(&lab_swatch(info.masstone, "Masstone"));
    swatches.append(&lab_swatch(info.tint, "1:1 tint with white"));
    swatches.append(&glaze);
    row.add_suffix(&swatches);
    row
}

fn lab_swatch(colour: [u8; 3], tooltip: &str) -> Swatch {
    let swatch = Swatch::new(
        colour,
        SwatchShape::RoundedRect,
        PIGMENT_SWATCH_SIZE.0,
        PIGMENT_SWATCH_SIZE.1,
    );
    swatch.set_tooltip_text(Some(tooltip));
    swatch
}
