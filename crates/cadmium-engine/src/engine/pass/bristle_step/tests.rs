use super::*;
use crate::engine::bristles::BRISTLE_VERTICES;

#[test]
fn each_strand_solve_names_its_own_entry_point() {
    assert_eq!(StrandSolve::Grouped.entry_point(), "solve");
    assert_eq!(StrandSolve::Serial.entry_point(), "solve_reference");
}

#[test]
fn the_serial_solve_covers_every_strand_one_per_thread() {
    let [x, y] = StrandSolve::Serial.workgroups();
    assert_eq!(y, 1);
    assert_eq!(x * BRISTLE_SOLVE_WORKGROUP_SIZE, MAX_BRISTLES as u32);
}

#[test]
fn the_grouped_solve_covers_every_strand_with_a_lane_per_vertex() {
    let [x, y] = StrandSolve::Grouped.workgroups();
    assert_eq!(y, 1);
    assert_eq!(x * BRISTLE_STRANDS_PER_GROUP, MAX_BRISTLES as u32);
    assert_eq!(
        BRISTLE_STRANDS_PER_GROUP * BRISTLE_VERTICES as u32,
        BRISTLE_SOLVE_WORKGROUP_SIZE
    );
}
