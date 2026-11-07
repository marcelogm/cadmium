use super::*;
use crate::pigments::Mixture;

const NO_PAINT: Mixture = Mixture {
    volume_um: 0.0,
    fractions: [0.0; crate::pigments::MAX_SLOTS],
};

fn dab_at(x: f32) -> Dab {
    Dab {
        x,
        ..bytemuck::Zeroable::zeroed()
    }
}

fn stroke_of(dab_count: usize, begun: bool, ended: bool) -> Queued {
    let mut stroke = QueuedStroke::new(Surface::Painting, Tip::Painting, BrushSettings::default());
    stroke.dabs = (0..dab_count).map(|i| dab_at(i as f32)).collect();
    stroke.begun = begun;
    stroke.ended = ended;
    Queued::Stroke(stroke)
}

fn knife_dip() -> Queued {
    Queued::Knife(KnifeLoad::Dip {
        mixture: NO_PAINT,
        amount: 1.0,
    })
}

fn restore() -> Queued {
    Queued::Brush(BrushChange::RestoreReserve { mixture: NO_PAINT })
}

fn apply(queue: &mut VecDeque<Queued>, step: WalkStep) {
    let head = queue.front_mut();
    match (step, head) {
        (WalkStep::Begin, Some(Queued::Stroke(stroke))) => stroke.begun = true,
        (WalkStep::Dabs(count), Some(Queued::Stroke(stroke))) => {
            stroke.dabs.drain(..count);
        }
        (WalkStep::End | WalkStep::Command | WalkStep::Barrier, Some(_)) => {
            queue.pop_front();
        }
        _ => {}
    }
}

fn walk(queue: &mut VecDeque<Queued>, limit: Option<usize>) -> Vec<WalkStep> {
    let mut progress = WalkProgress::limited_to(limit);
    let mut steps = Vec::new();
    loop {
        let step = next_step(queue.front(), progress);
        steps.push(step);
        if step == WalkStep::Wait {
            return steps;
        }
        apply(queue, step);
        progress = progress.took(step);
    }
}

#[test]
fn a_walk_ends_a_stroke_runs_the_change_behind_it_and_begins_the_next() {
    let mut queue = VecDeque::from([
        stroke_of(2, true, true),
        restore(),
        stroke_of(2, false, false),
    ]);
    assert_eq!(
        walk(&mut queue, None),
        [
            WalkStep::Dabs(2),
            WalkStep::End,
            WalkStep::Command,
            WalkStep::Begin,
            WalkStep::Dabs(2),
            WalkStep::Wait,
        ]
    );
}

#[test]
fn strokes_in_one_frame_share_its_dab_limit() {
    let mut queue = VecDeque::from([stroke_of(2, true, true), stroke_of(5, false, true)]);
    assert_eq!(
        walk(&mut queue, Some(4)),
        [
            WalkStep::Dabs(2),
            WalkStep::End,
            WalkStep::Begin,
            WalkStep::Dabs(2),
            WalkStep::Wait,
        ]
    );
    assert_eq!(
        walk(&mut queue, Some(4)),
        [WalkStep::Dabs(3), WalkStep::End, WalkStep::Wait]
    );
}

#[test]
fn a_knife_load_waits_for_the_next_frame_once_a_frame_has_encoded_work() {
    let mut queue = VecDeque::from([
        stroke_of(1, true, true),
        knife_dip(),
        stroke_of(1, false, true),
    ]);
    assert_eq!(
        walk(&mut queue, None),
        [WalkStep::Dabs(1), WalkStep::End, WalkStep::Wait]
    );
    assert_eq!(
        walk(&mut queue, None),
        [
            WalkStep::Barrier,
            WalkStep::Begin,
            WalkStep::Dabs(1),
            WalkStep::End,
            WalkStep::Wait,
        ]
    );
}

#[test]
fn a_stroke_under_way_holds_back_what_was_queued_behind_it() {
    let mut queue = VecDeque::from([stroke_of(0, true, false), restore()]);
    assert_eq!(walk(&mut queue, None), [WalkStep::Wait]);
    assert_eq!(queue.len(), 2);
}

#[test]
fn only_a_stroke_under_way_with_nothing_left_to_paint_lets_brush_changes_through() {
    assert!(!stroke_of(0, true, false).holds_back_brush_changes());
    assert!(stroke_of(1, true, false).holds_back_brush_changes());
    assert!(stroke_of(0, false, false).holds_back_brush_changes());
    assert!(restore().holds_back_brush_changes());
    assert!(knife_dip().holds_back_brush_changes());
}

#[test]
fn a_frame_out_of_dabs_stops_in_front_of_the_next_dab() {
    let mut queue = VecDeque::from([stroke_of(3, true, true)]);
    assert_eq!(
        walk(&mut queue, Some(1)),
        [WalkStep::Dabs(1), WalkStep::Wait]
    );
}
