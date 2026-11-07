use cadmium_engine::brush::{BrushSpec, HairSpec, HeadSpec};
use cadmium_engine::params::{ParamSpec, SimParams};
use cadmium_engine::tool::Holder;
use cadmium_studio::{hair_name, head_name};
use gtk::gdk;

use crate::sliders::format_param;
use crate::state::PigmentClick;
use crate::tools::panel::{CLEAN_LABEL, PanelTool};

const PERCENT: f64 = 100.0;
const MIN_LISTED_SHARE: f32 = 0.01;
pub(super) const MIN_SIZE_PX: f64 = 4.0;
pub(super) const MAX_SIZE_PX: f64 = 500.0;
pub(super) const SIZE_STEP: f64 = 0.01;
pub(super) const PLATE_GAP_KEY: &str = "knife_gap_um";
const PLATE_GAP_UNIT: &str = "µm";
pub(super) const EYEDROPPER_HINT: &str = "Click the canvas to pick up the paint on it. \
The right mouse button, or a pen button set to Pick Colour, picks a colour with any tool.";
/// Pencil grades from the softest lead to the hardest, HB in the middle.
const PENCIL_GRADES: [&str; 9] = ["8B", "6B", "4B", "2B", "HB", "2H", "4H", "6H", "8H"];
const MIDDLE_GRADE: usize = PENCIL_GRADES.len() / 2;
/// What a rack row calls a head or hair that matches no factory one.
const CUSTOM_PART: &str = "Custom";

pub(super) fn tool_hint(tool: PanelTool) -> &'static str {
    tool.handling().unwrap_or(EYEDROPPER_HINT)
}

/// The loaded swatch's colour and its right half's: the palette's mix beside the
/// held paint, or the face's paint alone, since the face never loads from the palette.
pub(super) fn loaded_swatch_colours(
    tool: PanelTool,
    palette: [u8; 3],
    held: [u8; 3],
) -> ([u8; 3], Option<[u8; 3]>) {
    if tool.holder() == Some(Holder::Face) {
        (held, None)
    } else {
        (palette, Some(held))
    }
}

pub(super) fn pile_is_highlighted(
    tool: PanelTool,
    slot: usize,
    tube_slot: usize,
    recipe: &[(usize, f32)],
) -> bool {
    match tool {
        PanelTool::Tube => slot == tube_slot,
        PanelTool::Paint(_) => recipe
            .iter()
            .any(|(chosen, parts)| *chosen == slot && *parts > 0.0),
    }
}

pub(super) fn tool_group_description<'a>(
    tool: PanelTool,
    slot_names: &[&'a str],
    tube_slot: usize,
) -> Option<&'a str> {
    match tool {
        PanelTool::Tube => slot_names.get(tube_slot).copied(),
        PanelTool::Paint(_) => None,
    }
}

pub(super) fn describe_recipe(names: &[&str], recipe: &[(usize, f32)]) -> String {
    let mut shares: Vec<(&str, f32)> = recipe
        .iter()
        .filter(|(_, parts)| *parts > 0.0)
        .filter_map(|(slot, parts)| names.get(*slot).map(|name| (*name, *parts)))
        .collect();
    let total: f32 = shares.iter().map(|(_, parts)| parts).sum();
    if total <= 0.0 {
        return CLEAN_LABEL.to_string();
    }
    if let [(name, _)] = shares.as_slice() {
        return name.to_string();
    }
    shares.sort_by(|a, b| b.1.total_cmp(&a.1));
    shares
        .iter()
        .map(|(name, parts)| (name, parts / total))
        .filter(|(_, share)| *share >= MIN_LISTED_SHARE)
        .map(|(name, share)| format!("{name} {:.0}%", f64::from(share) * PERCENT))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The pencil grade a hardness between 0 (softest) and 1 (hardest) reads as.
pub(super) fn hardness_grade(hardness: f32) -> &'static str {
    if hardness.is_nan() {
        return PENCIL_GRADES[MIDDLE_GRADE];
    }
    let last = (PENCIL_GRADES.len() - 1) as f32;
    PENCIL_GRADES[(hardness.clamp(0.0, 1.0) * last).round() as usize]
}

pub(super) fn plate_gap_spec() -> &'static ParamSpec {
    SimParams::specs()
        .iter()
        .find(|spec| spec.key == PLATE_GAP_KEY)
        .expect("the knife plate gap is a simulation parameter")
}

pub(super) fn plate_gap_text(gap_um: f32) -> String {
    format!(
        "{} {PLATE_GAP_UNIT}",
        format_param(plate_gap_spec(), gap_um)
    )
}

pub(super) fn pigment_click(modifiers: gdk::ModifierType) -> PigmentClick {
    if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
        PigmentClick::Mix
    } else if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
        PigmentClick::CleanAndLoad
    } else {
        PigmentClick::Choose
    }
}

pub(super) fn slider_needs_sync(slider_value: f64, diameter_px: f32) -> bool {
    slider_to_size(slider_value) != diameter_px
}

pub(super) fn size_to_slider(diameter_px: f32) -> f64 {
    f64::from(diameter_px).clamp(MIN_SIZE_PX, MAX_SIZE_PX).ln()
}

pub(super) fn slider_to_size(value: f64) -> f32 {
    value.exp().clamp(MIN_SIZE_PX, MAX_SIZE_PX).round() as f32
}

pub(super) fn percent_text(value: f64) -> String {
    format!("{:.0}%", value * PERCENT)
}

/// What a rack row says about its brush.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RackEntry {
    pub(super) title: String,
    pub(super) subtitle: String,
    /// The brush's full name, for the row's tooltip.
    pub(super) name: String,
    /// What the row's tip icon draws.
    pub(super) head: HeadSpec,
    pub(super) hair: HairSpec,
}

/// A factory brush reads as its head, then its hair; a user brush as its name, then
/// "head · hair", where a head or hair that is no factory one reads "Custom".
pub(super) fn rack_entry(brush: &BrushSpec, is_user: bool) -> RackEntry {
    let head = head_name(&brush.head).unwrap_or(CUSTOM_PART);
    let hair = hair_name(&brush.hair).unwrap_or(CUSTOM_PART);
    let (title, subtitle) = if is_user {
        (brush.name.clone(), format!("{head} · {hair}"))
    } else {
        (head.to_string(), hair.to_string())
    };
    RackEntry {
        title,
        subtitle,
        name: brush.name.clone(),
        head: brush.head,
        hair: brush.hair,
    }
}

#[cfg(test)]
mod tests;
