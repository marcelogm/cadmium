const PANEL_MARGIN: i32 = 12;
const GROUP_SPACING: i32 = 18;

pub(crate) fn panel_root() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(GROUP_SPACING)
        .margin_top(PANEL_MARGIN)
        .margin_bottom(PANEL_MARGIN)
        .margin_start(PANEL_MARGIN)
        .margin_end(PANEL_MARGIN)
        .build()
}
