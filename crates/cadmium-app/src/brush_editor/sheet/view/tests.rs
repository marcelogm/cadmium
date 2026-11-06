use super::frame_needed;

#[test]
fn a_sheet_that_is_gone_never_needs_a_frame() {
    for bits in 0..4 {
        let (needs_present, size_matches) = (bits & 1 != 0, bits & 2 != 0);
        assert!(!frame_needed(None, needs_present, size_matches));
    }
}

#[test]
fn a_resting_sheet_rests_once_presented_at_its_size() {
    assert!(!frame_needed(Some(false), false, true));
}

#[test]
fn a_sheet_needs_frames_while_it_paints_or_settles() {
    assert!(frame_needed(Some(true), false, true));
}

#[test]
fn a_sheet_needs_a_frame_for_a_queued_present() {
    assert!(frame_needed(Some(false), true, true));
}

#[test]
fn a_sheet_needs_a_frame_when_its_size_changed() {
    assert!(frame_needed(Some(false), false, false));
}
