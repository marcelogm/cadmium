//! The editor dialog's widgets: the studio and its toolbar beside the summary.

use adw::prelude::*;
use cadmium_engine::brush::BrushSpec;

use super::{Runner, ToolbarAction, stroke_labels, summary_rows};
use crate::studio_view::StudioView;

const CONTENT_WIDTH: i32 = 960;
const CONTENT_HEIGHT: i32 = 640;
const MARGIN: i32 = 12;
const SPACING: i32 = 12;
const TOOLBAR_SPACING: i32 = 6;
const SUMMARY_WIDTH: i32 = 280;
const SUMMARY_TITLE: &str = "Brush";
const STROKE_TOOLTIP: &str = "Demo Stroke";
const SLOW_MOTION_LABEL: &str = "Slow Motion";
const RESET_VIEW_LABEL: &str = "Reset View";
const PAUSE_ICON: (&str, &str) = ("media-playback-pause-symbolic", "Pause");
const PLAY_ICON: (&str, &str) = ("media-playback-start-symbolic", "Play");

/// The editor showing `view` with the stroke at `stroke` picked.
pub(super) fn dialog(
    brush: &BrushSpec,
    view: &StudioView,
    stroke: usize,
    run: Runner,
) -> adw::Dialog {
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(SPACING)
        .margin_top(MARGIN)
        .margin_bottom(MARGIN)
        .margin_start(MARGIN)
        .margin_end(MARGIN)
        .build();
    body.append(&viewport(view, stroke, run));
    body.append(&summary(brush));
    let content = adw::ToolbarView::new();
    content.add_top_bar(&adw::HeaderBar::new());
    content.set_content(Some(&body));
    adw::Dialog::builder()
        .title(&brush.name)
        .content_width(CONTENT_WIDTH)
        .content_height(CONTENT_HEIGHT)
        .child(&content)
        .build()
}

fn viewport(view: &StudioView, stroke: usize, run: Runner) -> gtk::Box {
    let column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(SPACING)
        .hexpand(true)
        .vexpand(true)
        .build();
    view.set_hexpand(true);
    view.set_vexpand(true);
    column.append(view);
    column.append(&toolbar(stroke, run));
    column
}

fn toolbar(stroke: usize, run: Runner) -> gtk::Box {
    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(TOOLBAR_SPACING)
        .halign(gtk::Align::Center)
        .build();
    bar.append(&stroke_dropdown(stroke, run.clone()));
    bar.append(&play_button(run.clone()));
    bar.append(&slow_motion_button(run.clone()));
    bar.append(&reset_view_button(run));
    bar
}

fn stroke_dropdown(stroke: usize, run: Runner) -> gtk::DropDown {
    let dropdown = gtk::DropDown::from_strings(&stroke_labels());
    dropdown.set_tooltip_text(Some(STROKE_TOOLTIP));
    if let Ok(stroke) = u32::try_from(stroke) {
        dropdown.set_selected(stroke);
    }
    dropdown.connect_selected_notify(move |dropdown| {
        if let Ok(index) = usize::try_from(dropdown.selected()) {
            run(ToolbarAction::Stroke(index));
        }
    });
    dropdown
}

/// Starts pressed: a new studio plays.
fn play_button(run: Runner) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::builder().active(true).build();
    show_playing(&button);
    button.connect_toggled(move |button| {
        show_playing(button);
        run(ToolbarAction::Play(button.is_active()));
    });
    button
}

fn show_playing(button: &gtk::ToggleButton) {
    let (icon, tooltip) = if button.is_active() {
        PAUSE_ICON
    } else {
        PLAY_ICON
    };
    button.set_icon_name(icon);
    button.set_tooltip_text(Some(tooltip));
}

fn slow_motion_button(run: Runner) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::with_label(SLOW_MOTION_LABEL);
    button.connect_toggled(move |button| run(ToolbarAction::SlowMotion(button.is_active())));
    button
}

fn reset_view_button(run: Runner) -> gtk::Button {
    let button = gtk::Button::with_label(RESET_VIEW_LABEL);
    button.connect_clicked(move |_| run(ToolbarAction::ResetView));
    button
}

fn summary(brush: &BrushSpec) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title(SUMMARY_TITLE)
        .width_request(SUMMARY_WIDTH)
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
