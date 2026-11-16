mod cells;
mod clusters;
mod handle;
#[cfg(test)]
mod lab;
mod layout;
mod material;
mod owners;
mod rig;
mod roots;
mod state;
mod tuning;

pub(crate) use cells::CELL_GRID_SIDE;
pub use layout::bristle_reach_px;
pub(super) use layout::disc_texel_count;
pub(crate) use layout::{
    BRISTLE_VERTICES, LayoutRequest, MAX_BRISTLES, ReachRequest, reach_bound_px,
};
pub(crate) use owners::OUTSIDE_TIP;
pub use rig::BRISTLE_SETTLE_MS;
pub(crate) use rig::BristleRig;
pub(crate) use state::{
    BRISTLE_FLARE_SHARE, BRISTLE_STATE_VECTORS, BRISTLE_WORKGROUP_SIZE, BRISTLES_PER_THREAD,
    BristleBuffers, FAN_REST_SPLAY_SHARE, MIN_SEGMENT_SPAN_PX,
};
pub use state::{BristleCell, BristleSnapshot, RELIEF_UM_PER_PX};

#[cfg(test)]
mod tests;
