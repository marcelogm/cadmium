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

#[test]
fn a_missing_pool_is_built_at_once() {
    let mut resize = ResizeDebounce::default();
    assert!(resize.needs_new_pool(None, (800, 600), Instant::now()));
}

#[test]
fn a_pool_of_the_right_size_is_kept_and_forgets_a_pending_resize() {
    let start = Instant::now();
    let mut resize = ResizeDebounce::default();
    assert!(!resize.needs_new_pool(Some((800, 600)), (820, 600), start));
    assert!(!resize.needs_new_pool(Some((800, 600)), (800, 600), start + RESIZE_SETTLE));
    // The earlier 820 wait was forgotten, so 820 starts waiting afresh.
    assert!(!resize.needs_new_pool(Some((800, 600)), (820, 600), start + RESIZE_SETTLE * 2));
}

#[test]
fn a_pool_of_another_size_is_rebuilt_once_the_size_settles() {
    let start = Instant::now();
    let mut resize = ResizeDebounce::default();
    assert!(!resize.needs_new_pool(Some((800, 600)), (820, 600), start));
    assert!(resize.needs_new_pool(Some((800, 600)), (820, 600), start + RESIZE_SETTLE));
}
