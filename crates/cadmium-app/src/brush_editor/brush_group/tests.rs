use super::changes_size;

#[test]
fn a_fractional_size_read_back_rounded_is_no_change() {
    assert!(!changes_size(40.0, 40.4));
    assert!(!changes_size(41.0, 40.6));
    assert!(!changes_size(40.0, 40.0));
}

#[test]
fn another_whole_size_is_a_change() {
    assert!(changes_size(41.0, 40.4));
    assert!(changes_size(39.0, 40.0));
}
