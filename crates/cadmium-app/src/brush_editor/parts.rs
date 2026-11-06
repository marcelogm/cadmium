//! The Head and Hair groups: a preset dropdown in each group's header, then a slider per
//! control; Hair keeps six of its twelve under "Advanced".

use adw::prelude::*;
use cadmium_studio::{HairPreset, HeadPreset};

use super::controls::{CUSTOM, Control, position_of};
use super::draft::Draft;
use crate::sliders::{SliderRow, slider_row};

/// A page step is this many slider steps.
const STEPS_PER_PAGE: f64 = 10.0;

/// The style class dimming a locked row (`style.css`).
const LOCKED: &str = "locked-control";
/// The editor's sliders are narrower than the lab's, so two-word titles fit on one line.
const SCALE_WIDTH: i32 = 112;

#[derive(Clone)]
pub(super) struct ControlRow {
    pub(super) control: Control,
    pub(super) slider: SliderRow,
}

pub(super) struct PartGroups {
    pub(super) head: adw::PreferencesGroup,
    pub(super) hair: adw::PreferencesGroup,
    pub(super) head_preset: gtk::DropDown,
    pub(super) hair_preset: gtk::DropDown,
    pub(super) rows: Vec<ControlRow>,
}

pub(super) fn part_groups() -> PartGroups {
    let head_preset = preset_dropdown("Head preset", &HeadPreset::ALL.map(HeadPreset::name));
    let hair_preset = preset_dropdown("Hair preset", &HairPreset::ALL.map(HairPreset::name));
    let head = group("Head", &head_preset);
    let hair = group("Hair", &hair_preset);
    let advanced = adw::ExpanderRow::builder().title("Advanced").build();
    let mut rows: Vec<ControlRow> = Control::head().map(control_row).collect();
    rows.iter().for_each(|row| head.add(&row.slider.row));
    let main: Vec<ControlRow> = Control::hair().map(control_row).collect();
    main.iter().for_each(|row| hair.add(&row.slider.row));
    let extra: Vec<ControlRow> = Control::hair_advanced().map(control_row).collect();
    extra
        .iter()
        .for_each(|row| advanced.add_row(&row.slider.row));
    hair.add(&advanced);
    rows.extend(main);
    rows.extend(extra);
    PartGroups {
        head,
        hair,
        head_preset,
        hair_preset,
        rows,
    }
}

impl PartGroups {
    /// Shows `draft`: each dropdown on its preset or Custom, each slider on its value, the
    /// sliders of a part sensitive only while it is Custom.
    pub(super) fn show(&self, draft: &Draft) {
        self.head_preset
            .set_selected(position_of(&HeadPreset::ALL, draft.head_preset()));
        self.hair_preset
            .set_selected(position_of(&HairPreset::ALL, draft.hair_preset()));
        for row in &self.rows {
            row.show(row.control.get(draft.current()));
            row.lock(!draft.is_custom(row.control.part()));
        }
    }
}

impl ControlRow {
    /// Puts the thumb on `value` (unless it is there already) and reads it out, on screen
    /// and to assistive technologies.
    pub(super) fn show(&self, value: f32) {
        let text = self.control.format(value);
        let scale = &self.slider.scale;
        if scale.value() != f64::from(value) {
            scale.set_value(f64::from(value));
        }
        self.slider.value_label.set_label(&text);
        self.slider
            .scale
            .update_property(&[gtk::accessible::Property::ValueText(&text)]);
    }

    /// A locked row is insensitive and dimmed.
    fn lock(&self, locked: bool) {
        let row = &self.slider.row;
        row.set_sensitive(!locked);
        if locked {
            row.add_css_class(LOCKED);
        } else {
            row.remove_css_class(LOCKED);
        }
    }
}

/// The presets' names, then "Custom".
fn preset_dropdown(label: &str, names: &[&str]) -> gtk::DropDown {
    let mut items = names.to_vec();
    items.push(CUSTOM);
    let dropdown = gtk::DropDown::builder()
        .model(&gtk::StringList::new(&items))
        .valign(gtk::Align::Center)
        .build();
    dropdown.update_property(&[gtk::accessible::Property::Label(label)]);
    dropdown
}

fn group(title: &str, preset: &gtk::DropDown) -> adw::PreferencesGroup {
    adw::PreferencesGroup::builder()
        .title(title)
        .header_suffix(preset)
        .build()
}

fn control_row(control: Control) -> ControlRow {
    let [low, high] = control.range().map(f64::from);
    let step = f64::from(control.step());
    let slider = slider_row(control.label(), low, high, step, step * STEPS_PER_PAGE);
    slider.scale.set_width_request(SCALE_WIDTH);
    ControlRow { control, slider }
}
