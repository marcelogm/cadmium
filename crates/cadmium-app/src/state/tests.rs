use std::cell::Cell;
use std::rc::Rc;

use crate::state::test_support::painting_state;

// `clear_tray` stands in for any state change: it wakes the views, and `wake` is private.

#[test]
fn every_wake_reaches_each_view() {
    let mut state = painting_state();
    let wakes = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
    for count in &wakes {
        let counted = count.clone();
        state.add_waker(Rc::new(move || {
            counted.set(counted.get() + 1);
            true
        }));
    }
    state.clear_tray();
    state.clear_tray();
    assert_eq!(wakes.each_ref().map(|count| count.get()), [2, 2]);
}

#[test]
fn the_waker_of_a_view_that_is_gone_is_dropped_on_the_next_wake() {
    let mut state = painting_state();
    let wakes = Rc::new(Cell::new(0));
    let counted = wakes.clone();
    let gone = Rc::new(Cell::new(false));
    let seen_gone = gone.clone();
    state.add_waker(Rc::new(move || {
        counted.set(counted.get() + 1);
        !seen_gone.get()
    }));
    gone.set(true);
    state.clear_tray();
    assert_eq!(wakes.get(), 1);
    assert!(state.wakers.is_empty());
    state.clear_tray();
    assert_eq!(wakes.get(), 1);
}
