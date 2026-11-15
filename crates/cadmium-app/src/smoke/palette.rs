//! The Palette dialog smoke step: the dialog on show, in the asked mode, for the screenshot.

use std::ffi::OsString;

use adw::prelude::*;
use gtk::glib;

use crate::LOG_DOMAIN;
use crate::palette_editor;
use crate::state::PaletteMode;
use crate::window::CadmiumWindow;

pub(super) const PALETTE: &str = "CADMIUM_SMOKE_PALETTE";

/// The mode `value` names (`circle`, `mixing`); any other value is ignored, with a warning.
pub(super) fn parse_mode(value: OsString) -> Option<PaletteMode> {
    let parsed = match value.to_str() {
        Some("circle") => Some(PaletteMode::Circle),
        Some("mixing") => Some(PaletteMode::Mixing),
        _ => None,
    };
    if parsed.is_none() {
        glib::g_warning!(
            LOG_DOMAIN,
            "ignoring {PALETTE}={value:?}: not a palette mode"
        );
    }
    parsed
}

/// Opens the Palette dialog in `mode`. A dialog that does not open ends the run: its
/// screenshot would lie.
pub(super) fn open(window: &CadmiumWindow, mode: PaletteMode) {
    palette_editor::open_in(window, mode);
    if !window.has_editor() {
        glib::g_critical!(LOG_DOMAIN, "smoke palette: the Palette dialog did not open");
        std::process::exit(1);
    }
}

/// Whether the dialog's mixing sheet, if one is on show, has shown a frame and rests.
pub(super) fn sheet_at_rest(window: &CadmiumWindow) -> bool {
    let sheet = window
        .editor()
        .and_then(|dialog| palette_editor::sheet_view_in(&dialog))
        .filter(|sheet| sheet.is_mapped());
    match sheet {
        None => true,
        Some(sheet) => sheet.has_frame() && !sheet.is_presenting(),
    }
}
