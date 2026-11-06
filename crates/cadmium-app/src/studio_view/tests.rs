use super::frame_needed;

#[test]
fn a_closed_studio_never_needs_a_frame() {
    for bits in 0..8 {
        let (playing, needs_present, size_matches) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
        assert!(!frame_needed(false, playing, needs_present, size_matches));
    }
}

#[test]
fn an_open_paused_studio_rests_once_presented_at_its_size() {
    assert!(!frame_needed(true, false, false, true));
}

#[test]
fn an_open_studio_needs_a_frame_while_playing() {
    assert!(frame_needed(true, true, false, true));
}

#[test]
fn an_open_studio_needs_a_frame_for_a_queued_present() {
    assert!(frame_needed(true, false, true, true));
}

#[test]
fn an_open_studio_needs_a_frame_when_its_size_changed() {
    assert!(frame_needed(true, false, false, false));
}
