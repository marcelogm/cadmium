mod cells;
mod clusters;
mod handle;
mod layout;
mod owners;
mod rig;
mod roots;
mod state;
mod tuning;

pub(crate) use cells::CELL_GRID_SIDE;
pub(crate) use clusters::{CLUSTER_SPAN_WORDS, MAX_CLUSTERS};
pub use layout::bristle_reach_px;
pub(super) use layout::disc_texel_count;
pub(crate) use layout::{
    BRISTLE_VERTICES, LayoutRequest, MAX_BRISTLES, ReachRequest, reach_bound_px,
};
pub(crate) use owners::OUTSIDE_TIP;
pub use rig::BRISTLE_SETTLE_MS;
pub(crate) use rig::BristleRig;
pub(crate) use state::{
    BRISTLE_FLARE_SHARE, BRISTLE_SOLVER_ITERATIONS, BRISTLE_STATE_VECTORS,
    BRISTLE_TIP_STIFFNESS_SHARE, BRISTLE_WORKGROUP_SIZE, BRISTLES_PER_THREAD, BristleBuffers,
    CLUSTER_CENTROIDS, FAN_REST_SPLAY_SHARE, LANE_SAMPLES, MAX_PLASTIC_LEAN, MIN_LEAN_DROP,
    MIN_SEGMENT_SPAN_PX, PLASTIC_BEND_THRESHOLD_RAD, TOUCH_TOLERANCE_PX,
};
pub use state::{BristleCell, BristleSnapshot, RELIEF_UM_PER_PX};
pub(crate) use tuning::BristleTuning;

#[cfg(test)]
mod tests;
