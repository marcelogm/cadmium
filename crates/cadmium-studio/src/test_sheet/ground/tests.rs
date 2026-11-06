use super::{COARSE_SHARE, FINE_SHARE, line_share};

#[test]
fn a_pixel_off_the_lines_keeps_the_whole_paper() {
    for at in [(10, 10), (2, 2), (19, 37), (99, 63)] {
        assert_eq!(line_share(at.0, at.1), 1.0, "{at:?}");
    }
}

#[test]
fn fine_lines_fall_every_fine_step_two_pixels_wide() {
    for at in [(20, 10), (21, 10), (10, 40), (10, 41), (180, 57)] {
        assert_eq!(line_share(at.0, at.1), FINE_SHARE, "{at:?}");
    }
    assert_eq!(line_share(22, 10), 1.0);
}

#[test]
fn coarse_lines_fall_every_fifth_fine_step_and_win_where_they_cross() {
    for at in [
        (0, 10),
        (100, 10),
        (101, 37),
        (10, 200),
        (100, 20),
        (40, 300),
    ] {
        assert_eq!(line_share(at.0, at.1), COARSE_SHARE, "{at:?}");
    }
}
