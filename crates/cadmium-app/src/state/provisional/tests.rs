use super::*;

const UPRIGHT: ViewTransform = ViewTransform {
    origin_x: 10.0,
    origin_y: 20.0,
    zoom: 2.0,
    rotation: 0.0,
};
const BLUE: [u8; 3] = [20, 40, 160];

fn dab_at(x: f32, radius: f32) -> Dab {
    Dab {
        x,
        y: 50.0,
        radius,
        pressure: 1.0,
        altitude: 0.0,
        azimuth: 0.0,
        direction: 0.0,
        speed: 0.0,
        time_ms: 0.0,
    }
}

fn row(count: usize, step_px: f32, radius: f32) -> Vec<Dab> {
    (0..count)
        .map(|index| dab_at(index as f32 * step_px, radius))
        .collect()
}

#[test]
fn dabs_closer_than_half_a_radius_collapse_into_fewer_marks() {
    let trail = ProvisionalTrail::of(BLUE, &row(100, 1.0, 20.0), UPRIGHT);
    let marks = trail.canvas_marks();
    assert!(marks.len() <= 14, "{} marks", marks.len());
    assert!(
        marks
            .windows(2)
            .all(|pair| pair[1].centre.0 - pair[0].centre.0 <= 10.0)
    );
}

#[test]
fn the_tip_of_the_queue_is_always_kept() {
    let trail = ProvisionalTrail::of(BLUE, &row(7, 1.0, 20.0), UPRIGHT);
    let tip = trail.canvas_marks().last().copied();
    assert_eq!(tip.map(|mark| mark.centre.0), Some(6.0));
}

#[test]
fn well_spread_dabs_each_keep_their_mark() {
    let trail = ProvisionalTrail::of(BLUE, &row(5, 30.0, 20.0), UPRIGHT);
    assert_eq!(trail.canvas_marks().len(), 5);
}

#[test]
fn widget_marks_follow_the_view_the_texture_was_drawn_with() {
    let trail = ProvisionalTrail::of(BLUE, &[dab_at(30.0, 5.0)], UPRIGHT);
    let marks = trail.widget_marks(2.0);
    assert_eq!(
        marks,
        vec![Mark {
            centre: (20.0, 30.0),
            radius: 5.0
        }]
    );
}

#[test]
fn no_queued_dabs_leave_no_marks() {
    assert!(
        ProvisionalTrail::of(BLUE, &[], UPRIGHT)
            .canvas_marks()
            .is_empty()
    );
}

#[test]
fn painting_the_head_of_the_queue_leaves_the_marks_behind_it_in_place() {
    let queued = row(100, 1.0, 20.0);
    let before = ProvisionalTrail::of(BLUE, &queued, UPRIGHT);
    let after = ProvisionalTrail::of(BLUE, &queued[3..], UPRIGHT);
    for mark in &after.canvas_marks()[1..] {
        assert!(
            before.canvas_marks().contains(mark),
            "the mark at {:?} moved",
            mark.centre
        );
    }
}
