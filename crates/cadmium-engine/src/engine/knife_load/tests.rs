use super::*;
use crate::engine::BLADE_LOAD_VALUES;

#[test]
fn a_nearly_full_blade_cell_limits_what_the_knife_draws_up() {
    let shares = blade_shares(4.0);
    let mut load = vec![0.0; BLADE_LOAD_VALUES];
    let empty_room = blade_room(&load, &shares);
    let (crowded, _) = shares[shares.len() / 2];
    load[crowded * TIP_CHANNELS + VOLUME_CHANNEL] = KNIFE_LAID_CAPACITY_UM - 10.0;
    let room = blade_room(&load, &shares);
    assert!(room < empty_room);
    let taken = PaintTotals {
        volume: f64::from(room),
        ..PaintTotals::default()
    };
    add_to_blade(&mut load, &shares, &taken);
    let fullest = load
        .chunks(TIP_CHANNELS)
        .map(|cell| cell[VOLUME_CHANNEL])
        .fold(0.0, f32::max);
    assert!(fullest <= KNIFE_LAID_CAPACITY_UM * (1.0 + 1e-6));
    assert!((load[crowded * TIP_CHANNELS + VOLUME_CHANNEL] - KNIFE_LAID_CAPACITY_UM).abs() < 1e-2);
}
