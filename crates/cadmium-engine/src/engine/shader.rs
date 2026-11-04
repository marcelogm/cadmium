use super::{
    BLADE_CELL_GROUP_SIZE, BLADE_DELTA_CHANNELS, BLADE_SPILL_BAND_PX, BLADE_SPILL_VALUES,
    DELTA_SCALE, DRY_WORKGROUP_SIZE, PIGMENT_CHANNELS, TIP_CHANNELS, TIP_HALF, TIP_SIZE,
    VOLUME_CHANNEL,
};
use crate::canvas::TILE_SIZE;
use crate::tool::TOOL_MODE_KNIFE;

const TIP_SUM_INVOCATIONS: u32 = 256;
const PICK_INVOCATIONS: u32 = 256;
const MIN_TOP_LAYER_UM: f32 = 0.01;
const BLADE_DELTA_SCALE: f32 = 4096.0;

pub(super) const DAB_SNIPPETS: [&str; 3] = [
    include_str!("../shaders/brush_tip.wgsl"),
    include_str!("../shaders/nearest_half.wgsl"),
    include_str!("../shaders/footprint.wgsl"),
];
pub(super) const EYEDROPPER_SNIPPETS: [&str; 2] = [
    include_str!("../shaders/brush_tip.wgsl"),
    include_str!("../shaders/slot_lane.wgsl"),
];
pub(super) const DRY_SNIPPETS: [&str; 4] = [
    include_str!("../shaders/pigment_slots.wgsl"),
    include_str!("../shaders/slot_lane.wgsl"),
    include_str!("../shaders/kubelka_munk.wgsl"),
    include_str!("../shaders/nearest_half.wgsl"),
];

pub(crate) fn shader_source(snippets: &[&str], body: &str) -> String {
    let mut source = format!(
        "const TIP_SIZE: u32 = {TIP_SIZE}u;\n\
         const TIP_HALF: i32 = {TIP_HALF};\n\
         const TIP_CHANNELS: u32 = {TIP_CHANNELS}u;\n\
         const PIGMENT_CHANNELS: u32 = {PIGMENT_CHANNELS}u;\n\
         const VOLUME_CHANNEL: u32 = {VOLUME_CHANNEL}u;\n\
         const DELTA_SCALE: f32 = {DELTA_SCALE:?};\n\
         const TOOL_MODE_KNIFE: u32 = {TOOL_MODE_KNIFE}u;\n\
         const TILE_SIZE: u32 = {TILE_SIZE}u;\n\
         const DRY_WORKGROUP_SIZE: u32 = {DRY_WORKGROUP_SIZE}u;\n\
         const PICK_INVOCATIONS: u32 = {PICK_INVOCATIONS}u;\n\
         const TIP_SUM_INVOCATIONS: u32 = {TIP_SUM_INVOCATIONS}u;\n\
         const MIN_TOP_LAYER_UM: f32 = {MIN_TOP_LAYER_UM:?};\n\
         const BLADE_DELTA_SCALE: f32 = {BLADE_DELTA_SCALE:?};\n\
         const BLADE_DELTA_CHANNELS: u32 = {BLADE_DELTA_CHANNELS}u;\n\
         const BLADE_SPILL_VALUES: u32 = {BLADE_SPILL_VALUES}u;\n\
         const BLADE_SPILL_BAND_PX: f32 = {BLADE_SPILL_BAND_PX:?};\n\
         const BLADE_CELL_GROUP_SIZE: u32 = {BLADE_CELL_GROUP_SIZE}u;\n"
    );
    for snippet in snippets {
        source.push_str(snippet);
        source.push('\n');
    }
    source.push_str(body);
    source
}
