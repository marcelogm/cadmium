use super::*;

fn nozzle(to: [f32; 2], seconds: f32) -> Nozzle {
    Nozzle {
        from: [0.0, 0.0],
        to,
        radius: 8.0,
        slot: 0,
        pressure: 1.0,
        seconds,
        still_reach_px: 0.0,
    }
}

fn held_for(seconds: f32) -> Dwell {
    Dwell {
        anchor: [0.0, 0.0],
        seconds,
    }
}

#[test]
fn a_nozzle_within_a_quarter_radius_of_its_anchor_keeps_dwelling() {
    let dwell = dwell_after(Some(held_for(0.5)), &nozzle([1.9, 0.0], 0.1));
    assert_eq!(dwell.anchor, [0.0, 0.0]);
    assert!((dwell.seconds - 0.6).abs() < 1e-6);
}

#[test]
fn a_nozzle_that_leaves_its_anchor_starts_dwelling_afresh_where_it_is() {
    let dwell = dwell_after(Some(held_for(0.5)), &nozzle([2.1, 0.0], 0.1));
    assert_eq!(dwell.anchor, [2.1, 0.0]);
    assert_eq!(dwell.seconds, 0.0);
}

#[test]
fn the_still_reach_is_the_wider_of_a_quarter_radius_and_the_screen_reach() {
    let reach = |screen_reach: f32| {
        still_reach_px(&Nozzle {
            still_reach_px: screen_reach,
            ..nozzle([0.0, 0.0], 0.1)
        })
    };
    assert_eq!(reach(12.0), 12.0);
    assert_eq!(reach(0.5), 2.0);
    for unknown in [0.0, -3.0, f32::NAN, f32::INFINITY] {
        assert_eq!(reach(unknown), 2.0);
    }
}

#[test]
fn the_first_squeeze_of_a_hold_dwells_from_where_it_touched() {
    let dwell = dwell_after(None, &nozzle([0.5, 0.0], 0.1));
    assert_eq!(dwell.anchor, [0.0, 0.0]);
    assert!((dwell.seconds - 0.1).abs() < 1e-6);
}

#[test]
fn a_moving_nozzle_raises_the_cord_whatever_its_speed() {
    let params = SimParams::default();
    let cord = params.get("tube_cord_um");
    for to in [[3.0, 0.0], [100.0, 0.0]] {
        let moving = nozzle(to, 0.02);
        let height = squeezed_height_um(&moving, &params, dwell_after(None, &moving));
        assert_eq!(height, cord);
    }
}

#[test]
fn a_blob_grows_only_after_the_nozzle_settles() {
    let params = SimParams::default();
    let still = nozzle([0.0, 0.0], 0.02);
    let height = |seconds| squeezed_height_um(&still, &params, held_for(seconds));
    assert_eq!(height(BLOB_SETTLE_SECONDS), params.get("tube_cord_um"));
    assert!(height(BLOB_SETTLE_SECONDS + 0.2) > height(BLOB_SETTLE_SECONDS + 0.1));
}

#[test]
fn a_blob_never_rises_above_the_tube_height() {
    let params = SimParams::default();
    let still = nozzle([0.0, 0.0], 0.02);
    assert_eq!(
        squeezed_height_um(&still, &params, held_for(60.0)),
        params.get("tube_max_um")
    );
}

#[test]
fn pressing_harder_raises_the_cord() {
    let params = SimParams::default();
    let hard = nozzle([20.0, 0.0], 0.1);
    let soft = Nozzle {
        pressure: 0.3,
        ..hard
    };
    assert!(
        squeezed_height_um(&hard, &params, held_for(0.0))
            > squeezed_height_um(&soft, &params, held_for(0.0))
    );
}

#[test]
fn no_time_squeezes_nothing() {
    let params = SimParams::default();
    for to in [[0.0, 0.0], [30.0, 0.0]] {
        assert_eq!(
            squeezed_height_um(&nozzle(to, 0.0), &params, held_for(1.0)),
            0.0
        );
    }
}
