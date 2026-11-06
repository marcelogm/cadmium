//! The brush editor: a read-only summary of the worn brush.

use adw::prelude::*;
use cadmium_engine::brush::BrushSpec;
use cadmium_studio::{hair_name, head_name};
use gtk::glib;

use crate::window::CadmiumWindow;

mod build;

/// What the summary calls a head or hair that is no longer a factory one.
const CUSTOM: &str = "Custom";

/// Shows the editor on the worn brush; while one is open, does nothing.
pub fn open(window: &CadmiumWindow) {
    if window.has_editor() {
        return;
    }
    let state = window.state();
    let brush = state.borrow().worn_brush_spec();
    let dialog = build::dialog(&brush);
    dialog.connect_closed(glib::clone!(
        #[weak]
        window,
        move |_| window.after_editor_closed()
    ));
    window.keep_editor(&dialog);
    dialog.present(Some(window));
}

/// The read-only rows describing `brush`, as (title, value).
fn summary_rows(brush: &BrushSpec) -> [(&'static str, String); 4] {
    [
        ("Name", brush.name.clone()),
        ("Head", head_name(&brush.head).unwrap_or(CUSTOM).to_owned()),
        ("Hair", hair_name(&brush.hair).unwrap_or(CUSTOM).to_owned()),
        ("Size", format!("{:.0} px", brush.size_px)),
    ]
}

#[cfg(test)]
mod tests;
