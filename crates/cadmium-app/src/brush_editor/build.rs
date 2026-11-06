//! The editor dialog's widgets: the brush's summary.

use adw::prelude::*;
use cadmium_engine::brush::BrushSpec;

use super::summary_rows;

const CONTENT_WIDTH: i32 = 360;
const MARGIN: i32 = 12;
const SUMMARY_TITLE: &str = "Brush";

/// The editor showing `brush`.
pub(super) fn dialog(brush: &BrushSpec) -> adw::Dialog {
    let content = adw::ToolbarView::new();
    content.add_top_bar(&adw::HeaderBar::new());
    content.set_content(Some(&summary(brush)));
    adw::Dialog::builder()
        .title(&brush.name)
        .content_width(CONTENT_WIDTH)
        .child(&content)
        .build()
}

fn summary(brush: &BrushSpec) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(SUMMARY_TITLE)
        .margin_top(MARGIN)
        .margin_bottom(MARGIN)
        .margin_start(MARGIN)
        .margin_end(MARGIN)
        .valign(gtk::Align::Start)
        .build();
    for (title, value) in summary_rows(brush) {
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(value)
            .subtitle_selectable(true)
            .css_classes(["property"])
            .build();
        group.add(&row);
    }
    group
}
