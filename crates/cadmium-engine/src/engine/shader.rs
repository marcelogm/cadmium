use super::bristles::{
    BRISTLE_FLARE_SHARE, BRISTLE_SOLVER_ITERATIONS, BRISTLE_STATE_VECTORS,
    BRISTLE_TIP_STIFFNESS_SHARE, BRISTLE_VERTICES, BRISTLE_WORKGROUP_SIZE, BRISTLES_PER_THREAD,
    CELL_GRID_SIDE, CLUSTER_CENTROIDS, CLUSTER_SPAN_WORDS, FAN_REST_SPLAY_SHARE, LANE_SAMPLES,
    MAX_BRISTLES, MAX_CLUSTERS, MAX_PLASTIC_LEAN, MIN_LEAN_DROP, MIN_SEGMENT_SPAN_PX, OUTSIDE_TIP,
    PLASTIC_BEND_THRESHOLD_RAD, RELIEF_UM_PER_PX, TOUCH_TOLERANCE_PX,
};
use super::pass::bristle_contact::{
    BRISTLE_SEGMENTS, CONTACT_ALONG_HIGH, CONTACT_ALONG_LOW, CONTACT_BIN_BITS, CONTACT_BOX_FIRST,
    CONTACT_BOX_LAST, CONTACT_BRISTLE_BITS, CONTACT_EXTENT_HIGH, CONTACT_EXTENT_LOW,
    CONTACT_EXTENT_SCALE, CONTACT_EXTENT_WORDS, CONTACT_LEVELS, CONTACT_WORKGROUP_SIZE,
    SWEPT_VECTORS,
};
use super::{
    BLADE_CELL_GROUP_SIZE, BLADE_DELTA_CHANNELS, BLADE_SPILL_BAND_PX, BLADE_SPILL_VALUES,
    DELTA_SCALE, DRY_WORKGROUP_SIZE, MAX_TIP_THICKNESS_UM, PIGMENT_CHANNELS, TIP_CHANNELS,
    TIP_HALF, TIP_SIZE, VOLUME_CHANNEL,
};
use crate::canvas::{TILE_CHANGE_HEADER_WORDS, TILE_SIZE};
use crate::compute::WORKGROUP_SIZE;
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
pub(super) const ADVECT_SNIPPETS: [&str; 4] = [
    include_str!("../shaders/brush_tip.wgsl"),
    include_str!("../shaders/nearest_half.wgsl"),
    include_str!("../shaders/footprint.wgsl"),
    include_str!("../shaders/tile_changes.wgsl"),
];
pub(super) const TOUCHED_TILE_SNIPPETS: [&str; 3] = [
    include_str!("../shaders/brush_tip.wgsl"),
    include_str!("../shaders/contact_mask.wgsl"),
    include_str!("../shaders/tile_changes.wgsl"),
];
pub(super) const EXCHANGE_SNIPPETS: [&str; 6] = [
    include_str!("../shaders/brush_tip.wgsl"),
    include_str!("../shaders/nearest_half.wgsl"),
    include_str!("../shaders/footprint.wgsl"),
    include_str!("../shaders/contact_mask.wgsl"),
    include_str!("../shaders/cell_grid.wgsl"),
    include_str!("../shaders/contact_reach.wgsl"),
];
pub(super) const SPREAD_SNIPPETS: [&str; 6] = [
    include_str!("../shaders/brush_tip.wgsl"),
    include_str!("../shaders/nearest_half.wgsl"),
    include_str!("../shaders/footprint.wgsl"),
    include_str!("../shaders/contact_mask.wgsl"),
    include_str!("../shaders/contact_press.wgsl"),
    include_str!("../shaders/contact_reach.wgsl"),
];
pub(super) const CONTACT_SNIPPETS: [&str; 6] = [
    include_str!("../shaders/brush_tip.wgsl"),
    include_str!("../shaders/bristle_rest.wgsl"),
    include_str!("../shaders/bristle_frame.wgsl"),
    include_str!("../shaders/bristle.wgsl"),
    include_str!("../shaders/contact_mask.wgsl"),
    include_str!("../shaders/cell_grid.wgsl"),
];
pub(super) const BRISTLE_SNIPPETS: [&str; 10] = [
    include_str!("../shaders/brush_tip.wgsl"),
    include_str!("../shaders/bristle_rest.wgsl"),
    include_str!("../shaders/bristle_frame.wgsl"),
    include_str!("../shaders/bristle.wgsl"),
    include_str!("../shaders/bristle_tuning.wgsl"),
    include_str!("../shaders/bristle_relief.wgsl"),
    include_str!("../shaders/bristle_friction.wgsl"),
    include_str!("../shaders/bristle_memory.wgsl"),
    include_str!("../shaders/bristle_point.wgsl"),
    include_str!("../shaders/bristle_clump.wgsl"),
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

fn bristle_constants() -> String {
    format!(
        "const MAX_BRISTLES: u32 = {MAX_BRISTLES}u;\n\
         const BRISTLE_VERTICES: u32 = {BRISTLE_VERTICES}u;\n\
         const BRISTLE_WORKGROUP_SIZE: u32 = {BRISTLE_WORKGROUP_SIZE}u;\n\
         const BRISTLES_PER_THREAD: u32 = {BRISTLES_PER_THREAD}u;\n\
         const BRISTLE_STATE_VECTORS: u32 = {BRISTLE_STATE_VECTORS}u;\n\
         const BRISTLE_SOLVER_ITERATIONS: u32 = {BRISTLE_SOLVER_ITERATIONS}u;\n\
         const BRISTLE_TIP_STIFFNESS_SHARE: f32 = {BRISTLE_TIP_STIFFNESS_SHARE:?};\n\
         const BRISTLE_FLARE_SHARE: f32 = {BRISTLE_FLARE_SHARE:?};\n\
         const FAN_REST_SPLAY_SHARE: f32 = {FAN_REST_SPLAY_SHARE:?};\n\
         const MIN_SEGMENT_SPAN_PX: f32 = {MIN_SEGMENT_SPAN_PX:?};\n\
         const TOUCH_TOLERANCE_PX: f32 = {TOUCH_TOLERANCE_PX:?};\n\
         const RELIEF_UM_PER_PX: f32 = {RELIEF_UM_PER_PX:?};\n\
         const PLASTIC_BEND_THRESHOLD_RAD: f32 = {PLASTIC_BEND_THRESHOLD_RAD:?};\n\
         const MAX_PLASTIC_LEAN: f32 = {MAX_PLASTIC_LEAN:?};\n\
         const MIN_LEAN_DROP: f32 = {MIN_LEAN_DROP:?};\n\
         const LANE_SAMPLES: u32 = {LANE_SAMPLES}u;\n\
         const MAX_CLUSTERS: u32 = {MAX_CLUSTERS}u;\n\
         const CLUSTER_SPAN_WORDS: u32 = {CLUSTER_SPAN_WORDS}u;\n\
         const CLUSTER_CENTROIDS: u32 = {CLUSTER_CENTROIDS}u;\n\
         const MAX_TIP_THICKNESS_UM: f32 = {MAX_TIP_THICKNESS_UM:?};\n\
         const OUTSIDE_TIP: u32 = {OUTSIDE_TIP}u;\n\
         const CONTACT_WORKGROUP_SIZE: u32 = {CONTACT_WORKGROUP_SIZE}u;\n\
         const CELL_GRID_SIDE: u32 = {CELL_GRID_SIDE}u;\n\
         const CONTACT_EXTENT_SCALE: f32 = {CONTACT_EXTENT_SCALE:?};\n\
         const CONTACT_EXTENT_LOW: u32 = {CONTACT_EXTENT_LOW}u;\n\
         const CONTACT_EXTENT_HIGH: u32 = {CONTACT_EXTENT_HIGH}u;\n\
         const CONTACT_ALONG_LOW: u32 = {CONTACT_ALONG_LOW}u;\n\
         const CONTACT_ALONG_HIGH: u32 = {CONTACT_ALONG_HIGH}u;\n\
         const CONTACT_EXTENT_WORDS: u32 = {CONTACT_EXTENT_WORDS}u;\n\
         const CONTACT_BOX_FIRST: u32 = {CONTACT_BOX_FIRST}u;\n\
         const CONTACT_BOX_LAST: u32 = {CONTACT_BOX_LAST}u;\n\
         const BRISTLE_SEGMENTS: u32 = {BRISTLE_SEGMENTS}u;\n\
         const SWEPT_VECTORS: u32 = {SWEPT_VECTORS}u;\n\
         const CONTACT_BRISTLE_BITS: u32 = {CONTACT_BRISTLE_BITS}u;\n\
         const CONTACT_BIN_BITS: u32 = {CONTACT_BIN_BITS}u;\n\
         const CONTACT_LEVELS: u32 = {CONTACT_LEVELS}u;\n"
    )
}

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
         const TILE_CHANGE_HEADER_WORDS: u32 = {TILE_CHANGE_HEADER_WORDS}u;\n\
         const BLOCK_SIDE_PX: u32 = {WORKGROUP_SIZE}u;\n\
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
    source.push_str(&bristle_constants());
    for snippet in snippets {
        source.push_str(snippet);
        source.push('\n');
    }
    source.push_str(body);
    source
}
