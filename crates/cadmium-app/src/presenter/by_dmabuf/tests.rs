use super::*;

#[test]
fn pool_is_rebuilt_only_after_the_size_holds_still() {
    let start = Instant::now();
    let mut resize = ResizeDebounce::default();
    assert!(!resize.has_settled((800, 600), start));
    assert!(!resize.has_settled((800, 600), start + RESIZE_SETTLE / 2));
    assert!(resize.has_settled((800, 600), start + RESIZE_SETTLE));
}

#[test]
fn a_new_size_during_a_drag_restarts_the_wait() {
    let start = Instant::now();
    let mut resize = ResizeDebounce::default();
    assert!(!resize.has_settled((800, 600), start));
    assert!(!resize.has_settled((820, 600), start + RESIZE_SETTLE));
    assert!(!resize.has_settled((820, 600), start + RESIZE_SETTLE * 3 / 2));
    assert!(resize.has_settled((820, 600), start + RESIZE_SETTLE * 2));
}
