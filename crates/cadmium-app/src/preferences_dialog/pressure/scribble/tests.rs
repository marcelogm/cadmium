use super::*;

#[test]
fn a_stroke_leaves_one_mark_per_move_as_wide_as_its_pressure() {
    let mut scribble = Scribble::default();
    scribble.begin((0.0, 0.0));
    scribble.extend((10.0, 0.0), 0.5);
    scribble.extend((20.0, 5.0), 1.0);
    let marks = scribble.marks();
    assert_eq!(marks.len(), 2);
    assert_eq!((marks[0].from, marks[0].to), ((0.0, 0.0), (10.0, 0.0)));
    assert_eq!(marks[0].width, MAX_WIDTH / 2.0);
    assert_eq!((marks[1].from, marks[1].width), ((10.0, 0.0), MAX_WIDTH));
}

#[test]
fn moving_without_a_stroke_leaves_nothing() {
    let mut scribble = Scribble::default();
    scribble.extend((10.0, 0.0), 1.0);
    scribble.begin((0.0, 0.0));
    scribble.end();
    scribble.extend((5.0, 5.0), 1.0);
    assert!(scribble.marks().is_empty());
}

#[test]
fn clearing_wipes_the_marks_and_ends_the_stroke() {
    let mut scribble = Scribble::default();
    scribble.begin((0.0, 0.0));
    scribble.extend((10.0, 0.0), 1.0);
    scribble.clear();
    scribble.extend((20.0, 0.0), 1.0);
    assert!(scribble.marks().is_empty());
}

#[test]
fn width_follows_pressure_inside_the_unit_range() {
    assert_eq!(width_for(0.0), 0.0);
    assert_eq!(width_for(2.0), MAX_WIDTH);
}
