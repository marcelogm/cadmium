use cadmium_engine::brush::{BrushSpec, factory_brushes};

use super::ExampleQueue;
use crate::thumbnails::ThumbnailKey;

fn key(index: usize) -> ThumbnailKey {
    ThumbnailKey::of(&factory_brushes()[index])
}

#[test]
fn the_first_draft_is_painted() {
    let mut queue = ExampleQueue::default();
    let order = queue.fire(key(0)).unwrap();
    assert!(queue.is_current(order));
}

#[test]
fn a_draft_already_shown_is_not_painted_again() {
    let mut queue = ExampleQueue::default();
    let order = queue.fire(key(0)).unwrap();
    assert!(queue.finished(order));
    assert_eq!(queue.fire(key(0)), None);
}

#[test]
fn a_renamed_draft_paints_nothing_new() {
    let mut queue = ExampleQueue::default();
    let order = queue.fire(key(0)).unwrap();
    queue.finished(order);
    let renamed = BrushSpec {
        name: "Renamed".into(),
        ..factory_brushes()[0].clone()
    };
    assert_eq!(queue.fire(ThumbnailKey::of(&renamed)), None);
}

#[test]
fn a_draft_being_painted_is_not_started_again() {
    let mut queue = ExampleQueue::default();
    let order = queue.fire(key(0)).unwrap();
    assert_eq!(queue.fire(key(0)), None);
    assert!(queue.is_current(order));
}

#[test]
fn a_new_draft_supersedes_the_painting_under_way() {
    let mut queue = ExampleQueue::default();
    let first = queue.fire(key(0)).unwrap();
    let second = queue.fire(key(1)).unwrap();
    assert!(!queue.is_current(first));
    assert!(!queue.finished(first));
    assert!(queue.finished(second));
    assert_eq!(queue.fire(key(1)), None);
}

#[test]
fn going_back_to_the_shown_draft_mid_painting_paints_it_again() {
    let mut queue = ExampleQueue::default();
    let shown = queue.fire(key(0)).unwrap();
    queue.finished(shown);
    let other = queue.fire(key(1)).unwrap();
    let back = queue.fire(key(0)).unwrap();
    assert!(!queue.is_current(other));
    assert!(queue.finished(back));
}

#[test]
fn a_failed_painting_can_be_tried_again() {
    let mut queue = ExampleQueue::default();
    let order = queue.fire(key(0)).unwrap();
    queue.failed(order);
    assert!(!queue.is_current(order));
    assert!(queue.fire(key(0)).is_some());
}
