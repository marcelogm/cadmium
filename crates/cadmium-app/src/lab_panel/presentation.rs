use cadmium_engine::params::ParamGroup;
use cadmium_engine::render::ViewMode;

use crate::state::PenReadout;

const VIEW_COLOR: &str = "color";
const VIEW_HEIGHT: &str = "height";
const VIEW_WETNESS: &str = "wetness";
const VIEW_PIGMENT: &str = "pigment";

pub(super) const VIEW_TOGGLES: [(&str, &str); 4] = [
    (VIEW_COLOR, "Cor"),
    (VIEW_HEIGHT, "Altura"),
    (VIEW_WETNESS, "Umidade"),
    (VIEW_PIGMENT, "Pigmento"),
];

pub(super) const PARAM_GROUPS: [(ParamGroup, &str); 5] = [
    (ParamGroup::Transfer, "Transferência"),
    (ParamGroup::Dynamics, "Dinâmica"),
    (ParamGroup::Tools, "Ferramentas"),
    (ParamGroup::Footprint, "Pegada"),
    (ParamGroup::Render, "Render"),
];

pub(super) fn view_name(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::Color => VIEW_COLOR,
        ViewMode::Height => VIEW_HEIGHT,
        ViewMode::Wetness => VIEW_WETNESS,
        ViewMode::Pigment(_) => VIEW_PIGMENT,
    }
}

pub(super) fn view_from_name(name: &str, pigment_slot: u8) -> Option<ViewMode> {
    match name {
        VIEW_COLOR => Some(ViewMode::Color),
        VIEW_HEIGHT => Some(ViewMode::Height),
        VIEW_WETNESS => Some(ViewMode::Wetness),
        VIEW_PIGMENT => Some(ViewMode::Pigment(pigment_slot)),
        _ => None,
    }
}

pub(super) fn pen_texts(pen: &PenReadout) -> [String; 4] {
    [
        format!("{:.2}", pen.pressure),
        format!("{:.0}° / {:.0}°", pen.tilt_x, pen.tilt_y),
        pen.samples_last_frame.to_string(),
        format!("{:.0} Hz", pen.rate_hz),
    ]
}

#[cfg(test)]
mod tests;
