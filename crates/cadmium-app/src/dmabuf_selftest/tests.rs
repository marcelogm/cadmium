use super::pattern::FramePattern;
use super::*;

#[test]
fn consecutive_frames_use_distinct_patterns() {
    let patterns: Vec<FramePattern> = (0..FRAMES).map(FramePattern::for_frame).collect();
    for pair in patterns.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
}

#[test]
fn first_frame_starts_from_the_reference_colour() {
    assert_eq!(
        FramePattern::for_frame(0).expected(0, 0),
        [40, 120, 200, 255]
    );
}

#[test]
fn pattern_varies_along_both_axes() {
    let pattern = FramePattern::for_frame(3);
    assert_ne!(pattern.expected(0, 0), pattern.expected(1, 0));
    assert_ne!(pattern.expected(0, 0), pattern.expected(0, 1));
}
