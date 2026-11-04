use adw::prelude::*;
use cadmium_engine::params::ParamSpec;

const SCALE_WIDTH: i32 = 140;
const VALUE_LABEL_CHARS: i32 = 6;
const PARAM_STEPS: f64 = 100.0;
const PARAM_PAGES: f64 = 10.0;
const WHOLE_NUMBER_PARAMS: [&str; 1] = ["bristle_count"];
const WHOLE_STEP: f64 = 1.0;
const STEP_DECIMALS_SLACK: f64 = 1e-6;

pub(crate) struct SliderRow {
    pub(crate) row: adw::ActionRow,
    pub(crate) scale: gtk::Scale,
    pub(crate) value_label: gtk::Label,
}

fn value_label() -> gtk::Label {
    gtk::Label::builder()
        .width_chars(VALUE_LABEL_CHARS)
        .xalign(1.0)
        .css_classes(["numeric", "dim-label"])
        .build()
}

pub(crate) fn slider_row(title: &str, lower: f64, upper: f64, step: f64, page: f64) -> SliderRow {
    let adjustment = gtk::Adjustment::new(lower, lower, upper, step, page, 0.0);
    let scale = gtk::Scale::builder()
        .adjustment(&adjustment)
        .draw_value(false)
        .width_request(SCALE_WIDTH)
        .valign(gtk::Align::Center)
        .build();
    scale.update_property(&[gtk::accessible::Property::Label(title)]);
    let value_label = value_label();
    let row = adw::ActionRow::builder().title(title).build();
    row.add_suffix(&scale);
    row.add_suffix(&value_label);
    SliderRow {
        row,
        scale,
        value_label,
    }
}

fn is_whole_number_param(spec: &ParamSpec) -> bool {
    WHOLE_NUMBER_PARAMS.contains(&spec.key)
}

fn param_step(spec: &ParamSpec) -> f64 {
    if is_whole_number_param(spec) {
        WHOLE_STEP
    } else {
        f64::from(spec.max - spec.min) / PARAM_STEPS
    }
}

fn decimals_for_step(step: f64) -> usize {
    if step <= 0.0 || !step.is_finite() {
        return 0;
    }
    (-(step.log10() + STEP_DECIMALS_SLACK).floor()).max(0.0) as usize
}

pub(crate) fn format_param(spec: &ParamSpec, value: f32) -> String {
    let decimals = decimals_for_step(param_step(spec));
    format!("{value:.decimals$}")
}

pub(crate) fn param_row(spec: &ParamSpec) -> SliderRow {
    let range = f64::from(spec.max - spec.min);
    let slider = slider_row(
        spec.label,
        f64::from(spec.min),
        f64::from(spec.max),
        param_step(spec),
        range / PARAM_PAGES,
    );
    if is_whole_number_param(spec) {
        slider.scale.set_round_digits(0);
    }
    slider
}

#[cfg(test)]
mod tests;
