use super::*;

#[test]
fn the_zoom_osd_sits_its_margin_above_whatever_of_the_tray_covers_the_canvas() {
    assert_eq!(zoom_osd_margin(0, 0), OSD_MARGIN);
    // Closed: the bottom bar covers the canvas's foot.
    assert_eq!(zoom_osd_margin(44, 0), OSD_MARGIN + 44);
    // Opening: the sheet grows past the bar.
    assert_eq!(zoom_osd_margin(44, 30), OSD_MARGIN + 44);
    assert_eq!(zoom_osd_margin(44, 380), OSD_MARGIN + 380);
}
